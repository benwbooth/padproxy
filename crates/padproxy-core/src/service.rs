//! Local control API for PadProxy.
//!
//! [`ServiceState`] handles JSON [`Request`]s and returns JSON [`Response`]s,
//! exposing slot management, status queries, and live remap apply/off. The CLI
//! `serve` command wraps this in a Unix-socket loop so other tools (and the GUI)
//! can drive PadProxy over a local API.

use crate::autodetect::detect_profile;
use crate::linux::{list_devices, resolve_device_info};
use crate::power::list_batteries;
use crate::profiles::{default_profile_dirs, load_profiles};
use crate::session::{RemapMessage, RemapSession};
use crate::slots::{
    load_slot_store_from, save_slot_store_to, slot_store_path, validate_slot, SlotStore, SLOT_COUNT,
};
use crate::{log_info, log_warn};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

/// A control request. JSON is internally tagged by a `cmd` field, e.g.
/// `{"cmd":"select_slot","device":"...","slot":2}`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Request {
    Status,
    ListProfiles,
    ListDevices,
    Detect,
    ListBatteries,
    ListSlots {
        #[serde(default)]
        device: Option<String>,
    },
    AssignSlot {
        device: String,
        slot: u8,
        profile: String,
    },
    SelectSlot {
        device: String,
        slot: u8,
    },
    ClearSlot {
        device: String,
        slot: u8,
    },
    Apply {
        profile: String,
        controller: String,
    },
    ApplySlot {
        controller: String,
        #[serde(default)]
        slot: Option<u8>,
    },
    RemapOff,
}

