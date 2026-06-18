//! GIMX-style wired external output.
//!
//! Streams a controller's state over a serial line to an external adapter (in
//! the style of a GIMX USB adapter, which relays controller state to a console).
//! PadProxy reads a source controller and writes fixed-size binary frames; an
//! adapter configured for the matching baud and frame layout drives the target.
//!
//! Frame layout (little-endian, 14 bytes):
//! - byte 0: start marker `0xA5`
//! - bytes 1..5: left stick X, Y as two `i16`
//! - bytes 5..9: right stick X, Y as two `i16`
//! - byte 9: left trigger `u8`
//! - byte 10: right trigger `u8`
//! - bytes 11..13: 16-bit button mask
//! - byte 13: additive checksum of bytes 0..13

use crate::event_code::event_from_input;
use serde::Serialize;

pub const FRAME_START: u8 = 0xA5;
pub const FRAME_LEN: usize = 14;

/// Standard gamepad buttons as bit positions in the frame's button mask.
#[derive(Clone, Copy, Debug)]
pub enum GimxButton {
    South = 0,
    East = 1,
    North = 2,
    West = 3,
    L1 = 4,
    R1 = 5,
    L2 = 6,
    R2 = 7,
    Select = 8,
    Start = 9,
    Mode = 10,
    ThumbL = 11,
    ThumbR = 12,
}

/// Current gamepad state to stream to the adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GamepadState {
    pub left_x: i16,
    pub left_y: i16,
    pub right_x: i16,
    pub right_y: i16,
    pub left_trigger: u8,
    pub right_trigger: u8,
    pub buttons: u16,
}

impl GamepadState {
    pub fn set_button(&mut self, button: GimxButton, pressed: bool) {
        let bit = 1u16 << (button as u16);
        if pressed {
            self.buttons |= bit;
        } else {
            self.buttons &= !bit;
        }
    }

    /// Encode the state as a fixed-size serial frame.
    pub fn encode_frame(&self) -> [u8; FRAME_LEN] {
        let mut frame = [0u8; FRAME_LEN];
        frame[0] = FRAME_START;
        frame[1..3].copy_from_slice(&self.left_x.to_le_bytes());
        frame[3..5].copy_from_slice(&self.left_y.to_le_bytes());
        frame[5..7].copy_from_slice(&self.right_x.to_le_bytes());
        frame[7..9].copy_from_slice(&self.right_y.to_le_bytes());
        frame[9] = self.left_trigger;
        frame[10] = self.right_trigger;
        frame[11..13].copy_from_slice(&self.buttons.to_le_bytes());
        // Simple checksum so the adapter can resync on a corrupt stream.
        frame[13] = frame[..13].iter().fold(0u8, |acc, b| acc.wrapping_add(*b));
        frame
    }

    /// Update the state from a source controller input event. Returns true if
    /// the state changed.
    pub fn apply_event(&mut self, event: &evdev::InputEvent) -> bool {
        let before = *self;
        let Some(code) = event_from_input(*event) else {
            return false;
        };
        let value = event.value();
        match code.name().as_str() {
            "btn:south" => self.set_button(GimxButton::South, value != 0),
            "btn:east" => self.set_button(GimxButton::East, value != 0),
            "btn:north" => self.set_button(GimxButton::North, value != 0),
            "btn:west" => self.set_button(GimxButton::West, value != 0),
            "btn:tl" => self.set_button(GimxButton::L1, value != 0),
            "btn:tr" => self.set_button(GimxButton::R1, value != 0),
            "btn:tl2" => self.set_button(GimxButton::L2, value != 0),
            "btn:tr2" => self.set_button(GimxButton::R2, value != 0),
            "btn:select" => self.set_button(GimxButton::Select, value != 0),
            "btn:start" => self.set_button(GimxButton::Start, value != 0),
            "btn:mode" => self.set_button(GimxButton::Mode, value != 0),
            "btn:thumbl" => self.set_button(GimxButton::ThumbL, value != 0),
            "btn:thumbr" => self.set_button(GimxButton::ThumbR, value != 0),
            "abs:x" => self.left_x = clamp_axis(value),
            "abs:y" => self.left_y = clamp_axis(value),
            "abs:rx" => self.right_x = clamp_axis(value),
            "abs:ry" => self.right_y = clamp_axis(value),
            "abs:z" => self.left_trigger = clamp_trigger(value),
            "abs:rz" => self.right_trigger = clamp_trigger(value),
            _ => {}
        }
        *self != before
    }
}

fn clamp_axis(value: i32) -> i16 {
    value.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

fn clamp_trigger(value: i32) -> u8 {
    value.clamp(0, 255) as u8
}

/// Stream a controller's state to a serial device as GIMX-style frames until
/// interrupted. The serial device should already be configured for the adapter
/// (e.g. baud) by the OS or `stty`.
pub fn run_serial_output(controller_path: &str, serial_path: &str) -> anyhow::Result<()> {
    use anyhow::Context;
    use evdev::Device;
    use std::io::Write;

    let mut source = Device::open(controller_path)
        .with_context(|| format!("failed to open controller {controller_path}"))?;
    let mut serial = std::fs::OpenOptions::new()
        .write(true)
        .open(serial_path)
        .with_context(|| format!("failed to open serial device {serial_path}"))?;

    eprintln!("PadProxy streaming GIMX-style frames from {controller_path} to {serial_path}. Press Ctrl-C to stop.");

    let mut state = GamepadState::default();
    // Send an initial frame so the adapter has a baseline.
    serial.write_all(&state.encode_frame())?;

    loop {
        for event in source
            .fetch_events()
            .context("failed reading controller events")?
        {
            if state.apply_event(&event) {
                serial
                    .write_all(&state.encode_frame())
                    .context("failed writing to serial device")?;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GamepadState, GimxButton, FRAME_LEN, FRAME_START};

    #[test]
    fn encodes_frame_with_marker_and_checksum() {
        let mut state = GamepadState {
            left_x: -32768,
            right_y: 1000,
            left_trigger: 255,
            ..Default::default()
        };
        state.set_button(GimxButton::South, true);
        state.set_button(GimxButton::Start, true);

        let frame = state.encode_frame();
        assert_eq!(frame.len(), FRAME_LEN);
        assert_eq!(frame[0], FRAME_START);
        // Left stick X is i16::MIN little-endian.
        assert_eq!(&frame[1..3], &(-32768i16).to_le_bytes());
        // Button mask has South (bit 0) and Start (bit 9).
        let mask = u16::from_le_bytes([frame[11], frame[12]]);
        assert_eq!(mask, (1 << 0) | (1 << 9));
        // Checksum matches.
        let expected = frame[..13].iter().fold(0u8, |a, b| a.wrapping_add(*b));
        assert_eq!(frame[13], expected);
    }

    #[test]
    fn set_button_toggles_bits() {
        let mut state = GamepadState::default();
        state.set_button(GimxButton::R2, true);
        assert_eq!(state.buttons, 1 << 7);
        state.set_button(GimxButton::R2, false);
        assert_eq!(state.buttons, 0);
    }
}
