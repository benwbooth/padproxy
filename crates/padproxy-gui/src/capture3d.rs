//! Orbit-capture for 3D / gaussian-splat reconstruction of a controller.
//!
//! A worker thread records color frames from the webcam while the user slowly
//! rotates the controller, saving them as a dataset plus a ready-to-run pipeline
//! script. The gaussian-splat *training* itself happens in external GPU tooling
//! (COLMAP for camera poses + `brush` for training/viewing) — this produces the
//! input those tools consume and can launch them when installed.

use anyhow::{Context, Result};
use base64::Engine;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

pub enum CaptureMessage {
    Progress { saved: u32, target: u32 },
    Preview(String),
    Status(String),
    Done(String),
    Failed(String),
}

pub struct OrbitCaptureSession {
    stop: Arc<AtomicBool>,
    receiver: mpsc::Receiver<CaptureMessage>,
    thread: Option<JoinHandle<()>>,
}

impl OrbitCaptureSession {
    pub fn start(camera_path: String, output_dir: String, target_frames: u32) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let (sender, receiver) = mpsc::channel();

        let thread = std::thread::spawn(move || {
            if let Err(error) = run(
                &camera_path,
                Path::new(&output_dir),
                target_frames,
                &worker_stop,
                &sender,
            ) {
                let _ = sender.send(CaptureMessage::Failed(error.to_string()));
            }
        });

        Self {
            stop,
            receiver,
            thread: Some(thread),
        }
    }

    pub fn poll(&self) -> Vec<CaptureMessage> {
        self.receiver.try_iter().collect()
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for OrbitCaptureSession {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Convert a YUYV buffer to interleaved RGB (BT.601).
fn yuyv_to_rgb(buf: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixels = (width * height) as usize;
    let mut rgb = vec![0u8; pixels * 3];
    let mut pixel = 0usize;
    let mut byte = 0usize;
    while pixel + 1 < pixels && byte + 3 < buf.len() {
        let y0 = buf[byte] as f32;
        let u = buf[byte + 1] as f32 - 128.0;
        let y1 = buf[byte + 2] as f32;
        let v = buf[byte + 3] as f32 - 128.0;
        for (k, y) in [y0, y1].into_iter().enumerate() {
            let r = (y + 1.402 * v).clamp(0.0, 255.0) as u8;
            let g = (y - 0.344 * u - 0.714 * v).clamp(0.0, 255.0) as u8;
            let b = (y + 1.772 * u).clamp(0.0, 255.0) as u8;
            let out = (pixel + k) * 3;
            rgb[out] = r;
            rgb[out + 1] = g;
            rgb[out + 2] = b;
        }
        pixel += 2;
        byte += 4;
    }
    rgb
}

fn encode_png_rgb(width: u32, height: u32, rgb: &[u8]) -> Result<Vec<u8>> {
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(rgb)?;
    }
    Ok(png)
}

/// Nearest 2x downscale of interleaved RGB.
fn downscale2_rgb(width: u32, height: u32, rgb: &[u8]) -> (u32, u32, Vec<u8>) {
    let (w, h) = (width / 2, height / 2);
    let mut out = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            let src = (((y * 2) * width + x * 2) * 3) as usize;
            out.extend_from_slice(&rgb[src..src + 3]);
        }
    }
    (w, h, out)
}

fn preview_data_uri(width: u32, height: u32, rgb: &[u8]) -> Option<String> {
    let (w, h, small) = downscale2_rgb(width, height, rgb);
    let png = encode_png_rgb(w, h, &small).ok()?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{b64}"))
}

/// Write a ready-to-run COLMAP + brush pipeline next to the captured frames.
fn write_pipeline(dir: &Path) -> Result<PathBuf> {
    let script = dir.join("run-splat.sh");
    let body = r#"#!/usr/bin/env bash
# Gaussian-splat pipeline for this PadProxy capture.
# Requires a GPU plus COLMAP (camera poses) and brush (training + viewer):
#   COLMAP: https://colmap.github.io/
#   brush:  https://github.com/ArthurBrussee/brush  (cargo install or release binary)
set -euo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"

echo "[1/2] Estimating camera poses with COLMAP..."
mkdir -p "$DIR/colmap"
colmap automatic_reconstructor \
  --workspace_path "$DIR/colmap" \
  --image_path "$DIR/frames" \
  --camera_model SIMPLE_PINHOLE \
  --dense 0

echo "[2/2] Training + viewing the gaussian splat with brush..."
brush "$DIR/colmap"
"#;
    std::fs::write(&script, body)
        .with_context(|| format!("failed to write {}", script.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms)?;
    }
    Ok(script)
}

