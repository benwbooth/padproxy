use crate::devices::DeviceInfo;
use anyhow::Result;
use evdev::{
    enumerate, AbsoluteAxisCode, AttributeSetRef, Device, EventType, KeyCode, RelativeAxisCode,
};
use std::path::{Path, PathBuf};

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    let mut devices = Vec::new();

    for (path, device) in enumerate() {
        let input_id = device.input_id();
        let name = device.name().unwrap_or("Unknown input device").to_string();
        let device_kind = device_kind(&path, &device);

        if !should_show_device(&path, &device, &name, &device_kind) {
            continue;
        }

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
            device_kind,
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
        .ok_or_else(|| anyhow::anyhow!("no input device matched {selector}"))
}

/// Resolve a selector (device id, name, or path) to full device info. An
/// absolute path that matches no known device is accepted as a raw path device.
pub fn resolve_device_info(selector: &str) -> Result<DeviceInfo> {
    if let Some(device) = list_devices()?
        .into_iter()
        .find(|device| device.id == selector || device.name == selector || device.path == selector)
    {
        return Ok(device);
    }

    if Path::new(selector).is_absolute() {
        return Ok(DeviceInfo {
            id: format!("path:{selector}"),
            name: selector.to_string(),
            path: selector.to_string(),
            device_kind: "path".to_string(),
            phys: String::new(),
            uniq: String::new(),
            bus: 0,
            vendor: 0,
            product: 0,
            version: 0,
            capabilities: Vec::new(),
        });
    }

    Err(anyhow::anyhow!("no input device matched {selector}"))
}

fn should_show_device(path: &Path, device: &Device, name: &str, device_kind: &str) -> bool {
    name_has_controller_hint(name)
        || name_has_motion_sensor_hint(name)
        || has_joystick_devlink(path)
        || looks_like_keyboard(device)
        || looks_like_mouse(device)
        || looks_like_motion_sensor(device)
        || (device_kind == "virtual" && looks_like_controller(device))
}

fn name_has_motion_sensor_hint(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "motion sensor",
        "motion sensors",
        "gyro",
        "accelerometer",
        "imu",
    ]
    .iter()
    .any(|hint| name.contains(hint))
}

/// A standalone motion sensor exposes gyro/accelerometer rotation axes
/// (`ABS_RX/RY/RZ`) without controller face buttons, so it can be grouped with a
/// controller and mapped (e.g. gyro-to-mouse).
fn looks_like_motion_sensor(device: &Device) -> bool {
    let Some(abs) = device.supported_absolute_axes() else {
        return false;
    };
    let has_rotation = abs.contains(AbsoluteAxisCode::ABS_RX)
        && abs.contains(AbsoluteAxisCode::ABS_RY)
        && abs.contains(AbsoluteAxisCode::ABS_RZ);
    let has_face_button = device
        .supported_keys()
        .map(|keys| {
            [
                KeyCode::BTN_SOUTH,
                KeyCode::BTN_EAST,
                KeyCode::BTN_NORTH,
                KeyCode::BTN_WEST,
            ]
            .iter()
            .any(|code| keys.contains(*code))
        })
        .unwrap_or(false);

    has_rotation && !has_face_button
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

fn looks_like_keyboard(device: &Device) -> bool {
    device
        .supported_keys()
        .map(|keys| key_set_has_keyboard_keys(keys))
        .unwrap_or(false)
}

fn looks_like_mouse(device: &Device) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    let Some(relative_axes) = device.supported_relative_axes() else {
        return false;
    };

    key_set_has_mouse_buttons(keys) && relative_set_has_mouse_motion(relative_axes)
}

