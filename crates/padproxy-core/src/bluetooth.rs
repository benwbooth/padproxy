//! Wireless controller power-off via Bluetooth disconnect.
//!
//! A connected Bluetooth controller exposes its Bluetooth address (MAC) as the
//! evdev device's `uniq` value. PadProxy powers the controller off by asking
//! BlueZ (through `bluetoothctl`) to disconnect that address, mirroring reWASD's
//! wireless power-off command.

use crate::devices::DeviceInfo;
use anyhow::{bail, Context, Result};
use std::process::Command;

/// Returns true if `value` looks like a Bluetooth MAC address (`AA:BB:..:FF`).
pub fn is_mac_address(value: &str) -> bool {
    let parts: Vec<&str> = value.split(':').collect();
    parts.len() == 6
        && parts
            .iter()
            .all(|part| part.len() == 2 && part.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Find the Bluetooth address for a device from its `uniq` or `phys` fields.
pub fn bluetooth_address(device: &DeviceInfo) -> Option<String> {
    if is_mac_address(&device.uniq) {
        return Some(device.uniq.to_ascii_uppercase());
    }
    // Some drivers report the device address as the second half of `phys`
    // (`hostmac/devicemac`).
    if let Some((_, candidate)) = device.phys.split_once('/') {
        if is_mac_address(candidate) {
            return Some(candidate.to_ascii_uppercase());
        }
    }
    None
}

/// Power off (disconnect) a wireless controller via BlueZ.
pub fn power_off(device: &DeviceInfo) -> Result<()> {
    let address = bluetooth_address(device).ok_or_else(|| {
        anyhow::anyhow!(
            "no Bluetooth address for {}; power-off needs a wireless controller",
            device.name
        )
    })?;

    let status = Command::new("bluetoothctl")
        .args(["disconnect", &address])
        .status()
        .context("failed to run bluetoothctl (is BlueZ installed?)")?;

    if !status.success() {
        bail!("bluetoothctl could not disconnect {address}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{bluetooth_address, is_mac_address};
    use crate::devices::DeviceInfo;

    fn device(uniq: &str, phys: &str) -> DeviceInfo {
        DeviceInfo {
            id: "id".to_string(),
            name: "Wireless Controller".to_string(),
            path: "/dev/input/event0".to_string(),
            device_kind: "physical".to_string(),
            phys: phys.to_string(),
            uniq: uniq.to_string(),
            bus: 0,
            vendor: 0,
            product: 0,
            version: 0,
            capabilities: Vec::new(),
        }
    }

    #[test]
    fn recognizes_mac_addresses() {
        assert!(is_mac_address("a4:c1:38:9f:00:11"));
        assert!(is_mac_address("A4:C1:38:9F:00:11"));
        assert!(!is_mac_address("not-a-mac"));
        assert!(!is_mac_address("a4:c1:38:9f:00")); // too short
        assert!(!is_mac_address("a4:c1:38:9f:00:zz")); // non-hex
    }

    #[test]
    fn resolves_address_from_uniq_or_phys() {
        assert_eq!(
            bluetooth_address(&device("a4:c1:38:9f:00:11", "")),
            Some("A4:C1:38:9F:00:11".to_string())
        );
        assert_eq!(
            bluetooth_address(&device("", "00:1a:7d:da:71:13/a4:c1:38:9f:00:11")),
            Some("A4:C1:38:9F:00:11".to_string())
        );
        assert_eq!(bluetooth_address(&device("usb-0000:00", "usb-phys")), None);
    }
}
