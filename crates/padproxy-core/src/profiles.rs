use crate::devices::DeviceInfo;
use crate::event_code::{parse_event_code, virtual_xbox_supports, EventCode, EventKind};
use crate::outputs::normalize_output_type;
use anyhow::{anyhow, Context, Result};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const MAIN_LAYER_ID: &str = "main";
pub const MAX_SHIFT_LAYERS: usize = 10;
pub const DEFAULT_TURBO_INTERVAL_MS: u64 = 75;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceMatch {
    pub name: Option<String>,
    pub serial: Option<String>,
    pub phys: Option<String>,
    pub vendor_id: Option<IdValue>,
    pub product_id: Option<IdValue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdValue(u16);

#[derive(Clone, Debug, Serialize)]
pub struct Mapping {
    pub from: EventCode,
    pub to: EventCode,
    pub from_name: String,
    pub to_name: String,
    pub action: MappingAction,
    pub turbo: Option<TurboSettings>,
    #[serde(rename = "macro")]
    pub macro_settings: Option<MacroSettings>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingAction {
    Map,
    Disable,
    Macro,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TurboSettings {
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MacroSettings {
    pub mode: MacroMode,
    pub events: Vec<MacroEvent>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroMode {
    Press,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MacroEvent {
    pub kind: MacroEventKind,
    pub code: Option<EventCode>,
    pub code_name: String,
    pub value: i32,
    pub pause_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroEventKind {
    Down,
    Up,
    Tap,
    Axis,
    Pause,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AnalogSettings {
    pub axes: Vec<AnalogTuning>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnalogTuning {
    pub code: EventCode,
    pub code_name: String,
    pub deadzone: f64,
    pub sensitivity: f64,
    pub invert: bool,
    pub output_min: i32,
    pub output_max: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerActivationMode {
    Hold,
    Toggle,
}

#[derive(Clone, Debug, Serialize)]
pub struct LayerActivation {
    pub mode: LayerActivationMode,
    pub control: EventCode,
    pub control_name: String,
    pub consume: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    pub activation: Option<LayerActivation>,
    pub mappings: Vec<Mapping>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub device_match: DeviceMatch,
    pub output_type: String,
    pub passthrough: bool,
    pub grab_source: bool,
    pub source_path: PathBuf,
    pub mappings: Vec<Mapping>,
    pub layers: Vec<Layer>,
    pub analog: AnalogSettings,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    r#match: DeviceMatch,
    output: Option<RawOutput>,
    passthrough: Option<bool>,
    grab_source: Option<bool>,
    mappings: Option<Vec<RawMapping>>,
    layers: Option<Vec<RawLayer>>,
    analog: Option<RawAnalogSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawOutput {
    Scalar(String),
    Object { r#type: Option<String> },
}

#[derive(Debug, Deserialize)]
struct RawMapping {
    from: String,
    to: Option<String>,
    action: Option<String>,
    r#type: Option<String>,
    turbo: Option<RawTurbo>,
    turbo_interval_ms: Option<u64>,
    #[serde(rename = "macro")]
    macro_settings: Option<RawMacroSettings>,
    macro_events: Option<Vec<RawMacroEvent>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawTurbo {
    Enabled(bool),
    Object(RawTurboObject),
}

#[derive(Debug, Default, Deserialize)]
struct RawTurboObject {
    enabled: Option<bool>,
    interval_ms: Option<u64>,
    rate_hz: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMacroSettings {
    mode: Option<String>,
    #[serde(default)]
    events: Vec<RawMacroEvent>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMacroEvent {
    down: Option<String>,
    up: Option<String>,
    tap: Option<String>,
    axis: Option<String>,
    value: Option<i32>,
    pause_ms: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
struct RawAnalogSettings {
    axes: Option<Vec<RawAnalogTuning>>,
}

#[derive(Debug, Deserialize)]
struct RawAnalogTuning {
    code: String,
    deadzone: Option<f64>,
    sensitivity: Option<f64>,
    invert: Option<bool>,
    output_min: Option<i32>,
    output_max: Option<i32>,
    min: Option<i32>,
    max: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct RawLayer {
    id: Option<String>,
    name: Option<String>,
    activation: Option<RawLayerActivation>,
    activator: Option<RawLayerActivation>,
    mappings: Option<Vec<RawMapping>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawLayerActivation {
    Scalar(String),
    Object(RawLayerActivationObject),
}

#[derive(Debug, Default, Deserialize)]
struct RawLayerActivationObject {
    mode: Option<String>,
    control: Option<String>,
    from: Option<String>,
    source: Option<String>,
    button: Option<String>,
    hold: Option<String>,
    toggle: Option<String>,
    consume: Option<bool>,
}

impl IdValue {
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for IdValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdValueVisitor;

        impl Visitor<'_> for IdValueVisitor {
            type Value = IdValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a 16-bit integer or hex string like 0x045e")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u16::try_from(value)
                    .map(IdValue)
                    .map_err(|_| E::custom(format!("device id {value} is outside u16 range")))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 {
                    return Err(E::custom(format!("device id {value} must be non-negative")));
                }
                self.visit_u64(value as u64)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let trimmed = value.trim();
                let parsed = if let Some(hex) = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                {
                    u16::from_str_radix(hex, 16)
                } else {
                    trimmed.parse()
                };

                parsed
                    .map(IdValue)
                    .map_err(|_| E::custom(format!("invalid device id {value:?}")))
            }
        }

        deserializer.deserialize_any(IdValueVisitor)
    }
}

impl Serialize for IdValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

impl DeviceMatch {
    pub fn matches(&self, device: &DeviceInfo) -> bool {
        if let Some(name) = &self.name {
            if name != "*"
                && !device
                    .name
                    .to_ascii_lowercase()
                    .contains(&name.to_ascii_lowercase())
            {
                return false;
            }
        }
        if let Some(serial) = &self.serial {
            if &device.uniq != serial {
                return false;
            }
        }
        if let Some(phys) = &self.phys {
            if &device.phys != phys {
                return false;
            }
        }
        if let Some(vendor_id) = self.vendor_id {
            if device.vendor != vendor_id.as_u16() {
                return false;
            }
        }
        if let Some(product_id) = self.product_id {
            if device.product != product_id.as_u16() {
                return false;
            }
        }
        true
    }
}

impl Profile {
    pub fn mapping_table(&self) -> HashMap<EventCode, EventCode> {
        self.main_layer().mapping_table()
    }

    pub fn main_layer(&self) -> &Layer {
        self.layers
            .iter()
            .find(|layer| layer.id == MAIN_LAYER_ID)
            .unwrap_or_else(|| {
                self.layers
                    .first()
                    .expect("parsed profiles always contain a main layer")
            })
    }
}

impl Layer {
    pub fn mapping_table(&self) -> HashMap<EventCode, EventCode> {
        self.mappings
            .iter()
            .filter(|mapping| mapping.action == MappingAction::Map)
            .map(|mapping| (mapping.from, mapping.to))
            .collect()
    }

    pub fn mapping_behavior_table(&self) -> HashMap<EventCode, Mapping> {
        self.mappings
            .iter()
            .map(|mapping| (mapping.from, mapping.clone()))
            .collect()
    }
}

impl AnalogSettings {
    pub fn tuning_table(&self) -> HashMap<EventCode, AnalogTuning> {
        self.axes
            .iter()
            .map(|tuning| (tuning.code, tuning.clone()))
            .collect()
    }
}

impl AnalogTuning {
    pub fn apply(&self, value: i32) -> i32 {
        let Some(range) = AxisRange::for_event(self.code) else {
            return value;
        };

        let normalized = if range.centered {
            let max_abs = range.max.unsigned_abs().max(range.min.unsigned_abs()) as f64;
            if max_abs == 0.0 {
                0.0
            } else {
                (value as f64 / max_abs).clamp(-1.0, 1.0)
            }
        } else {
            let span = (range.max - range.min) as f64;
            if span == 0.0 {
                0.0
            } else {
                ((value - range.min) as f64 / span).clamp(0.0, 1.0)
            }
        };

        let deadzone = self.deadzone.clamp(0.0, 0.99);
        let mut adjusted = if range.centered {
            let magnitude = normalized.abs();
            if magnitude <= deadzone {
                0.0
            } else {
                normalized.signum() * ((magnitude - deadzone) / (1.0 - deadzone))
            }
        } else if normalized <= deadzone {
            0.0
        } else {
            (normalized - deadzone) / (1.0 - deadzone)
        };

        adjusted *= self.sensitivity;
        adjusted = if range.centered {
            adjusted.clamp(-1.0, 1.0)
        } else {
            adjusted.clamp(0.0, 1.0)
        };

        if self.invert {
            adjusted = if range.centered {
                -adjusted
            } else {
                1.0 - adjusted
            };
        }

        let scaled = if range.centered {
            let max_abs = range.max.unsigned_abs().max(range.min.unsigned_abs()) as f64;
            (adjusted * max_abs).round() as i32
        } else {
            let span = (range.max - range.min) as f64;
            range.min + (adjusted * span).round() as i32
        };

        scaled.clamp(self.output_min, self.output_max)
    }
}

#[derive(Clone, Copy)]
struct AxisRange {
    min: i32,
    max: i32,
    centered: bool,
}

impl AxisRange {
    fn for_event(event: EventCode) -> Option<Self> {
        if event.kind != EventKind::Absolute {
            return None;
        }

        match event.name().as_str() {
            "abs:z" | "abs:rz" => Some(Self {
                min: 0,
                max: 255,
                centered: false,
            }),
            "abs:hat0x" | "abs:hat0y" => Some(Self {
                min: -1,
                max: 1,
                centered: true,
            }),
            _ => Some(Self {
                min: -32768,
                max: 32767,
                centered: true,
            }),
        }
    }
}

pub fn default_profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut have_configured_profile_dir = false;

    if let Some(dir) = std::env::var_os("PADPROXY_PROFILE_DIR") {
        for path in std::env::split_paths(&dir) {
            push_profile_dir(&mut dirs, path);
            have_configured_profile_dir = true;
        }
    }

    if !have_configured_profile_dir {
        push_profile_dir(
            &mut dirs,
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("profiles"),
        );
    }

    if let Some(home) = std::env::var_os("HOME") {
        push_profile_dir(
            &mut dirs,
            PathBuf::from(home).join(".config/padproxy/profiles.d"),
        );
    }

    push_profile_dir(&mut dirs, PathBuf::from("/etc/padproxy/profiles.d"));
    dirs
}

fn push_profile_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}

pub fn load_profiles(dirs: &[PathBuf]) -> Result<Vec<Profile>> {
    let mut profiles = HashMap::new();

    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }

        for entry in
            fs::read_dir(dir).with_context(|| format!("failed to list {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let extension = path.extension().and_then(|value| value.to_str());
            if !matches!(extension, Some("yaml" | "yml")) {
                continue;
            }
            let profile = load_profile(&path)?;
            profiles.insert(profile.id.clone(), profile);
        }
    }

    let mut profiles = profiles.into_values().collect::<Vec<_>>();
    profiles.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(profiles)
}

pub fn load_profile(path: &Path) -> Result<Profile> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_profile_bytes(&bytes, path)
}

pub fn parse_profile_bytes(bytes: &[u8], source_path: &Path) -> Result<Profile> {
    let raw: RawProfile = serde_yaml::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", source_path.display()))?;

    let id = raw.id.unwrap_or_else(|| {
        source_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let name = raw.name.unwrap_or_else(|| id.clone());
    let output_type = match raw.output {
        Some(RawOutput::Scalar(value)) => value,
        Some(RawOutput::Object { r#type }) => r#type.unwrap_or_else(|| "xbox360".to_string()),
        None => "xbox360".to_string(),
    };
    let output_type = normalize_output_type(&output_type);

    let mut layers = Vec::new();
    layers.push(Layer {
        id: MAIN_LAYER_ID.to_string(),
        name: "Main".to_string(),
        activation: None,
        mappings: parse_mappings(raw.mappings, "top-level mappings")?,
    });

    for (index, layer) in raw.layers.unwrap_or_default().into_iter().enumerate() {
        let fallback_id = format!("shift_{}", index + 1);
        let id = normalize_layer_id(
            layer
                .id
                .as_deref()
                .or(layer.name.as_deref())
                .unwrap_or(&fallback_id),
        )?;
        let name = layer.name.unwrap_or_else(|| {
            if id == MAIN_LAYER_ID {
                "Main".to_string()
            } else {
                id.replace('_', " ")
            }
        });
        let activation = layer.activation.or(layer.activator);
        let mappings = parse_mappings(layer.mappings, &format!("layer {id} mappings"))?;

        if id == MAIN_LAYER_ID {
            if activation.is_some() {
                return Err(anyhow!("main layer cannot have an activation control"));
            }
            layers[0].name = name;
            layers[0].mappings.extend(mappings);
            continue;
        }

        if layers.iter().any(|existing| existing.id == id) {
            return Err(anyhow!("duplicate layer id {id}"));
        }

        let activation = parse_layer_activation(
            activation.ok_or_else(|| anyhow!("layer {id} requires activation"))?,
            &id,
        )?;

        layers.push(Layer {
            id,
            name,
            activation: Some(activation),
            mappings,
        });
    }

    if layers.len().saturating_sub(1) > MAX_SHIFT_LAYERS {
        return Err(anyhow!(
            "profiles support up to {MAX_SHIFT_LAYERS} shift layers"
        ));
    }

    let mappings = layers[0].mappings.clone();
    let analog = parse_analog_settings(raw.analog)?;

    Ok(Profile {
        id,
        name,
        description: raw.description.unwrap_or_default(),
        device_match: raw.r#match,
        output_type,
        passthrough: raw.passthrough.unwrap_or(true),
        grab_source: raw.grab_source.unwrap_or(true),
        source_path: source_path.to_path_buf(),
        mappings,
        layers,
        analog,
    })
}

fn parse_mappings(mappings: Option<Vec<RawMapping>>, context: &str) -> Result<Vec<Mapping>> {
    let mut parsed = Vec::new();
    for mapping in mappings.unwrap_or_default() {
        let from = parse_event_code(&mapping.from)
            .ok_or_else(|| anyhow!("unknown source event {} in {context}", mapping.from))?;
        let action_value = mapping.action.as_deref().or(mapping.r#type.as_deref());
        let mut action = parse_mapping_action(action_value)?;
        let has_macro_settings = mapping.macro_settings.is_some() || mapping.macro_events.is_some();
        if has_macro_settings && action_value.is_none() {
            action = MappingAction::Macro;
        }
        let to_value = mapping.to.as_deref();
        let action = match (action, to_value.map(normalize_mapping_keyword)) {
            (_, Some(keyword)) if matches!(keyword.as_str(), "disable" | "disabled" | "none") => {
                MappingAction::Disable
            }
            (action, _) => action,
        };
        let macro_settings = parse_macro_settings(
            mapping.macro_settings,
            mapping.macro_events,
            action,
            from,
            &mapping.from,
            context,
        )?;

        let to = match action {
            MappingAction::Map => {
                let to = to_value.ok_or_else(|| {
                    anyhow!("mapping from {} in {context} requires to", mapping.from)
                })?;
                let to = parse_event_code(to)
                    .ok_or_else(|| anyhow!("unknown target event {to} in {context}"))?;
                if !virtual_xbox_supports(to) {
                    return Err(anyhow!(
                        "target event {} in {context} is not supported by the current virtual pad",
                        to.name()
                    ));
                }
                to
            }
            MappingAction::Disable => from,
            MappingAction::Macro => macro_settings
                .as_ref()
                .and_then(|macro_settings| {
                    macro_settings.events.iter().find_map(|event| event.code)
                })
                .unwrap_or(from),
        };

        let turbo = parse_turbo(mapping.turbo, mapping.turbo_interval_ms)?;
        if turbo.is_some() {
            if action == MappingAction::Disable {
                return Err(anyhow!(
                    "disabled mapping from {} in {context} cannot use turbo",
                    mapping.from
                ));
            }
            if action == MappingAction::Macro {
                return Err(anyhow!(
                    "macro mapping from {} in {context} cannot use turbo",
                    mapping.from
                ));
            }
            if from.kind != crate::event_code::EventKind::Key
                || to.kind != crate::event_code::EventKind::Key
            {
                return Err(anyhow!(
                    "turbo mapping from {} in {context} requires button source and target",
                    mapping.from
                ));
            }
        }
        parsed.push(Mapping {
            from,
            to,
            from_name: from.name(),
            to_name: to.name(),
            action,
            turbo,
            macro_settings,
        });
    }
    Ok(parsed)
}

fn parse_mapping_action(value: Option<&str>) -> Result<MappingAction> {
    let Some(value) = value else {
        return Ok(MappingAction::Map);
    };

    match normalize_mapping_keyword(value).as_str() {
        "map" | "remap" | "virtual" | "controller" => Ok(MappingAction::Map),
        "disable" | "disabled" | "mute" | "block" | "none" => Ok(MappingAction::Disable),
        "macro" | "combo" | "sequence" => Ok(MappingAction::Macro),
        other => Err(anyhow!("unknown mapping action {other}")),
    }
}

fn parse_macro_settings(
    raw: Option<RawMacroSettings>,
    raw_events: Option<Vec<RawMacroEvent>>,
    action: MappingAction,
    from: EventCode,
    from_name: &str,
    context: &str,
) -> Result<Option<MacroSettings>> {
    if action != MappingAction::Macro {
        if raw.is_some() || raw_events.is_some() {
            return Err(anyhow!(
                "mapping from {from_name} in {context} has macro events but action is not macro"
            ));
        }
        return Ok(None);
    }

    if from.kind != EventKind::Key {
        return Err(anyhow!(
            "macro mapping from {from_name} in {context} requires a button source"
        ));
    }

    let (mode, raw_events) = match (raw, raw_events) {
        (Some(_), Some(_)) => {
            return Err(anyhow!(
                "macro mapping from {from_name} in {context} cannot use both macro and macro_events"
            ))
        }
        (Some(raw), None) => (
            raw.mode
                .as_deref()
                .map(parse_macro_mode)
                .transpose()?
                .unwrap_or(MacroMode::Press),
            raw.events,
        ),
        (None, Some(raw_events)) => (MacroMode::Press, raw_events),
        (None, None) => {
            return Err(anyhow!(
                "macro mapping from {from_name} in {context} requires macro events"
            ))
        }
    };

    if raw_events.is_empty() {
        return Err(anyhow!(
            "macro mapping from {from_name} in {context} requires at least one event"
        ));
    }

    let mut events = Vec::new();
    for (index, raw_event) in raw_events.into_iter().enumerate() {
        events.push(parse_macro_event(raw_event, index, from_name, context)?);
    }

    Ok(Some(MacroSettings { mode, events }))
}

fn parse_macro_mode(value: &str) -> Result<MacroMode> {
    match normalize_mapping_keyword(value).as_str() {
        "press" | "single_press" | "on_press" | "execute" | "execute_at_once" => {
            Ok(MacroMode::Press)
        }
        other => Err(anyhow!("unknown macro mode {other}")),
    }
}

fn parse_macro_event(
    raw: RawMacroEvent,
    index: usize,
    from_name: &str,
    context: &str,
) -> Result<MacroEvent> {
    let specified = [
        raw.down.is_some(),
        raw.up.is_some(),
        raw.tap.is_some(),
        raw.axis.is_some(),
        raw.pause_ms.is_some(),
    ]
    .into_iter()
    .filter(|specified| *specified)
    .count();

    if specified != 1 {
        return Err(anyhow!(
            "macro event {} from {from_name} in {context} must set exactly one of down, up, tap, axis, or pause_ms",
            index + 1
        ));
    }

    if let Some(pause_ms) = raw.pause_ms {
        if pause_ms > 60_000 {
            return Err(anyhow!(
                "macro pause_ms for event {} from {from_name} in {context} must be at most 60000",
                index + 1
            ));
        }
        return Ok(MacroEvent {
            kind: MacroEventKind::Pause,
            code: None,
            code_name: String::new(),
            value: 0,
            pause_ms,
        });
    }

    if let Some(code) = raw.down {
        let code = parse_macro_target(&code, MacroEventKind::Down, index, from_name, context)?;
        return Ok(MacroEvent {
            kind: MacroEventKind::Down,
            code: Some(code),
            code_name: code.name(),
            value: 1,
            pause_ms: 0,
        });
    }

    if let Some(code) = raw.up {
        let code = parse_macro_target(&code, MacroEventKind::Up, index, from_name, context)?;
        return Ok(MacroEvent {
            kind: MacroEventKind::Up,
            code: Some(code),
            code_name: code.name(),
            value: 0,
            pause_ms: 0,
        });
    }

    if let Some(code) = raw.tap {
        let code = parse_macro_target(&code, MacroEventKind::Tap, index, from_name, context)?;
        return Ok(MacroEvent {
            kind: MacroEventKind::Tap,
            code: Some(code),
            code_name: code.name(),
            value: 1,
            pause_ms: 0,
        });
    }

    let code = raw
        .axis
        .expect("specified count guarantees axis is present when no other event matched");
    let code = parse_macro_target(&code, MacroEventKind::Axis, index, from_name, context)?;
    let value = raw.value.ok_or_else(|| {
        anyhow!(
            "macro axis event {} from {from_name} in {context} requires value",
            index + 1
        )
    })?;
    let range = AxisRange::for_event(code).ok_or_else(|| {
        anyhow!(
            "macro axis event {} from {from_name} in {context} must target an absolute axis",
            index + 1
        )
    })?;
    if value < range.min || value > range.max {
        return Err(anyhow!(
            "macro axis value for event {} from {from_name} in {context} must stay within {}..{}",
            index + 1,
            range.min,
            range.max
        ));
    }

    Ok(MacroEvent {
        kind: MacroEventKind::Axis,
        code: Some(code),
        code_name: code.name(),
        value,
        pause_ms: 0,
    })
}

fn parse_macro_target(
    value: &str,
    kind: MacroEventKind,
    index: usize,
    from_name: &str,
    context: &str,
) -> Result<EventCode> {
    let code = parse_event_code(value).ok_or_else(|| {
        anyhow!(
            "unknown macro target event {value} for event {} from {from_name} in {context}",
            index + 1
        )
    })?;
    if !virtual_xbox_supports(code) {
        return Err(anyhow!(
            "macro target event {} for event {} from {from_name} in {context} is not supported by the current virtual pad",
            code.name(),
            index + 1
        ));
    }

    match kind {
        MacroEventKind::Down | MacroEventKind::Up | MacroEventKind::Tap
            if code.kind != EventKind::Key =>
        {
            Err(anyhow!(
                "macro button event {} from {from_name} in {context} must target a button",
                index + 1
            ))
        }
        MacroEventKind::Axis if code.kind != EventKind::Absolute => Err(anyhow!(
            "macro axis event {} from {from_name} in {context} must target an absolute axis",
            index + 1
        )),
        _ => Ok(code),
    }
}

fn parse_turbo(
    raw: Option<RawTurbo>,
    turbo_interval_ms: Option<u64>,
) -> Result<Option<TurboSettings>> {
    let (enabled, interval_ms) = match raw {
        Some(RawTurbo::Enabled(value)) => (
            value,
            turbo_interval_ms.unwrap_or(DEFAULT_TURBO_INTERVAL_MS),
        ),
        Some(RawTurbo::Object(object)) => {
            let mut interval_ms = turbo_interval_ms.unwrap_or(DEFAULT_TURBO_INTERVAL_MS);
            if let Some(value) = object.interval_ms {
                interval_ms = value;
            }
            if let Some(rate_hz) = object.rate_hz {
                if !(rate_hz.is_finite() && rate_hz > 0.0) {
                    return Err(anyhow!("turbo rate_hz must be positive"));
                }
                interval_ms = (1000.0 / (rate_hz * 2.0)).round() as u64;
            }
            (object.enabled.unwrap_or(true), interval_ms)
        }
        None => (
            turbo_interval_ms.is_some(),
            turbo_interval_ms.unwrap_or(DEFAULT_TURBO_INTERVAL_MS),
        ),
    };

    if !enabled {
        return Ok(None);
    }
    if !(10..=5000).contains(&interval_ms) {
        return Err(anyhow!("turbo interval_ms must be between 10 and 5000"));
    }

    Ok(Some(TurboSettings { interval_ms }))
}

fn normalize_mapping_keyword(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace([' ', '-'], "_")
}

fn parse_analog_settings(raw: Option<RawAnalogSettings>) -> Result<AnalogSettings> {
    let Some(raw) = raw else {
        return Ok(AnalogSettings::default());
    };

    let mut axes = Vec::new();
    for axis in raw.axes.unwrap_or_default() {
        let code = parse_event_code(&axis.code)
            .ok_or_else(|| anyhow!("unknown analog axis {}", axis.code))?;
        if code.kind != EventKind::Absolute {
            return Err(anyhow!(
                "analog tuning {} must target an absolute axis",
                axis.code
            ));
        }
        if !virtual_xbox_supports(code) {
            return Err(anyhow!(
                "analog tuning {} is not supported by the current virtual pad",
                axis.code
            ));
        }
        if axes
            .iter()
            .any(|existing: &AnalogTuning| existing.code == code)
        {
            return Err(anyhow!("duplicate analog tuning for {}", code.name()));
        }

        let range = AxisRange::for_event(code).expect("absolute axes have an output range");
        let deadzone = axis.deadzone.unwrap_or(0.0);
        if !(0.0..=0.99).contains(&deadzone) {
            return Err(anyhow!(
                "analog deadzone for {} must be between 0 and 0.99",
                axis.code
            ));
        }
        let sensitivity = axis.sensitivity.unwrap_or(1.0);
        if !(0.01..=4.0).contains(&sensitivity) {
            return Err(anyhow!(
                "analog sensitivity for {} must be between 0.01 and 4.0",
                axis.code
            ));
        }

        let output_min = axis.output_min.or(axis.min).unwrap_or(range.min);
        let output_max = axis.output_max.or(axis.max).unwrap_or(range.max);
        if output_min < range.min || output_max > range.max || output_min >= output_max {
            return Err(anyhow!(
                "analog output range for {} must stay within {}..{}",
                axis.code,
                range.min,
                range.max
            ));
        }

        axes.push(AnalogTuning {
            code,
            code_name: code.name(),
            deadzone,
            sensitivity,
            invert: axis.invert.unwrap_or(false),
            output_min,
            output_max,
        });
    }

    Ok(AnalogSettings { axes })
}

fn parse_layer_activation(raw: RawLayerActivation, layer_id: &str) -> Result<LayerActivation> {
    let (mode, control, consume) = match raw {
        RawLayerActivation::Scalar(control) => (LayerActivationMode::Hold, control, true),
        RawLayerActivation::Object(object) => {
            let mut mode = object
                .mode
                .as_deref()
                .map(parse_activation_mode)
                .transpose()?
                .unwrap_or(LayerActivationMode::Hold);
            let mut control = object
                .control
                .or(object.from)
                .or(object.source)
                .or(object.button);

            if let Some(hold) = object.hold {
                mode = LayerActivationMode::Hold;
                control = Some(hold);
            }
            if let Some(toggle) = object.toggle {
                mode = LayerActivationMode::Toggle;
                control = Some(toggle);
            }

            (
                mode,
                control.ok_or_else(|| anyhow!("layer {layer_id} activation requires a control"))?,
                object.consume.unwrap_or(true),
            )
        }
    };

    let control = parse_event_code(&control)
        .ok_or_else(|| anyhow!("unknown activation control {control} for layer {layer_id}"))?;

    Ok(LayerActivation {
        mode,
        control,
        control_name: control.name(),
        consume,
    })
}

fn parse_activation_mode(value: &str) -> Result<LayerActivationMode> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .as_str()
    {
        "hold" | "held" | "momentary" | "while_held" => Ok(LayerActivationMode::Hold),
        "toggle" | "toggles" | "on_off" => Ok(LayerActivationMode::Toggle),
        other => Err(anyhow!("unknown layer activation mode {other}")),
    }
}

fn normalize_layer_id(value: &str) -> Result<String> {
    let normalized = value
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '_')
        .collect::<String>();
    if normalized.is_empty() {
        return Err(anyhow!(
            "layer id must contain at least one letter or number"
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use crate::event_code::parse_event_code;

    use super::{
        parse_profile_bytes, AnalogTuning, LayerActivationMode, MacroEventKind, MacroMode,
        MappingAction, DEFAULT_TURBO_INTERVAL_MS, MAIN_LAYER_ID, MAX_SHIFT_LAYERS,
    };
    use std::path::Path;

    #[test]
    fn flat_mappings_become_main_layer() {
        let profile = parse_profile_bytes(
            br#"
id: flat
mappings:
  - from: btn:south
    to: btn:east
"#,
            Path::new("flat.yaml"),
        )
        .unwrap();

        assert_eq!(profile.layers.len(), 1);
        assert_eq!(profile.layers[0].id, MAIN_LAYER_ID);
        assert_eq!(profile.mappings.len(), 1);
        assert_eq!(profile.mapping_table().len(), 1);
        assert_eq!(profile.mappings[0].action, MappingAction::Map);
        assert!(profile.mappings[0].turbo.is_none());
    }

    #[test]
    fn parses_hold_and_toggle_shift_layers() {
        let profile = parse_profile_bytes(
            br#"
id: layered
layers:
  - id: main
    mappings:
      - from: btn:south
        to: btn:south
  - id: shift_1
    name: Shift 1
    activation:
      hold: btn:tl
      consume: true
    mappings:
      - from: btn:south
        to: btn:east
  - id: shift_2
    activation:
      mode: toggle
      control: btn:tr
      consume: false
    mappings:
      - from: btn:south
        to: btn:west
"#,
            Path::new("layered.yaml"),
        )
        .unwrap();

        assert_eq!(profile.layers.len(), 3);
        assert_eq!(profile.layers[0].mappings.len(), 1);
        let hold = profile.layers[1].activation.as_ref().unwrap();
        assert_eq!(hold.mode, LayerActivationMode::Hold);
        assert_eq!(hold.control_name, "btn:tl");
        assert!(hold.consume);

        let toggle = profile.layers[2].activation.as_ref().unwrap();
        assert_eq!(toggle.mode, LayerActivationMode::Toggle);
        assert_eq!(toggle.control_name, "btn:tr");
        assert!(!toggle.consume);
    }

    #[test]
    fn rejects_too_many_shift_layers() {
        let mut yaml = String::from("id: too-many\nlayers:\n");
        for index in 0..=MAX_SHIFT_LAYERS {
            yaml.push_str(&format!(
                "  - id: shift_{index}\n    activation: btn:tl\n    mappings: []\n"
            ));
        }

        let error = parse_profile_bytes(yaml.as_bytes(), Path::new("too-many.yaml"))
            .unwrap_err()
            .to_string();
        assert!(error.contains("shift layers"), "{error}");
    }

    #[test]
    fn parses_disabled_and_turbo_mappings() {
        let profile = parse_profile_bytes(
            br#"
id: behavior
mappings:
  - from: btn:select
    action: disable
  - from: btn:south
    to: btn:east
    turbo: true
  - from: btn:west
    to: btn:north
    turbo:
      interval_ms: 120
"#,
            Path::new("behavior.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings[0].action, MappingAction::Disable);
        assert_eq!(profile.mappings[0].to_name, "btn:select");
        assert_eq!(profile.mapping_table().len(), 2);
        assert_eq!(
            profile.mappings[1].turbo.as_ref().unwrap().interval_ms,
            DEFAULT_TURBO_INTERVAL_MS
        );
        assert_eq!(profile.mappings[2].turbo.as_ref().unwrap().interval_ms, 120);
    }

    #[test]
    fn rejects_turbo_for_disabled_or_axis_mappings() {
        let disabled_error = parse_profile_bytes(
            br#"
id: disabled-turbo
mappings:
  - from: btn:south
    action: disable
    turbo: true
"#,
            Path::new("disabled-turbo.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            disabled_error.contains("cannot use turbo"),
            "{disabled_error}"
        );

        let axis_error = parse_profile_bytes(
            br#"
id: axis-turbo
mappings:
  - from: abs:x
    to: abs:y
    turbo: true
"#,
            Path::new("axis-turbo.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            axis_error.contains("button source and target"),
            "{axis_error}"
        );
    }

    #[test]
    fn parses_controller_macro_mappings() {
        let profile = parse_profile_bytes(
            br#"
id: macro
mappings:
  - from: btn:start
    action: macro
    macro:
      mode: press
      events:
        - down: btn:south
        - pause_ms: 40
        - up: btn:south
        - tap: btn:east
        - axis: abs:x
          value: 12000
        - pause_ms: 20
        - axis: abs:x
          value: 0
"#,
            Path::new("macro.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings.len(), 1);
        assert_eq!(profile.mappings[0].action, MappingAction::Macro);
        assert_eq!(profile.mappings[0].to_name, "btn:south");
        assert!(profile.mapping_table().is_empty());

        let macro_settings = profile.mappings[0].macro_settings.as_ref().unwrap();
        assert_eq!(macro_settings.mode, MacroMode::Press);
        assert_eq!(macro_settings.events.len(), 7);
        assert_eq!(macro_settings.events[0].kind, MacroEventKind::Down);
        assert_eq!(macro_settings.events[0].code_name, "btn:south");
        assert_eq!(macro_settings.events[1].kind, MacroEventKind::Pause);
        assert_eq!(macro_settings.events[1].pause_ms, 40);
        assert_eq!(macro_settings.events[3].kind, MacroEventKind::Tap);
        assert_eq!(macro_settings.events[4].kind, MacroEventKind::Axis);
        assert_eq!(macro_settings.events[4].code_name, "abs:x");
        assert_eq!(macro_settings.events[4].value, 12000);
    }

    #[test]
    fn rejects_invalid_macro_mappings() {
        let axis_source_error = parse_profile_bytes(
            br#"
id: axis-source
mappings:
  - from: abs:x
    action: macro
    macro:
      events:
        - tap: btn:south
"#,
            Path::new("axis-source.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            axis_source_error.contains("requires a button source"),
            "{axis_source_error}"
        );

        let turbo_error = parse_profile_bytes(
            br#"
id: turbo-macro
mappings:
  - from: btn:start
    action: macro
    turbo: true
    macro:
      events:
        - tap: btn:south
"#,
            Path::new("turbo-macro.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(turbo_error.contains("cannot use turbo"), "{turbo_error}");

        let bad_axis_error = parse_profile_bytes(
            br#"
id: bad-axis-macro
mappings:
  - from: btn:start
    action: macro
    macro:
      events:
        - axis: abs:x
"#,
            Path::new("bad-axis-macro.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            bad_axis_error.contains("requires value"),
            "{bad_axis_error}"
        );
    }

    #[test]
    fn parses_analog_axis_tuning() {
        let profile = parse_profile_bytes(
            br#"
id: analog
analog:
  axes:
    - code: abs:x
      deadzone: 0.2
      sensitivity: 1.5
      invert: true
      output_min: -20000
      output_max: 20000
    - code: abs:z
      deadzone: 0.1
      max: 200
"#,
            Path::new("analog.yaml"),
        )
        .unwrap();

        assert_eq!(profile.analog.axes.len(), 2);
        assert_eq!(profile.analog.axes[0].code_name, "abs:x");
        assert_eq!(profile.analog.axes[0].deadzone, 0.2);
        assert_eq!(profile.analog.axes[0].sensitivity, 1.5);
        assert!(profile.analog.axes[0].invert);
        assert_eq!(profile.analog.axes[0].output_min, -20000);
        assert_eq!(profile.analog.axes[1].code_name, "abs:z");
        assert_eq!(profile.analog.tuning_table().len(), 2);
    }

    #[test]
    fn rejects_invalid_analog_tuning() {
        let error = parse_profile_bytes(
            br#"
id: bad-analog
analog:
  axes:
    - code: btn:south
      deadzone: 0.1
"#,
            Path::new("bad-analog.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("absolute axis"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-range
analog:
  axes:
    - code: abs:x
      output_min: -50000
"#,
            Path::new("bad-range.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("output range"), "{error}");
    }

    #[test]
    fn applies_analog_tuning_to_sticks_and_triggers() {
        let stick = AnalogTuning {
            code: parse_event_code("abs:x").unwrap(),
            code_name: "abs:x".to_string(),
            deadzone: 0.2,
            sensitivity: 1.5,
            invert: true,
            output_min: -20000,
            output_max: 20000,
        };
        assert_eq!(stick.apply(3000), 0);
        assert!(stick.apply(16_384) < -18_000);
        assert_eq!(stick.apply(32_767), -20000);

        let trigger = AnalogTuning {
            code: parse_event_code("abs:z").unwrap(),
            code_name: "abs:z".to_string(),
            deadzone: 0.1,
            sensitivity: 1.0,
            invert: false,
            output_min: 0,
            output_max: 200,
        };
        assert_eq!(trigger.apply(10), 0);
        assert_eq!(trigger.apply(255), 200);
    }
}
