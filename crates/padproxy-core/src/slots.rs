use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

pub const SLOT_COUNT: u8 = 4;
pub const DEFAULT_SELECTED_SLOT: u8 = 1;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct SlotStore {
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceSlots>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceSlots {
    pub selected_slot: Option<u8>,
    #[serde(default)]
    pub slots: BTreeMap<u8, SlotAssignment>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SlotAssignment {
    pub profile_id: String,
}

impl SlotStore {
    pub fn assign_profile(&mut self, device_id: &str, slot: u8, profile_id: &str) -> Result<()> {
        validate_slot(slot)?;
        validate_device_id(device_id)?;
        if profile_id.trim().is_empty() {
            return Err(anyhow!("slot profile id cannot be empty"));
        }

        let device = self.devices.entry(device_id.to_string()).or_default();
        device.slots.insert(
            slot,
            SlotAssignment {
                profile_id: profile_id.to_string(),
            },
        );
        if device.selected_slot.is_none() {
            device.selected_slot = Some(slot);
        }
        Ok(())
    }

    pub fn select_slot(&mut self, device_id: &str, slot: u8) -> Result<()> {
        validate_slot(slot)?;
        validate_device_id(device_id)?;

        self.devices
            .entry(device_id.to_string())
            .or_default()
            .selected_slot = Some(slot);
        Ok(())
    }

    pub fn clear_slot(&mut self, device_id: &str, slot: u8) -> Result<()> {
        validate_slot(slot)?;
        validate_device_id(device_id)?;

        if let Some(device) = self.devices.get_mut(device_id) {
            device.slots.remove(&slot);
            if device.slots.is_empty() && device.selected_slot.is_none() {
                self.devices.remove(device_id);
            }
        }
        Ok(())
    }

    pub fn selected_slot(&self, device_id: &str) -> u8 {
        self.devices
            .get(device_id)
            .and_then(|device| device.selected_slot)
            .filter(|slot| validate_slot(*slot).is_ok())
            .unwrap_or(DEFAULT_SELECTED_SLOT)
    }

    pub fn profile_for_slot(&self, device_id: &str, slot: u8) -> Option<&str> {
        if validate_slot(slot).is_err() {
            return None;
        }

        self.devices
            .get(device_id)
            .and_then(|device| device.slots.get(&slot))
            .map(|assignment| assignment.profile_id.as_str())
    }
}

pub fn slot_store_path() -> PathBuf {
    if let Some(path) = std::env::var_os("PADPROXY_SLOT_FILE") {
        return PathBuf::from(path);
    }

    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("padproxy/slots.json");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/padproxy/slots.json");
    }

    PathBuf::from("slots.json")
}

pub fn load_slot_store() -> Result<SlotStore> {
    let path = slot_store_path();
    if !path.exists() {
        return Ok(SlotStore::default());
    }

    let bytes =
        fs::read(&path).with_context(|| format!("failed to read slot store {}", path.display()))?;
    if bytes.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(SlotStore::default());
    }

    serde_json::from_slice(&bytes)
        .with_context(|| format!("failed to parse slot store {}", path.display()))
}

pub fn save_slot_store(store: &SlotStore) -> Result<()> {
    let path = slot_store_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create slot store directory {}", parent.display())
        })?;
    }

    let bytes = serde_json::to_vec_pretty(store).context("failed to serialize slot store")?;
    fs::write(&path, bytes)
        .with_context(|| format!("failed to write slot store {}", path.display()))
}

pub fn validate_slot(slot: u8) -> Result<()> {
    if (1..=SLOT_COUNT).contains(&slot) {
        Ok(())
    } else {
        Err(anyhow!("slot must be between 1 and {SLOT_COUNT}"))
    }
}

fn validate_device_id(device_id: &str) -> Result<()> {
    if device_id.trim().is_empty() {
        Err(anyhow!("slot device id cannot be empty"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{SlotStore, DEFAULT_SELECTED_SLOT, SLOT_COUNT};

    #[test]
    fn assigns_selects_and_clears_device_slots() {
        let mut store = SlotStore::default();
        assert_eq!(store.selected_slot("device-1"), DEFAULT_SELECTED_SLOT);
        assert!(store.profile_for_slot("device-1", 1).is_none());

        store.assign_profile("device-1", 2, "nes").unwrap();
        assert_eq!(store.selected_slot("device-1"), 2);
        assert_eq!(store.profile_for_slot("device-1", 2), Some("nes"));

        store.select_slot("device-1", 4).unwrap();
        assert_eq!(store.selected_slot("device-1"), 4);

        store.clear_slot("device-1", 2).unwrap();
        assert!(store.profile_for_slot("device-1", 2).is_none());
        assert_eq!(store.selected_slot("device-1"), 4);
    }

    #[test]
    fn rejects_slots_outside_supported_range() {
        let mut store = SlotStore::default();
        assert!(store.assign_profile("device-1", 0, "nes").is_err());
        assert!(store.select_slot("device-1", SLOT_COUNT + 1).is_err());
        assert!(store.clear_slot("", 1).is_err());
    }

    #[test]
    fn serializes_slot_store_with_numeric_slot_keys() {
        let mut store = SlotStore::default();
        store.assign_profile("device-1", 1, "nes").unwrap();
        store.select_slot("device-1", 1).unwrap();

        let json = serde_json::to_string(&store).unwrap();
        let decoded: SlotStore = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.selected_slot("device-1"), 1);
        assert_eq!(decoded.profile_for_slot("device-1", 1), Some("nes"));
    }
}
