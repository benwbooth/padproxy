//! Webcam press-localization: figure out *where* on a controller each button is
//! by correlating a button-press event with the visual change in a webcam frame.
//!
//! The idea sidesteps both unreliable visual button detection and the fact that
//! appearance can't reveal an evdev code: the user presses one button at a time,
//! the input layer already knows which code fired, and this module finds where
//! in the frame the press happened (the finger/button moving against a resting
//! baseline). Input event + located change → a labelled hotspot for that code.

use serde::Serialize;

/// A grayscale frame.
#[derive(Clone, Debug)]
pub struct GrayFrame {
    pub width: u32,
    pub height: u32,
    /// Row-major luminance, length = width * height.
    pub pixels: Vec<u8>,
}

impl GrayFrame {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Self {
        debug_assert_eq!(pixels.len(), (width * height) as usize);
        Self {
            width,
            height,
            pixels,
        }
    }
}

/// Normalized location of a localized press (0.0..1.0 within the frame).
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct PressLocation {
    pub x: f32,
    pub y: f32,
    /// Fraction of pixels that changed — a rough confidence/extent signal.
    pub area: f32,
}

/// Locate where a press happened by comparing a resting `baseline` frame with a
/// frame captured at the press. Returns the intensity-weighted centroid of the
/// changed region, or `None` if too little changed (likely noise).
///
/// `threshold` is the per-pixel luminance delta that counts as "changed".
/// `min_area` is the minimum fraction of changed pixels required (0.0..1.0).
pub fn localize_press(
    baseline: &GrayFrame,
    pressed: &GrayFrame,
    threshold: u8,
    min_area: f32,
) -> Option<PressLocation> {
    if baseline.width != pressed.width
        || baseline.height != pressed.height
        || baseline.pixels.len() != pressed.pixels.len()
        || baseline.pixels.is_empty()
    {
        return None;
    }

    let width = baseline.width as usize;
    let mut weight_sum = 0f64;
    let mut x_sum = 0f64;
    let mut y_sum = 0f64;
    let mut changed = 0u64;

    for (index, (&b, &p)) in baseline.pixels.iter().zip(&pressed.pixels).enumerate() {
        let delta = (b as i16 - p as i16).unsigned_abs();
        if delta as u8 >= threshold {
            let x = (index % width) as f64;
            let y = (index / width) as f64;
            let w = delta as f64;
            x_sum += x * w;
            y_sum += y * w;
            weight_sum += w;
            changed += 1;
        }
    }

    let total = baseline.pixels.len() as f32;
    let area = changed as f32 / total;
    if weight_sum <= 0.0 || area < min_area {
        return None;
    }

    Some(PressLocation {
        x: (x_sum / weight_sum) as f32 / baseline.width as f32,
        y: (y_sum / weight_sum) as f32 / baseline.height as f32,
        area,
    })
}

/// A control located on the captured controller, ready to drive a per-device
/// diagram or mapping.
#[derive(Clone, Debug, Serialize)]
pub struct LocatedControl {
    /// Event code name, e.g. `btn:south`.
    pub code: String,
    pub x: f32,
    pub y: f32,
}

/// Accumulates located controls discovered during a capture session, keeping the
/// strongest (largest-area) localization per code.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ControllerLayout {
    pub controls: Vec<LocatedControl>,
    #[serde(skip)]
    best_area: std::collections::HashMap<String, f32>,
}

impl ControllerLayout {
    pub fn record(&mut self, code: &str, location: PressLocation) {
        let better = self
            .best_area
            .get(code)
            .map(|&best| location.area > best)
            .unwrap_or(true);
        if !better {
            return;
        }
        self.best_area.insert(code.to_string(), location.area);
        if let Some(existing) = self.controls.iter_mut().find(|c| c.code == code) {
            existing.x = location.x;
            existing.y = location.y;
        } else {
            self.controls.push(LocatedControl {
                code: code.to_string(),
                x: location.x,
                y: location.y,
            });
        }
    }

    pub fn len(&self) -> usize {
        self.controls.len()
    }

    pub fn is_empty(&self) -> bool {
        self.controls.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::{localize_press, ControllerLayout, GrayFrame, PressLocation};

    fn frame_with_bright_square(w: u32, h: u32, cx: u32, cy: u32, half: u32) -> GrayFrame {
        let mut pixels = vec![0u8; (w * h) as usize];
        for y in cy.saturating_sub(half)..(cy + half).min(h) {
            for x in cx.saturating_sub(half)..(cx + half).min(w) {
                pixels[(y * w + x) as usize] = 255;
            }
        }
        GrayFrame::new(w, h, pixels)
    }

    #[test]
    fn localizes_change_to_the_pressed_region() {
        let baseline = GrayFrame::new(100, 100, vec![0u8; 100 * 100]);
        // A bright "finger/button" change in the lower-right quadrant.
        let pressed = frame_with_bright_square(100, 100, 75, 80, 6);

        let loc = localize_press(&baseline, &pressed, 40, 0.0005).unwrap();
        assert!((loc.x - 0.75).abs() < 0.05, "x={}", loc.x);
        assert!((loc.y - 0.80).abs() < 0.05, "y={}", loc.y);
        assert!(loc.area > 0.0);
    }

    #[test]
    fn rejects_noise_below_min_area() {
        let baseline = GrayFrame::new(100, 100, vec![0u8; 100 * 100]);
        // A single changed pixel — far below min_area.
        let mut px = vec![0u8; 100 * 100];
        px[42] = 255;
        let pressed = GrayFrame::new(100, 100, px);
        assert!(localize_press(&baseline, &pressed, 40, 0.01).is_none());
    }

    #[test]
    fn layout_keeps_strongest_localization_per_code() {
        let mut layout = ControllerLayout::default();
        layout.record(
            "btn:south",
            PressLocation {
                x: 0.1,
                y: 0.1,
                area: 0.01,
            },
        );
        layout.record(
            "btn:south",
            PressLocation {
                x: 0.8,
                y: 0.8,
                area: 0.05,
            },
        );
        assert_eq!(layout.len(), 1);
        assert!((layout.controls[0].x - 0.8).abs() < 1e-6);
    }
}