/// A control response.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Response {
    fn ok(data: Value) -> Self {
        Response {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    fn err(message: impl Into<String>) -> Self {
        Response {
            ok: false,
            data: None,
            error: Some(message.into()),
        }
    }
}

struct ActiveRemap {
    controller_id: String,
    controller_path: String,
    profile_id: String,
    virtual_nodes: Vec<String>,
    session: RemapSession,
}

/// Holds control-daemon state: the slot store path and any active remap.
pub struct ServiceState {
    slot_store_path: PathBuf,
    active: Option<ActiveRemap>,
}

impl Default for ServiceState {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceState {
    pub fn new() -> Self {
        Self {
            slot_store_path: slot_store_path(),
            active: None,
        }
    }

    /// Construct a state using a specific slot store file (used in tests).
    pub fn with_slot_store_path(path: PathBuf) -> Self {
        Self {
            slot_store_path: path,
            active: None,
        }
    }

    /// Parse and handle one JSON request line, returning a JSON response line.
    pub fn handle_json(&mut self, line: &str) -> String {
        let response = match serde_json::from_str::<Request>(line.trim()) {
            Ok(request) => self.handle(request),
            Err(error) => Response::err(format!("invalid request: {error}")),
        };
        serde_json::to_string(&response).unwrap_or_else(|_| {
            "{\"ok\":false,\"error\":\"failed to encode response\"}".to_string()
        })
    }

    /// Handle a typed request.
    pub fn handle(&mut self, request: Request) -> Response {
        self.refresh_active();
        match request {
            Request::Status => self.status(),
            Request::ListProfiles => self.list_profiles(),
            Request::ListDevices => self.list_devices(),
            Request::Detect => self.detect(),
            Request::ListBatteries => Response::ok(json!(list_batteries())),
            Request::ListSlots { device } => self.list_slots(device.as_deref()),
            Request::AssignSlot {
                device,
                slot,
                profile,
            } => self.assign_slot(&device, slot, &profile),
            Request::SelectSlot { device, slot } => self.select_slot(&device, slot),
            Request::ClearSlot { device, slot } => self.clear_slot(&device, slot),
            Request::Apply {
                profile,
                controller,
            } => self.apply(&profile, &controller),
            Request::ApplySlot { controller, slot } => self.apply_slot(&controller, slot),
            Request::RemapOff => self.remap_off(),
        }
    }

    /// Drop the active remap if its worker has stopped or failed.
    fn refresh_active(&mut self) {
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let mut finished = active.session.is_finished();
        for message in active.session.poll() {
            match message {
                RemapMessage::Stopped => finished = true,
                RemapMessage::Failed(error) => {
                    log_warn!("service", "active remap failed: {error}");
                    finished = true;
                }
                RemapMessage::Running(_) => {}
            }
        }
        if finished {
            self.active = None;
        }
    }

    fn status(&self) -> Response {
        let active = self.active.as_ref().map(|active| {
            json!({
                "controller": active.controller_id,
                "controller_path": active.controller_path,
                "profile": active.profile_id,
                "virtual_nodes": active.virtual_nodes,
            })
        });
        Response::ok(json!({ "remap_active": active.is_some(), "active": active }))
    }

    fn list_profiles(&self) -> Response {
        match load_profiles(&default_profile_dirs()) {
            Ok(profiles) => Response::ok(json!(profiles
                .into_iter()
                .map(|profile| json!({
                    "id": profile.id,
                    "name": profile.name,
                    "source_path": profile.source_path.display().to_string(),
                }))
                .collect::<Vec<_>>())),
            Err(error) => Response::err(error.to_string()),
        }
    }

    fn list_devices(&self) -> Response {
        match list_devices() {
            Ok(devices) => Response::ok(json!(devices)),
            Err(error) => Response::err(error.to_string()),
        }
    }

    fn detect(&self) -> Response {
        let profiles = match load_profiles(&default_profile_dirs()) {
            Ok(profiles) => profiles,
            Err(error) => return Response::err(error.to_string()),
        };
        match detect_profile(&profiles) {
            Some((profile, detail)) => Response::ok(json!({
                "profile": profile.id,
                "name": profile.name,
                "process": detail.process_name,
            })),
            None => Response::ok(json!({ "profile": Value::Null })),
        }
    }

    fn load_store(&self) -> Result<SlotStore, Response> {
        load_slot_store_from(&self.slot_store_path)
            .map_err(|error| Response::err(error.to_string()))
    }

    fn save_store(&self, store: &SlotStore) -> Result<(), Response> {
        save_slot_store_to(&self.slot_store_path, store)
            .map_err(|error| Response::err(error.to_string()))
    }

    fn list_slots(&self, device: Option<&str>) -> Response {
        let store = match self.load_store() {
            Ok(store) => store,
            Err(response) => return response,
        };

        let device_ids: Vec<String> = match device {
            Some(selector) => match resolve_device_info(selector) {
                Ok(device) => vec![device.id],
                Err(error) => return Response::err(error.to_string()),
            },
            None => store.devices.keys().cloned().collect(),
        };

        let slots: Vec<Value> = device_ids
            .iter()
            .map(|device_id| device_slots_json(&store, device_id))
            .collect();
        Response::ok(json!(slots))
    }

    fn assign_slot(&self, selector: &str, slot: u8, profile: &str) -> Response {
        let device = match resolve_device_info(selector) {
            Ok(device) => device,
            Err(error) => return Response::err(error.to_string()),
        };
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(response) => return response,
        };
        if let Err(error) = store.assign_profile(&device.id, slot, profile) {
            return Response::err(error.to_string());
        }
        if let Err(response) = self.save_store(&store) {
            return response;
        }
        Response::ok(device_slots_json(&store, &device.id))
    }

    fn select_slot(&self, selector: &str, slot: u8) -> Response {
        let device = match resolve_device_info(selector) {
            Ok(device) => device,
            Err(error) => return Response::err(error.to_string()),
        };
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(response) => return response,
        };
        if let Err(error) = store.select_slot(&device.id, slot) {
            return Response::err(error.to_string());
        }
        if let Err(response) = self.save_store(&store) {
            return response;
        }
        Response::ok(device_slots_json(&store, &device.id))
    }

    fn clear_slot(&self, selector: &str, slot: u8) -> Response {
        let device = match resolve_device_info(selector) {
            Ok(device) => device,
            Err(error) => return Response::err(error.to_string()),
        };
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(response) => return response,
        };
        if let Err(error) = store.clear_slot(&device.id, slot) {
            return Response::err(error.to_string());
        }
        if let Err(response) = self.save_store(&store) {
            return response;
        }
        Response::ok(device_slots_json(&store, &device.id))
    }

    fn apply(&mut self, profile_selector: &str, controller: &str) -> Response {
        let device = match resolve_device_info(controller) {
            Ok(device) => device,
            Err(error) => return Response::err(error.to_string()),
        };
        let profile = match load_profiles(&default_profile_dirs())
            .map_err(|error| error.to_string())
            .and_then(|profiles| {
                profiles
                    .into_iter()
                    .find(|profile| {
                        profile.id == profile_selector || profile.name == profile_selector
                    })
                    .ok_or_else(|| format!("no profile matched {profile_selector}"))
            }) {
            Ok(profile) => profile,
            Err(error) => return Response::err(error),
        };
        self.start_remap(device.id, device.path, profile.id.clone(), profile)
    }

    fn apply_slot(&mut self, controller: &str, slot: Option<u8>) -> Response {
        let device = match resolve_device_info(controller) {
            Ok(device) => device,
            Err(error) => return Response::err(error.to_string()),
        };
        let mut store = match self.load_store() {
            Ok(store) => store,
            Err(response) => return response,
        };
        let selected_slot = match slot {
            Some(slot) => {
                if let Err(error) =
                    validate_slot(slot).and_then(|_| store.select_slot(&device.id, slot))
                {
                    return Response::err(error.to_string());
                }
                if let Err(response) = self.save_store(&store) {
                    return response;
                }
                slot
            }
            None => store.selected_slot(&device.id),
        };
        let profile_id = match store.profile_for_slot(&device.id, selected_slot) {
            Some(profile_id) => profile_id.to_string(),
            None => {
                return Response::err(format!("slot {selected_slot} on {} is empty", device.id))
            }
        };
        let profile = match load_profiles(&default_profile_dirs())
            .map_err(|error| error.to_string())
            .and_then(|profiles| {
                profiles
                    .into_iter()
                    .find(|profile| profile.id == profile_id)
                    .ok_or_else(|| format!("no profile matched {profile_id}"))
            }) {
            Ok(profile) => profile,
            Err(error) => return Response::err(error),
        };
        self.start_remap(device.id, device.path, profile_id, profile)
    }

    fn start_remap(
        &mut self,
        controller_id: String,
        controller_path: String,
        profile_id: String,
        profile: crate::profiles::Profile,
    ) -> Response {
        // Replace any existing remap.
        self.active = None;

        let session = RemapSession::start(profile, controller_path.clone());
        match session.recv() {
            Some(RemapMessage::Running(virtual_nodes)) => {
                log_info!(
                    "service",
                    "applied profile {profile_id} on {controller_id} via API"
                );
                let response = Response::ok(json!({
                    "controller": controller_id,
                    "profile": profile_id,
                    "virtual_nodes": virtual_nodes,
                }));
                self.active = Some(ActiveRemap {
                    controller_id,
                    controller_path,
                    profile_id,
                    virtual_nodes,
                    session,
                });
                response
            }
            Some(RemapMessage::Failed(error)) => Response::err(error),
            Some(RemapMessage::Stopped) | None => {
                Response::err("remap stopped before it started".to_string())
            }
        }
    }

    fn remap_off(&mut self) -> Response {
        match self.active.take() {
            Some(mut active) => {
                active.session.stop();
                log_info!("service", "remap turned off via API");
                Response::ok(json!({ "stopped": active.profile_id }))
            }
            None => Response::ok(json!({ "stopped": Value::Null })),
        }
    }
}