fn name_has_controller_hint(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    // Names that, on their own, identify a controller family.
    let direct_match = [
        "gamepad",
        "joystick",
        "joypad",
        "joy-con",
        "joycon",
        "joy con",
        "x-box",
        "xbox",
        "xinput",
        "dualshock",
        "dualsense",
        "playstation",
        "sony interactive",
        "steam controller",
        "steam deck",
        "steam virtual",
        "nintendo",
        "switch pro",
        "gamecube",
        "gc controller",
        "stadia",
        "shield",
        "8bitdo",
        "horipad",
        "hori ",
        "powera",
        "azeron",
        "elite",
        "navigation",
    ]
    .iter()
    .any(|hint| name.contains(hint));

    direct_match
        || (name.contains("controller")
            && [
                "8bitdo",
                "dual",
                "game",
                "nintendo",
                "playstation",
                "pro",
                "sony",
                "steam",
                "switch",
                "wireless",
                "x-box",
                "xbox",
                "xinput",
            ]
            .iter()
            .any(|hint| name.contains(hint)))
}

fn has_joystick_devlink(path: &Path) -> bool {
    let Ok(target) = std::fs::canonicalize(path) else {
        return false;
    };

    for dir in ["/dev/input/by-id", "/dev/input/by-path"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if !file_name.contains("event-joystick") {
                continue;
            }
            if std::fs::canonicalize(entry.path())
                .map(|link_target| link_target == target)
                .unwrap_or(false)
            {
                return true;
            }
        }
    }

    false
}

fn device_kind(path: &Path, device: &Device) -> String {
    let name = device.name().unwrap_or_default().to_ascii_lowercase();
    let phys = device
        .physical_path()
        .unwrap_or_default()
        .to_ascii_lowercase();

    if sysfs_device_path(path)
        .and_then(|sysfs_path| std::fs::canonicalize(sysfs_path).ok())
        .map(|sysfs_path| sysfs_path.starts_with("/sys/devices/virtual/input"))
        .unwrap_or(false)
        || name.contains("virtual")
        || name.contains("padproxy")
        || phys.contains("uinput")
    {
        "virtual".to_string()
    } else if device.input_id().bus_type().0 == 0x06 {
        "virtual".to_string()
    } else {
        "physical".to_string()
    }
}

fn sysfs_device_path(path: &Path) -> Option<PathBuf> {
    let event_name = path.file_name()?;
    Some(
        Path::new("/sys/class/input")
            .join(event_name)
            .join("device"),
    )
}

