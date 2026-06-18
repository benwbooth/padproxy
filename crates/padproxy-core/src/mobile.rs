//! Mobile controller server.
//!
//! A phone or tablet app can act as a controller by sending small JSON state
//! packets over UDP. PadProxy translates each packet into a virtual gamepad,
//! mirroring reWASD's mobile-controller feature. The wire format is one JSON
//! object per datagram:
//!
//! ```json
//! {"buttons": {"a": true, "up": false}, "axes": {"lx": 0.5, "rt": 1.0}}
//! ```
//!
//! Buttons are face/shoulder/stick names plus d-pad directions; stick axes use a
//! -1.0..1.0 range and trigger axes use 0.0..1.0.

use crate::remapper::build_virtual_gamepad;
use anyhow::{Context, Result};
use evdev::{AbsoluteAxisCode, AttributeSet, EventType, InputEvent, KeyCode, RelativeAxisCode};
use serde::Deserialize;
use std::collections::HashMap;
use std::net::UdpSocket;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MobileState {
    #[serde(default)]
    pub buttons: HashMap<String, bool>,
    #[serde(default)]
    pub axes: HashMap<String, f32>,
}

/// Parse one UDP datagram into a controller state.
pub fn parse_packet(bytes: &[u8]) -> Result<MobileState> {
    serde_json::from_slice(bytes).context("invalid mobile controller packet")
}

/// Map a button name to its virtual gamepad key, if it is a face/shoulder/stick
/// button (d-pad directions are handled as hat axes separately).
fn button_key(name: &str) -> Option<KeyCode> {
    Some(match name {
        "a" => KeyCode::BTN_SOUTH,
        "b" => KeyCode::BTN_EAST,
        "x" => KeyCode::BTN_NORTH,
        "y" => KeyCode::BTN_WEST,
        "lb" | "l1" => KeyCode::BTN_TL,
        "rb" | "r1" => KeyCode::BTN_TR,
        "back" | "select" => KeyCode::BTN_SELECT,
        "start" => KeyCode::BTN_START,
        "guide" | "home" => KeyCode::BTN_MODE,
        "ls" | "l3" => KeyCode::BTN_THUMBL,
        "rs" | "r3" => KeyCode::BTN_THUMBR,
        _ => return None,
    })
}

/// Map a stick/trigger axis name to its absolute axis and value range.
fn axis_setup(name: &str) -> Option<(AbsoluteAxisCode, i32, i32)> {
    Some(match name {
        "lx" => (AbsoluteAxisCode::ABS_X, -32768, 32767),
        "ly" => (AbsoluteAxisCode::ABS_Y, -32768, 32767),
        "rx" => (AbsoluteAxisCode::ABS_RX, -32768, 32767),
        "ry" => (AbsoluteAxisCode::ABS_RY, -32768, 32767),
        "lt" | "l2" => (AbsoluteAxisCode::ABS_Z, 0, 255),
        "rt" | "r2" => (AbsoluteAxisCode::ABS_RZ, 0, 255),
        _ => return None,
    })
}

fn scale_axis(unit: f32, min: i32, max: i32) -> i32 {
    if min < 0 {
        (unit.clamp(-1.0, 1.0) * max as f32).round() as i32
    } else {
        (unit.clamp(0.0, 1.0) * max as f32).round() as i32
    }
}

/// Translate a controller state snapshot into virtual gamepad events.
pub fn events_for_state(state: &MobileState) -> Vec<InputEvent> {
    let mut events = Vec::new();

    for (name, pressed) in &state.buttons {
        let value = i32::from(*pressed);
        if let Some(key) = button_key(name) {
            events.push(InputEvent::new(EventType::KEY.0, key.0, value));
        }
    }

    // D-pad directions drive the hat axes; opposing presses cancel.
    let dpad = |name: &str| state.buttons.get(name).copied().unwrap_or(false);
    let hat_x = i32::from(dpad("right")) - i32::from(dpad("left"));
    let hat_y = i32::from(dpad("down")) - i32::from(dpad("up"));
    events.push(InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_HAT0X.0,
        hat_x,
    ));
    events.push(InputEvent::new(
        EventType::ABSOLUTE.0,
        AbsoluteAxisCode::ABS_HAT0Y.0,
        hat_y,
    ));

    for (name, unit) in &state.axes {
        if let Some((axis, min, max)) = axis_setup(name) {
            events.push(InputEvent::new(
                EventType::ABSOLUTE.0,
                axis.0,
                scale_axis(*unit, min, max),
            ));
        }
    }

    events
}

/// Run the mobile controller server: bind a UDP socket, create a virtual
/// gamepad, and replay incoming controller states onto it until interrupted.
pub fn run_server(port: u16, output_type: &str) -> Result<()> {
    let socket = UdpSocket::bind(("0.0.0.0", port))
        .with_context(|| format!("failed to bind UDP port {port}"))?;
    let mut pad = build_virtual_gamepad(
        output_type,
        &AttributeSet::<KeyCode>::new(),
        &AttributeSet::<RelativeAxisCode>::new(),
        false,
    )?;
    if let Ok(nodes) = pad.enumerate_dev_nodes_blocking() {
        for node in nodes.flatten() {
            eprintln!("Mobile controller virtual pad: {}", node.display());
        }
    }
    eprintln!("PadProxy mobile controller server listening on UDP {port}. Press Ctrl-C to stop.");

    let mut buf = [0u8; 4096];
    loop {
        let (len, _addr) = socket
            .recv_from(&mut buf)
            .context("failed to receive packet")?;
        match parse_packet(&buf[..len]) {
            Ok(state) => {
                let events = events_for_state(&state);
                if let Err(error) = pad.emit(&events) {
                    eprintln!("Mobile controller emit failed: {error}");
                }
            }
            Err(error) => eprintln!("Ignoring invalid mobile packet: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{events_for_state, parse_packet, scale_axis};
    use evdev::{AbsoluteAxisCode, EventType, KeyCode};

    #[test]
    fn scales_stick_and_trigger_axes() {
        assert_eq!(scale_axis(1.0, -32768, 32767), 32767);
        assert_eq!(scale_axis(-1.0, -32768, 32767), -32767);
        assert_eq!(scale_axis(0.0, -32768, 32767), 0);
        assert_eq!(scale_axis(1.0, 0, 255), 255);
        // Trigger values never go negative.
        assert_eq!(scale_axis(-0.5, 0, 255), 0);
    }

    #[test]
    fn parses_packet_and_builds_events() {
        let state =
            parse_packet(br#"{"buttons":{"a":true,"right":true},"axes":{"lx":1.0,"rt":1.0}}"#)
                .unwrap();
        let events = events_for_state(&state);

        // BTN_SOUTH pressed.
        assert!(events.iter().any(|e| e.event_type().0 == EventType::KEY.0
            && e.code() == KeyCode::BTN_SOUTH.0
            && e.value() == 1));
        // D-pad right -> ABS_HAT0X = +1.
        assert!(events
            .iter()
            .any(|e| e.event_type().0 == EventType::ABSOLUTE.0
                && e.code() == AbsoluteAxisCode::ABS_HAT0X.0
                && e.value() == 1));
        // Left stick X at full right.
        assert!(events
            .iter()
            .any(|e| e.code() == AbsoluteAxisCode::ABS_X.0 && e.value() == 32767));
        // Right trigger fully pressed.
        assert!(events
            .iter()
            .any(|e| e.code() == AbsoluteAxisCode::ABS_RZ.0 && e.value() == 255));
    }

    #[test]
    fn rejects_invalid_packet() {
        assert!(parse_packet(b"not json").is_err());
    }
}
