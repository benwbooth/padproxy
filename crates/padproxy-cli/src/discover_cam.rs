//! Webcam button discovery: point a camera at a controller, press each button
//! once, and PadProxy locates where on the controller each button is by
//! correlating the press event with the visual change in the frame. Produces a
//! per-controller layout (code -> normalized x/y) usable for a custom diagram.

use anyhow::{Context, Result};
use evdev::Device;
use padproxy_core::event_code::{event_from_input, EventKind};
use padproxy_core::vision::{localize_press, ControllerLayout, GrayFrame};
use std::path::PathBuf;

/// Convert a YUYV buffer (`Y0 U Y1 V` per two pixels) to grayscale by taking the
/// luminance byte of every pixel.
fn yuyv_to_gray(buf: &[u8], width: u32, height: u32) -> GrayFrame {
    let count = (width * height) as usize;
    let mut pixels = Vec::with_capacity(count);
    for i in 0..count {
        pixels.push(buf.get(i * 2).copied().unwrap_or(0));
    }
    GrayFrame::new(width, height, pixels)
}

fn save_layout(layout: &ControllerLayout, output: &PathBuf) -> Result<()> {
    let json = serde_json::to_string_pretty(layout)?;
    std::fs::write(output, json).with_context(|| format!("failed to write {}", output.display()))
}

pub fn run(controller_path: &str, camera_path: &str, output: PathBuf) -> Result<()> {
    let mut controller = Device::open(controller_path)
        .with_context(|| format!("failed to open controller {controller_path}"))?;
    controller
        .set_nonblocking(true)
        .context("failed to set controller nonblocking")?;

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
        .context("failed to start camera (does it support 640x480 YUYV?)")?;

    eprintln!(
        "Point the webcam at your controller ({width}x{height}). Press each button once, \
         clearly. Located buttons are saved to {} as you go. Press Ctrl-C when done.",
        output.display()
    );

    let mut layout = ControllerLayout::default();
    let mut baseline: Option<GrayFrame> = None;
    let mut held: i32 = 0;

    loop {
        let buf = camera.capture().context("camera frame read failed")?;
        let frame = yuyv_to_gray(&buf, width, height);

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
                            match baseline
                                .as_ref()
                                .and_then(|base| localize_press(base, &frame, 25, 0.001))
                            {
                                Some(loc) => {
                                    layout.record(&code.name(), loc);
                                    eprintln!(
                                        "located {} at ({:.2}, {:.2})",
                                        code.name(),
                                        loc.x,
                                        loc.y
                                    );
                                    save_layout(&layout, &output)?;
                                }
                                None => eprintln!(
                                    "{}: no clear visual change — press more distinctly",
                                    code.name()
                                ),
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

        // Refresh the resting baseline whenever no button is held.
        if held == 0 {
            baseline = Some(frame);
        }
    }
}