fn capabilities(device: &Device) -> Vec<String> {
    let mut values = Vec::new();

    if let Some(keys) = device.supported_keys() {
        if key_set_has_keyboard_keys(keys) {
            values.push("KEYBOARD".to_string());
        }
        if key_set_has_mouse_buttons(keys) {
            values.push("MOUSE_BUTTONS".to_string());
        }
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
                KeyCode::KEY_SPACE => "KEY_SPACE",
                KeyCode::KEY_ENTER => "KEY_ENTER",
                KeyCode::KEY_ESC => "KEY_ESC",
                KeyCode::KEY_TAB => "KEY_TAB",
                KeyCode::KEY_LEFTCTRL => "KEY_LEFTCTRL",
                KeyCode::KEY_LEFTSHIFT => "KEY_LEFTSHIFT",
                KeyCode::KEY_LEFTALT => "KEY_LEFTALT",
                KeyCode::KEY_A => "KEY_A",
                KeyCode::KEY_S => "KEY_S",
                KeyCode::KEY_D => "KEY_D",
                KeyCode::KEY_W => "KEY_W",
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

    if let Some(relative_axes) = device.supported_relative_axes() {
        if relative_set_has_mouse_motion(relative_axes) {
            values.push("MOUSE_MOTION".to_string());
        }
        for axis in relative_axes.iter() {
            let name = match axis {
                RelativeAxisCode::REL_X => "REL_X",
                RelativeAxisCode::REL_Y => "REL_Y",
                RelativeAxisCode::REL_WHEEL => "REL_WHEEL",
                RelativeAxisCode::REL_HWHEEL => "REL_HWHEEL",
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

fn key_set_has_keyboard_keys(keys: &AttributeSetRef<KeyCode>) -> bool {
    let has_letter_cluster = [
        KeyCode::KEY_A,
        KeyCode::KEY_S,
        KeyCode::KEY_D,
        KeyCode::KEY_W,
    ]
    .iter()
    .filter(|code| keys.contains(**code))
    .count()
        >= 2;
    let has_control_key = [
        KeyCode::KEY_SPACE,
        KeyCode::KEY_ENTER,
        KeyCode::KEY_ESC,
        KeyCode::KEY_TAB,
        KeyCode::KEY_LEFTCTRL,
        KeyCode::KEY_LEFTSHIFT,
        KeyCode::KEY_LEFTALT,
    ]
    .iter()
    .any(|code| keys.contains(*code));

    has_letter_cluster && has_control_key
}

fn key_set_has_mouse_buttons(keys: &AttributeSetRef<KeyCode>) -> bool {
    keys.contains(KeyCode::BTN_LEFT) || keys.contains(KeyCode::BTN_RIGHT)
}

fn relative_set_has_mouse_motion(relative_axes: &AttributeSetRef<RelativeAxisCode>) -> bool {
    relative_axes.contains(RelativeAxisCode::REL_X)
        && relative_axes.contains(RelativeAxisCode::REL_Y)
}

#[cfg(test)]
mod tests {
    use super::{
        key_set_has_keyboard_keys, key_set_has_mouse_buttons, name_has_controller_hint,
        name_has_motion_sensor_hint, relative_set_has_mouse_motion,
    };
    use evdev::{AttributeSet, KeyCode, RelativeAxisCode};

    #[test]
    fn recognizes_motion_sensor_names() {
        assert!(name_has_motion_sensor_hint(
            "Sony Interactive Entertainment DualSense Wireless Controller Motion Sensors"
        ));
        assert!(name_has_motion_sensor_hint(
            "Nintendo Switch Pro Controller IMU"
        ));
        assert!(!name_has_motion_sensor_hint("USB Keyboard"));
    }

    #[test]
    fn recognizes_controller_families_by_name() {
        for name in [
            "Microsoft X-Box 360 pad",
            "Sony Interactive Entertainment DualSense Wireless Controller",
            "Nintendo Switch Pro Controller",
            "Joy-Con (L)",
            "Steam Deck Controller",
            "Valve Steam Controller",
            "Google Stadia Controller",
            "NVIDIA Shield Controller",
            "8BitDo SN30 Pro",
            "Mayflash GameCube Controller Adapter",
        ] {
            assert!(
                name_has_controller_hint(name),
                "should recognize controller: {name}"
            );
        }

        for name in ["USB Webcam", "AT Translated Set 2 keyboard", "Power Button"] {
            assert!(
                !name_has_controller_hint(name),
                "should not flag non-controller: {name}"
            );
        }
    }

    #[test]
    fn recognizes_real_keyboard_key_sets() {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::KEY_A);
        keys.insert(KeyCode::KEY_S);
        keys.insert(KeyCode::KEY_SPACE);

        assert!(key_set_has_keyboard_keys(&keys));

        let mut power_keys = AttributeSet::<KeyCode>::new();
        power_keys.insert(KeyCode::KEY_POWER);
        power_keys.insert(KeyCode::KEY_SLEEP);
        assert!(!key_set_has_keyboard_keys(&power_keys));
    }

    #[test]
    fn recognizes_mouse_buttons_and_motion() {
        let mut keys = AttributeSet::<KeyCode>::new();
        keys.insert(KeyCode::BTN_LEFT);

        let mut relative_axes = AttributeSet::<RelativeAxisCode>::new();
        relative_axes.insert(RelativeAxisCode::REL_X);
        relative_axes.insert(RelativeAxisCode::REL_Y);

        assert!(key_set_has_mouse_buttons(&keys));
        assert!(relative_set_has_mouse_motion(&relative_axes));

        let mut wheel_only = AttributeSet::<RelativeAxisCode>::new();
        wheel_only.insert(RelativeAxisCode::REL_WHEEL);
        assert!(!relative_set_has_mouse_motion(&wheel_only));
    }
}
