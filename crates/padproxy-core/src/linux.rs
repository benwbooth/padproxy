use crate::devices::DeviceInfo;
use anyhow::Result;
use evdev::{enumerate, AbsoluteAxisCode, Device, EventType, KeyCode};
use std::path::Path;

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let mut devices = Vec::new();

    for (path, device) in enumerate() {
        if !looks_like_controller(&device) {
            continue;
        }

        let input_id = device.input_id();
        let name = device.name().unwrap_or("Unknown controller").to_string();
        let phys = device.physical_path().unwrap_or("").to_string();
        let uniq = device.unique_name().unwrap_or("").to_string();
        let vendor = input_id.vendor();
        let product = input_id.product();
        let id = if uniq.is_empty() {
            format!("linux:{vendor:04x}:{product:04x}:{name}:{}", path.display())
        } else {
            format!("linux:{vendor:04x}:{product:04x}:{uniq}")
        };

        devices.push(DeviceInfo {
            id,
            name,
            path: path.to_string_lossy().to_string(),
            phys,
            uniq,
            bus: input_id.bus_type().0,
            vendor,
            product,
            version: input_id.version(),
            capabilities: capabilities(&device),
        });
    }

    Ok(devices)
}

pub fn resolve_device(selector: &str) -> Result<String> {
    if Path::new(selector).is_absolute() {
        return Ok(selector.to_string());
    }

    list_devices()?
        .into_iter()
        .find(|device| device.id == selector || device.name == selector || device.path == selector)
        .map(|device| device.path)
        .ok_or_else(|| anyhow::anyhow!("no controller matched {selector}"))
}

fn looks_like_controller(device: &Device) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    let Some(abs) = device.supported_absolute_axes() else {
        return false;
    };

    let has_face_button = [
        KeyCode::BTN_SOUTH,
        KeyCode::BTN_EAST,
        KeyCode::BTN_NORTH,
        KeyCode::BTN_WEST,
    ]
    .iter()
    .any(|code| keys.contains(*code));

    let has_xy = abs.contains(AbsoluteAxisCode::ABS_X) && abs.contains(AbsoluteAxisCode::ABS_Y);
    has_face_button && has_xy
}

fn capabilities(device: &Device) -> Vec<String> {
    let mut values = Vec::new();

    if let Some(keys) = device.supported_keys() {
        for key in keys.iter() {
            let name = match key {
                KeyCode::BTN_SOUTH => "BTN_SOUTH",
                KeyCode::BTN_EAST => "BTN_EAST",
                KeyCode::BTN_NORTH => "BTN_NORTH",
                KeyCode::BTN_WEST => "BTN_WEST",
                KeyCode::BTN_TL => "BTN_TL",
                KeyCode::BTN_TR => "BTN_TR",
                KeyCode::BTN_TL2 => "BTN_TL2",
                KeyCode::BTN_TR2 => "BTN_TR2",
                KeyCode::BTN_SELECT => "BTN_SELECT",
                KeyCode::BTN_START => "BTN_START",
                KeyCode::BTN_MODE => "BTN_MODE",
                KeyCode::BTN_THUMBL => "BTN_THUMBL",
                KeyCode::BTN_THUMBR => "BTN_THUMBR",
                _ => continue,
            };
            values.push(name.to_string());
        }
    }

    if let Some(abs) = device.supported_absolute_axes() {
        for axis in abs.iter() {
            let name = match axis {
                AbsoluteAxisCode::ABS_X => "ABS_X",
                AbsoluteAxisCode::ABS_Y => "ABS_Y",
                AbsoluteAxisCode::ABS_RX => "ABS_RX",
                AbsoluteAxisCode::ABS_RY => "ABS_RY",
                AbsoluteAxisCode::ABS_Z => "ABS_Z",
                AbsoluteAxisCode::ABS_RZ => "ABS_RZ",
                AbsoluteAxisCode::ABS_HAT0X => "ABS_HAT0X",
                AbsoluteAxisCode::ABS_HAT0Y => "ABS_HAT0Y",
                _ => continue,
            };
            values.push(name.to_string());
        }
    }

    if device.supported_events().contains(EventType::FORCEFEEDBACK) {
        values.push("EV_FF".to_string());
    }

    values
}
