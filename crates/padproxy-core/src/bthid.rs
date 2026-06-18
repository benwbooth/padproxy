//! HID gamepad report core for Bluetooth controller emulation.
//!
//! Provides the standard gamepad HID report descriptor and the matching input
//! report encoding. This is the transport-independent core used to present
//! PadProxy's controller to another host over Bluetooth HID-over-GATT (see the
//! `bt-emulate` CLI command), and is equally usable for USB-gadget HID.

use crate::gimx::GamepadState;

/// Length of an encoded HID input report (16-bit buttons + 4×i16 axes + 2×u8
/// triggers).
pub const HID_REPORT_LEN: usize = 12;

/// HID report descriptor for a standard gamepad: 16 buttons, X/Y/Rx/Ry sticks
/// (16-bit signed), and Z/Rz triggers (8-bit). The input report layout matches
/// [`encode_hid_report`].
pub const HID_REPORT_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop)
    0x09, 0x05, // Usage (Game Pad)
    0xA1, 0x01, // Collection (Application)
    // 16 buttons -> 2 bytes
    0x05, 0x09, //   Usage Page (Button)
    0x19, 0x01, //   Usage Minimum (1)
    0x29, 0x10, //   Usage Maximum (16)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x10, //   Report Count (16)
    0x81, 0x02, //   Input (Data,Var,Abs)
    // 4 sticks (X, Y, Rx, Ry), 16-bit signed -> 8 bytes
    0x05, 0x01, //   Usage Page (Generic Desktop)
    0x09, 0x30, //   Usage (X)
    0x09, 0x31, //   Usage (Y)
    0x09, 0x33, //   Usage (Rx)
    0x09, 0x34, //   Usage (Ry)
    0x16, 0x00, 0x80, //   Logical Minimum (-32768)
    0x26, 0xFF, 0x7F, //   Logical Maximum (32767)
    0x75, 0x10, //   Report Size (16)
    0x95, 0x04, //   Report Count (4)
    0x81, 0x02, //   Input (Data,Var,Abs)
    // 2 triggers (Z, Rz), 8-bit unsigned -> 2 bytes
    0x09, 0x32, //   Usage (Z)
    0x09, 0x35, //   Usage (Rz)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xFF, 0x00, //   Logical Maximum (255)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x02, //   Report Count (2)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0xC0, // End Collection
];

/// Encode a gamepad state into a HID input report matching
/// [`HID_REPORT_DESCRIPTOR`].
pub fn encode_hid_report(state: &GamepadState) -> [u8; HID_REPORT_LEN] {
    let mut report = [0u8; HID_REPORT_LEN];
    report[0..2].copy_from_slice(&state.buttons.to_le_bytes());
    report[2..4].copy_from_slice(&state.left_x.to_le_bytes());
    report[4..6].copy_from_slice(&state.left_y.to_le_bytes());
    report[6..8].copy_from_slice(&state.right_x.to_le_bytes());
    report[8..10].copy_from_slice(&state.right_y.to_le_bytes());
    report[10] = state.left_trigger;
    report[11] = state.right_trigger;
    report
}

#[cfg(test)]
mod tests {
    use super::{encode_hid_report, HID_REPORT_DESCRIPTOR, HID_REPORT_LEN};
    use crate::gimx::{GamepadState, GimxButton};

    #[test]
    fn descriptor_is_well_formed() {
        // Starts with the Generic Desktop usage page and ends with End
        // Collection, and has a sane length.
        assert_eq!(&HID_REPORT_DESCRIPTOR[0..2], &[0x05, 0x01]);
        assert_eq!(*HID_REPORT_DESCRIPTOR.last().unwrap(), 0xC0);
        assert!(HID_REPORT_DESCRIPTOR.len() > 30);
    }

    #[test]
    fn encodes_report_in_descriptor_order() {
        let mut state = GamepadState {
            left_x: -1,
            right_y: 2,
            left_trigger: 200,
            right_trigger: 255,
            ..Default::default()
        };
        state.set_button(GimxButton::South, true);
        state.set_button(GimxButton::ThumbR, true);

        let report = encode_hid_report(&state);
        assert_eq!(report.len(), HID_REPORT_LEN);
        // Buttons first (South=bit0, ThumbR=bit12).
        assert_eq!(
            u16::from_le_bytes([report[0], report[1]]),
            (1 << 0) | (1 << 12)
        );
        // Then left stick X (-1), then triggers at the tail.
        assert_eq!(i16::from_le_bytes([report[2], report[3]]), -1);
        assert_eq!(report[10], 200);
        assert_eq!(report[11], 255);
    }
}
