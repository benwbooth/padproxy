//! Controller LED settings.
//!
//! Linux exposes device LEDs under `/sys/class/leds`. Game controllers (DualShock
//! player lights, DualSense RGB, etc.) appear here with a `brightness` /
//! `max_brightness` pair and, for RGB lights, a `multi_intensity` triple.
//! PadProxy enumerates and sets them so player indicators and light bars can be
//! configured, matching reWASD's LED settings.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::Path;

const LEDS_DIR: &str = "/sys/class/leds";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LedInfo {
    /// LED node name, e.g. `input5::numlock` or `0005:054C:09CC.0003:rgb:indicator`.
    pub name: String,
    /// Current brightness, when readable.
    pub brightness: Option<u32>,
    /// Maximum brightness, when readable.
    pub max_brightness: Option<u32>,
    /// Space-separated per-channel intensities for RGB LEDs (`multi_intensity`).
    pub multi_intensity: Option<String>,
}

/// Enumerate LED devices from sysfs, sorted by name.
pub fn list_leds() -> Vec<LedInfo> {
    list_leds_in(Path::new(LEDS_DIR))
}

fn list_leds_in(dir: &Path) -> Vec<LedInfo> {
    let mut leds = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return leds;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        leds.push(LedInfo {
            name,
            brightness: read_u32(&path.join("brightness")),
            max_brightness: read_u32(&path.join("max_brightness")),
            multi_intensity: read_trimmed(&path.join("multi_intensity")),
        });
    }

    leds.sort_by(|a, b| a.name.cmp(&b.name));
    leds
}

/// Set an LED's brightness, clamped to its `max_brightness`.
pub fn set_led_brightness(name: &str, brightness: u32) -> Result<()> {
    set_led_brightness_in(Path::new(LEDS_DIR), name, brightness)
}

fn set_led_brightness_in(dir: &Path, name: &str, brightness: u32) -> Result<()> {
    let led_dir = led_dir(dir, name)?;
    let max = read_u32(&led_dir.join("max_brightness"));
    let value = match max {
        Some(max) => brightness.min(max),
        None => brightness,
    };
    std::fs::write(led_dir.join("brightness"), value.to_string())
        .with_context(|| format!("failed to set brightness for LED {name}"))
}

/// Set an RGB LED's per-channel intensities via `multi_intensity`.
pub fn set_led_color(name: &str, channels: &[u32]) -> Result<()> {
    set_led_color_in(Path::new(LEDS_DIR), name, channels)
}

fn set_led_color_in(dir: &Path, name: &str, channels: &[u32]) -> Result<()> {
    if channels.is_empty() {
        bail!("LED color requires at least one channel intensity");
    }
    let led_dir = led_dir(dir, name)?;
    let multi = led_dir.join("multi_intensity");
    if !multi.exists() {
        bail!("LED {name} does not support multi_intensity color");
    }
    let value = channels
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    std::fs::write(&multi, value).with_context(|| format!("failed to set color for LED {name}"))
}

/// Resolve and validate an LED directory, rejecting path traversal in `name`.
fn led_dir(dir: &Path, name: &str) -> Result<std::path::PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains("..") {
        bail!("invalid LED name {name}");
    }
    let led_dir = dir.join(name);
    if !led_dir.is_dir() {
        bail!("no LED named {name}");
    }
    Ok(led_dir)
}

fn read_u32(path: &Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|value| value.trim().parse().ok())
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::{list_leds_in, set_led_brightness_in, set_led_color_in};

    fn make_led(dir: &std::path::Path, name: &str, brightness: &str, max: &str) {
        let led = dir.join(name);
        std::fs::create_dir_all(&led).unwrap();
        std::fs::write(led.join("brightness"), brightness).unwrap();
        std::fs::write(led.join("max_brightness"), max).unwrap();
    }

    #[test]
    fn lists_and_sets_led_brightness_clamped() {
        let dir = std::env::temp_dir().join(format!("padproxy-leds-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        make_led(&dir, "controller::player1", "0", "255");

        let leds = list_leds_in(&dir);
        assert_eq!(leds.len(), 1);
        assert_eq!(leds[0].name, "controller::player1");
        assert_eq!(leds[0].max_brightness, Some(255));

        // Over-max brightness is clamped to max.
        set_led_brightness_in(&dir, "controller::player1", 9000).unwrap();
        let written = std::fs::read_to_string(dir.join("controller::player1/brightness")).unwrap();
        assert_eq!(written, "255");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sets_rgb_multi_intensity_and_rejects_bad_names() {
        let dir = std::env::temp_dir().join(format!("padproxy-leds-rgb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let led = dir.join("rgb:indicator");
        std::fs::create_dir_all(&led).unwrap();
        std::fs::write(led.join("multi_intensity"), "0 0 0").unwrap();

        set_led_color_in(&dir, "rgb:indicator", &[10, 20, 30]).unwrap();
        let written = std::fs::read_to_string(led.join("multi_intensity")).unwrap();
        assert_eq!(written, "10 20 30");

        // Path traversal and missing LEDs are rejected.
        assert!(set_led_brightness_in(&dir, "../escape", 1).is_err());
        assert!(set_led_color_in(&dir, "rgb:indicator", &[]).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