fn device_slots_json(store: &SlotStore, device_id: &str) -> Value {
    let selected = store.selected_slot(device_id);
    let slots: Vec<Value> = (1..=SLOT_COUNT)
        .map(|slot| {
            json!({
                "slot": slot,
                "selected": slot == selected,
                "profile": store.profile_for_slot(device_id, slot),
            })
        })
        .collect();
    json!({ "device": device_id, "selected_slot": selected, "slots": slots })
}

#[cfg(test)]
mod tests {
    use super::{Request, ServiceState};
    use serde_json::json;

    fn temp_state(name: &str) -> (ServiceState, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!(
            "padproxy-service-slots-{}-{name}.json",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        (ServiceState::with_slot_store_path(path.clone()), path)
    }

    #[test]
    fn assigns_and_lists_slots_over_api() {
        let (mut state, path) = temp_state("assign");

        let response = state.handle(Request::AssignSlot {
            device: "/dev/input/event-test".to_string(),
            slot: 2,
            profile: "nes".to_string(),
        });
        assert!(response.ok, "assign failed: {:?}", response.error);

        let data = response.data.unwrap();
        assert_eq!(data["device"], json!("path:/dev/input/event-test"));
        assert_eq!(data["selected_slot"], json!(2));

        let listed = state.handle(Request::ListSlots {
            device: Some("/dev/input/event-test".to_string()),
        });
        assert!(listed.ok);
        let slots = listed.data.unwrap();
        let slot2 = &slots[0]["slots"][1];
        assert_eq!(slot2["slot"], json!(2));
        assert_eq!(slot2["profile"], json!("nes"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn rejects_invalid_slot_over_api() {
        let (mut state, path) = temp_state("select");
        let response = state.handle(Request::SelectSlot {
            device: "/dev/input/event-test".to_string(),
            slot: 9,
        });
        assert!(!response.ok);
        assert!(response.error.unwrap().contains("slot"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn handles_json_round_trip_and_bad_input() {
        let (mut state, path) = temp_state("json");
        let out = state.handle_json("{\"cmd\":\"status\"}");
        assert!(out.contains("\"ok\":true"));
        assert!(out.contains("remap_active"));

        let bad = state.handle_json("not json");
        assert!(bad.contains("\"ok\":false"));
        assert!(bad.contains("invalid request"));
        let _ = std::fs::remove_file(&path);
    }
}