fn run(
    camera_path: &str,
    output_dir: &Path,
    target_frames: u32,
    stop: &AtomicBool,
    sender: &mpsc::Sender<CaptureMessage>,
) -> Result<()> {
    let frames_dir = output_dir.join("frames");
    std::fs::create_dir_all(&frames_dir)
        .with_context(|| format!("failed to create {}", frames_dir.display()))?;

    let mut camera = rscam::Camera::new(camera_path)
        .with_context(|| format!("failed to open camera {camera_path}"))?;
    let (width, height) = (640u32, 480u32);
    camera
        .start(&rscam::Config {
            interval: (1, 30),
            resolution: (width, height),
            format: b"YUYV",
            ..Default::default()
        })
        .context("camera does not support 640x480 YUYV")?;

    let _ = sender.send(CaptureMessage::Status(
        "Slowly rotate the controller so the camera sees every side.".to_string(),
    ));

    let mut saved: u32 = 0;
    let mut tick: u32 = 0;
    // Save roughly 3 frames/sec at 30fps capture so successive views differ.
    let save_every = 10u32;

    while saved < target_frames && !stop.load(Ordering::Relaxed) {
        let buf = camera.capture().context("camera frame read failed")?;
        let rgb = yuyv_to_rgb(&buf, width, height);
        tick = tick.wrapping_add(1);

        if tick % 3 == 0 {
            if let Some(uri) = preview_data_uri(width, height, &rgb) {
                let _ = sender.send(CaptureMessage::Preview(uri));
            }
        }

        if tick % save_every == 0 {
            let png = encode_png_rgb(width, height, &rgb)?;
            let path = frames_dir.join(format!("frame_{saved:04}.png"));
            std::fs::write(&path, png)
                .with_context(|| format!("failed to write {}", path.display()))?;
            saved += 1;
            let _ = sender.send(CaptureMessage::Progress {
                saved,
                target: target_frames,
            });
        }
    }

    let script = write_pipeline(output_dir)?;
    let _ = sender.send(CaptureMessage::Status(format!(
        "Saved {saved} frames. Run {} to build the splat (needs COLMAP + brush + a GPU).",
        script.display()
    )));
    let _ = sender.send(CaptureMessage::Done(output_dir.display().to_string()));
    Ok(())
}

/// Launch the generated splat pipeline if COLMAP and brush are on PATH.
/// Returns guidance describing what happened.
pub fn launch_pipeline(output_dir: &str) -> String {
    let have = |tool: &str| {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("command -v {tool}"))
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    };

    let missing: Vec<&str> = ["colmap", "brush"]
        .into_iter()
        .filter(|tool| !have(tool))
        .collect();
    if !missing.is_empty() {
        return format!(
            "Install {} to build the splat, then run run-splat.sh in {output_dir}. \
             COLMAP: colmap.github.io · brush: github.com/ArthurBrussee/brush",
            missing.join(" and ")
        );
    }

    let script = Path::new(output_dir).join("run-splat.sh");
    match std::process::Command::new("bash").arg(&script).spawn() {
        Ok(_) => format!(
            "Started splat pipeline ({}). The brush viewer will open when training begins.",
            script.display()
        ),
        Err(error) => format!("Failed to launch pipeline: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{write_pipeline, yuyv_to_rgb};

    #[test]
    fn yuyv_gray_maps_to_gray_rgb() {
        // Two neutral pixels (Y=128, U=V=128 → no chroma).
        let rgb = yuyv_to_rgb(&[128, 128, 128, 128], 2, 1);
        assert_eq!(rgb.len(), 6);
        for channel in rgb {
            assert!((channel as i16 - 128).abs() <= 1, "channel={channel}");
        }
    }

    #[test]
    fn pipeline_script_is_written_and_runnable() {
        let dir = std::env::temp_dir().join(format!("padproxy-splat-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = write_pipeline(&dir).unwrap();
        let body = std::fs::read_to_string(&script).unwrap();
        assert!(body.contains("colmap"));
        assert!(body.contains("brush"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&script).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "script should be executable");
        }
        std::fs::remove_dir_all(&dir).ok();
    }
}
