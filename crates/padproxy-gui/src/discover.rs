//! Background webcam button-discovery session driving the GUI's discovery flow.
//!
//! A worker thread reads the webcam and the controller, localizes each button
//! press against a resting baseline (see `padproxy_core::vision`), and sends
//! located buttons plus a downscaled preview image back to the UI.

use anyhow::{Context, Result};
use base64::Engine;
use evdev::Device;
use padproxy_core::event_code::{event_from_input, EventKind};
use padproxy_core::vision::{localize_press, ControllerLayout, GrayFrame};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;

pub enum DiscoverMessage {
    Located {
        code: String,
        x: f32,
        y: f32,
    },
    /// A `data:image/png;base64,...` URI of the latest (downscaled) camera frame.
    Preview(String),
    Status(String),
    Failed(String),
}

pub struct DiscoverSession {
    stop: Arc<AtomicBool>,
    receiver: mpsc::Receiver<DiscoverMessage>,
    layout: Arc<Mutex<ControllerLayout>>,
    thread: Option<JoinHandle<()>>,
}

impl DiscoverSession {
    pub fn start(controller_path: String, camera_path: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let (sender, receiver) = mpsc::channel();
        let layout = Arc::new(Mutex::new(ControllerLayout::default()));
        let worker_layout = Arc::clone(&layout);

        let thread = std::thread::spawn(move || {
            if let Err(error) = run(
                &controller_path,
                &camera_path,
                &worker_stop,
                &sender,
                &worker_layout,
            ) {
                let _ = sender.send(DiscoverMessage::Failed(error.to_string()));
            }
        });

        Self {
            stop,
            receiver,
            layout,
            thread: Some(thread),
        }
    }

    pub fn poll(&self) -> Vec<DiscoverMessage> {
        self.receiver.try_iter().collect()
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    /// The accumulated layout as JSON (same shape as `discover-cam` output).
    pub fn layout_json(&self) -> String {
        self.layout
            .lock()
            .ok()
            .and_then(|layout| serde_json::to_string(&*layout).ok())
            .unwrap_or_default()
    }
}

impl Drop for DiscoverSession {
    fn drop(&mut self) {
        self.stop();
    }
}

fn yuyv_to_gray(buf: &[u8], width: u32, height: u32) -> GrayFrame {
    let count = (width * height) as usize;
    let mut pixels = Vec::with_capacity(count);
    for i in 0..count {
        pixels.push(buf.get(i * 2).copied().unwrap_or(0));
    }
    GrayFrame::new(width, height, pixels)
}

/// Downscale a grayscale frame 2x by nearest sampling.
fn downscale2(frame: &GrayFrame) -> (u32, u32, Vec<u8>) {
    let (w, h) = (frame.width / 2, frame.height / 2);
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            out.push(frame.pixels[((y * 2) * frame.width + x * 2) as usize]);
        }
    }
    (w, h, out)
}

fn preview_data_uri(frame: &GrayFrame) -> Option<String> {
    let (w, h, pixels) = downscale2(frame);
    let mut png = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut png, w, h);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(&pixels).ok()?;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{b64}"))
}

fn run(
    controller_path: &str,
    camera_path: &str,
    stop: &AtomicBool,
    sender: &mpsc::Sender<DiscoverMessage>,
    layout: &Mutex<ControllerLayout>,
) -> Result<()> {
    let mut controller = Device::open(controller_path)
        .with_context(|| format!("failed to open controller {controller_path}"))?;
    controller.set_nonblocking(true).ok();

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

    let _ = sender.send(DiscoverMessage::Status(
        "Point the webcam at your controller, then press each button once.".to_string(),
    ));

    let mut baseline: Option<GrayFrame> = None;
    let mut held: i32 = 0;
    let mut tick: u32 = 0;

    while !stop.load(Ordering::Relaxed) {
        let buf = camera.capture().context("camera frame read failed")?;
        let frame = yuyv_to_gray(&buf, width, height);

        // Throttle preview updates to ~6 fps.
        tick = tick.wrapping_add(1);
        if tick % 5 == 0 {
            if let Some(uri) = preview_data_uri(&frame) {
                let _ = sender.send(DiscoverMessage::Preview(uri));
            }
        }

        match controller.fetch_events() {
            Ok(events) => {
                for event in events {
                    let Some(code) = event_from_input(event) else {
                        continue;
                    };
                    if code.kind != EventKind::Key || !code.name().starts_with("btn:") {
                        continue;
                    }
                    match event.value() {
                        1 => {
                            held += 1;
                            if let Some(loc) = baseline
                                .as_ref()
                                .and_then(|base| localize_press(base, &frame, 25, 0.001))
                            {
                                if let Ok(mut layout) = layout.lock() {
                                    layout.record(&code.name(), loc);
                                }
                                let _ = sender.send(DiscoverMessage::Located {
                                    code: code.name(),
                                    x: loc.x,
                                    y: loc.y,
                                });
                            } else {
                                let _ = sender.send(DiscoverMessage::Status(format!(
                                    "{}: no clear change — press more distinctly",
                                    code.name()
                                )));
                            }
                        }
                        0 => held = (held - 1).max(0),
                        _ => {}
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => return Err(error).context("failed reading controller events"),
        }

        if held == 0 {
            baseline = Some(frame);
        }
    }

    Ok(())
}
