//! Battery and wireless status for controllers and other input peripherals.
//!
//! Linux exposes per-device batteries under `/sys/class/power_supply`. Wireless
//! controllers (and other Bluetooth/USB peripherals) report a `scope=Device`
//! power supply with capacity and charge status, which PadProxy surfaces so the
//! UI and CLI can show battery and wireless state where available.

use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

const POWER_SUPPLY_DIR: &str = "/sys/class/power_supply";

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BatteryInfo {
    /// Power-supply node name, e.g. `hid-<id>-battery`.
    pub name: String,
    /// Human-readable model name, when the device reports one.
    pub model: Option<String>,
    /// Charging status, e.g. `Charging`, `Discharging`, `Full`, `Unknown`.
    pub status: Option<String>,
    /// Remaining charge as a percentage, when reported.
    pub capacity: Option<u8>,
    /// Coarse charge level, e.g. `Low`, `Normal`, `High`, `Full`.
    pub capacity_level: Option<String>,
    /// Whether the device is currently present/connected.
    pub present: Option<bool>,
    /// Whether the supply is online (powered/linked).
    pub online: Option<bool>,
}

/// Parse a `uevent` file body from a `/sys/class/power_supply/*` node.
///
/// Returns `None` for supplies that are not a device-scoped battery (for
/// example a laptop's system battery or a mains adapter), so the result only
/// includes controller-style peripheral batteries.
pub fn parse_power_supply_uevent(name: &str, contents: &str) -> Option<BatteryInfo> {
    let mut fields: HashMap<&str, &str> = HashMap::new();
    for line in contents.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once('=') {
            if let Some(key) = key.trim().strip_prefix("POWER_SUPPLY_") {
                fields.insert(key, value.trim());
            }
        }
    }

    if !fields
        .get("TYPE")
        .map(|value| value.eq_ignore_ascii_case("Battery"))
        .unwrap_or(false)
    {
        return None;
    }

    if !fields
        .get("SCOPE")
        .map(|value| value.eq_ignore_ascii_case("Device"))
        .unwrap_or(false)
    {
        return None;
    }

    let name = fields
        .get("NAME")
        .map(|value| value.to_string())
        .unwrap_or_else(|| name.to_string());

    Some(BatteryInfo {
        name,
        model: non_empty(fields.get("MODEL_NAME").copied()),
        status: non_empty(fields.get("STATUS").copied()),
        capacity: fields.get("CAPACITY").and_then(|value| value.parse().ok()),
        capacity_level: non_empty(fields.get("CAPACITY_LEVEL").copied()),
        present: parse_bool(fields.get("PRESENT").copied()),
        online: parse_bool(fields.get("ONLINE").copied()),
    })
}

/// Enumerate device-scoped peripheral batteries from sysfs.
pub fn list_batteries() -> Vec<BatteryInfo> {
    list_batteries_in(Path::new(POWER_SUPPLY_DIR))
}

fn list_batteries_in(dir: &Path) -> Vec<BatteryInfo> {
    let mut batteries = Vec::new();

    let Ok(entries) = std::fs::read_dir(dir) else {
        return batteries;
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let uevent_path = entry.path().join("uevent");
        let Ok(contents) = std::fs::read_to_string(&uevent_path) else {
            continue;
        };
        if let Some(info) = parse_power_supply_uevent(&name, &contents) {
            batteries.push(info);
        }
    }

    batteries.sort_by(|a, b| a.name.cmp(&b.name));
    batteries
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn parse_bool(value: Option<&str>) -> Option<bool> {
    match value.map(str::trim) {
        Some("1") => Some(true),
        Some("0") => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_power_supply_uevent, BatteryInfo};

    #[test]
    fn parses_device_scoped_controller_battery() {
        let uevent = "\
DEVTYPE=power_supply
POWER_SUPPLY_NAME=hid-001122-battery
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_STATUS=Discharging
POWER_SUPPLY_PRESENT=1
POWER_SUPPLY_ONLINE=1
POWER_SUPPLY_CAPACITY=72
POWER_SUPPLY_SCOPE=Device
POWER_SUPPLY_MODEL_NAME=Wireless Controller
";
        let info = parse_power_supply_uevent("hid-001122-battery", uevent).unwrap();
        assert_eq!(
            info,
            BatteryInfo {
                name: "hid-001122-battery".to_string(),
                model: Some("Wireless Controller".to_string()),
                status: Some("Discharging".to_string()),
                capacity: Some(72),
                capacity_level: None,
                present: Some(true),
                online: Some(true),
            }
        );
    }

    #[test]
    fn ignores_system_battery_and_mains_adapter() {
        let laptop = "\
POWER_SUPPLY_NAME=BAT0
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_SCOPE=System
POWER_SUPPLY_CAPACITY=55
";
        assert!(parse_power_supply_uevent("BAT0", laptop).is_none());

        let mains = "\
POWER_SUPPLY_NAME=AC
POWER_SUPPLY_TYPE=Mains
POWER_SUPPLY_ONLINE=1
";
        assert!(parse_power_supply_uevent("AC", mains).is_none());
    }

    #[test]
    fn falls_back_to_node_name_and_capacity_level() {
        let uevent = "\
POWER_SUPPLY_TYPE=Battery
POWER_SUPPLY_SCOPE=Device
POWER_SUPPLY_CAPACITY_LEVEL=Low
";
        let info = parse_power_supply_uevent("hid-aabb-battery", uevent).unwrap();
        assert_eq!(info.name, "hid-aabb-battery");
        assert_eq!(info.capacity, None);
        assert_eq!(info.capacity_level.as_deref(), Some("Low"));
        assert_eq!(info.model, None);
    }
}
