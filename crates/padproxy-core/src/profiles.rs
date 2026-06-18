use crate::devices::DeviceInfo;
use crate::event_code::{
    parse_event_code, virtual_output_supports, virtual_xbox_supports, EventCode, EventKind,
};
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
pub const DEFAULT_LONG_PRESS_MS: u64 = 450;
pub const DEFAULT_MULTI_PRESS_MS: u64 = 300;
pub const MACRO_TAP_RELEASE_MS: u64 = 20;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceMatch {
    pub name: Option<String>,
    pub serial: Option<String>,
    pub phys: Option<String>,
    pub vendor_id: Option<IdValue>,
    pub product_id: Option<IdValue>,
}

/// Patterns that identify the game/app a profile is meant for, used for process
/// autodetection. A pattern matches a running process when it equals the
/// process basename (case-insensitively) or, if it contains `*`/`?`, when it
/// glob-matches the process name.
#[derive(Clone, Debug, Default, Serialize)]
pub struct ProcessMatch {
    pub patterns: Vec<String>,
}

impl ProcessMatch {
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Returns true if any pattern matches the given process name. The name may
    /// be a short `comm` value or a full executable path; both the raw value
    /// and its basename are tested.
    pub fn matches(&self, process_name: &str) -> bool {
        let candidate = process_name.trim();
        if candidate.is_empty() {
            return false;
        }
        let basename = candidate.rsplit('/').next().unwrap_or(candidate);
        self.patterns
            .iter()
            .any(|pattern| pattern_matches(pattern, candidate, basename))
    }
}

fn pattern_matches(pattern: &str, candidate: &str, basename: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    if pattern.contains('*') || pattern.contains('?') {
        glob_matches_ci(pattern, candidate) || glob_matches_ci(pattern, basename)
    } else {
        pattern.eq_ignore_ascii_case(basename) || pattern.eq_ignore_ascii_case(candidate)
    }
}

/// Case-insensitive glob match supporting `*` (any run) and `?` (one char).
fn glob_matches_ci(pattern: &str, value: &str) -> bool {
    let pattern: Vec<char> = pattern.to_ascii_lowercase().chars().collect();
    let value: Vec<char> = value.to_ascii_lowercase().chars().collect();

    // Iterative backtracking glob matcher.
    let (mut p, mut v) = (0usize, 0usize);
    let (mut star_p, mut star_v): (Option<usize>, usize) = (None, 0);

    while v < value.len() {
        if p < pattern.len() && (pattern[p] == '?' || pattern[p] == value[v]) {
            p += 1;
            v += 1;
        } else if p < pattern.len() && pattern[p] == '*' {
            star_p = Some(p);
            star_v = v;
            p += 1;
        } else if let Some(sp) = star_p {
            p = sp + 1;
            star_v += 1;
            v = star_v;
        } else {
            return false;
        }
    }

    while p < pattern.len() && pattern[p] == '*' {
        p += 1;
    }
    p == pattern.len()
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
    pub activator: ActivatorSettings,
    pub turbo: Option<TurboSettings>,
    #[serde(rename = "macro")]
    pub macro_settings: Option<MacroSettings>,
    pub command: Option<CommandSettings>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingAction {
    Map,
    Disable,
    Macro,
    Command,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ActivatorSettings {
    pub kind: ActivatorKind,
    pub delay_ms: u64,
}

impl Default for ActivatorSettings {
    fn default() -> Self {
        Self {
            kind: ActivatorKind::Press,
            delay_ms: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivatorKind {
    Press,
    Release,
    LongPress,
    DoublePress,
    TriplePress,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct TurboSettings {
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MacroSettings {
    pub mode: MacroMode,
    pub events: Vec<MacroEvent>,
    pub release_events: Vec<MacroEvent>,
    pub duration_ms: u64,
    pub release_duration_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CommandSettings {
    pub action: CommandAction,
    pub command_line: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandAction {
    StopMacros,
    RunCommand,
    /// Emergency "turn remap off": stop the running remap, release the virtual
    /// device, and ungrab the source controller.
    RemapOff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MacroMode {
    Press,
    Hold,
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
    Relative,
    Pause,
    Cancel,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct AnalogSettings {
    pub axes: Vec<AnalogTuning>,
    pub sticks: Vec<StickPairSettings>,
    pub swap_sticks: bool,
    pub digital_sticks: Vec<DigitalStickMapping>,
}

#[derive(Clone, Debug, Serialize)]
pub struct AnalogTuning {
    pub code: EventCode,
    pub code_name: String,
    pub deadzone: f64,
    pub sensitivity: f64,
    pub curve: AnalogCurve,
    pub curve_exponent: f64,
    pub invert: bool,
    pub output_min: i32,
    pub output_max: i32,
    pub zones: Vec<AnalogZoneMapping>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalogCurve {
    Linear,
    Soft,
    Aggressive,
    Custom,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AnalogZoneMapping {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub target: EventCode,
    pub target_name: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct StickPairSettings {
    pub x_axis: EventCode,
    pub x_axis_name: String,
    pub y_axis: EventCode,
    pub y_axis_name: String,
    pub rotation_degrees: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StickTransformMapping {
    pub source_x: EventCode,
    pub source_y: EventCode,
    pub output_x: EventCode,
    pub output_y: EventCode,
    pub rotation_degrees: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DigitalStickMapping {
    pub x_axis: EventCode,
    pub x_axis_name: String,
    pub y_axis: EventCode,
    pub y_axis_name: String,
    pub threshold: f64,
    pub output_x: EventCode,
    pub output_x_name: String,
    pub output_y: EventCode,
    pub output_y_name: String,
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
    /// Additional source devices grouped into the same virtual controller.
    pub groups: Vec<DeviceMatch>,
    pub process_match: ProcessMatch,
    pub output_type: String,
    /// Additional virtual output devices emitted alongside the primary one.
    pub outputs: Vec<String>,
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
    #[serde(default)]
    group: Vec<DeviceMatch>,
    process: Option<RawStringOrList>,
    output: Option<RawOutput>,
    #[serde(default)]
    outputs: Vec<String>,
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
    activator: Option<RawMappingActivator>,
    turbo: Option<RawTurbo>,
    turbo_interval_ms: Option<u64>,
    #[serde(rename = "macro")]
    macro_settings: Option<RawMacroSettings>,
    macro_events: Option<Vec<RawMacroEvent>>,
    command: Option<RawCommandSettings>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawMappingActivator {
    Scalar(String),
    Object(RawMappingActivatorObject),
}

#[derive(Debug, Default, Deserialize)]
struct RawMappingActivatorObject {
    kind: Option<String>,
    r#type: Option<String>,
    mode: Option<String>,
    delay_ms: Option<u64>,
    timeout_ms: Option<u64>,
    interval_ms: Option<u64>,
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
    release_events: Option<Vec<RawMacroEvent>>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMacroEvent {
    down: Option<String>,
    up: Option<String>,
    tap: Option<String>,
    axis: Option<String>,
    stick: Option<String>,
    x: Option<f64>,
    y: Option<f64>,
    rel: Option<String>,
    value: Option<i32>,
    pause_ms: Option<u64>,
    cancel: Option<bool>,
    #[serde(rename = "break")]
    break_event: Option<bool>,
    stop: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawCommandSettings {
    Scalar(String),
    Object(RawCommandObject),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawCommandLine {
    Args(Vec<String>),
    Shell(String),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawStringOrList {
    Scalar(String),
    List(Vec<String>),
}

impl RawStringOrList {
    fn into_vec(self) -> Vec<String> {
        match self {
            RawStringOrList::Scalar(value) => vec![value],
            RawStringOrList::List(values) => values,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct RawCommandObject {
    action: Option<String>,
    name: Option<String>,
    command: Option<String>,
    command_line: Option<RawCommandLine>,
    args: Option<Vec<String>>,
    argv: Option<Vec<String>>,
    program: Option<String>,
    executable: Option<String>,
    shell: Option<String>,
    run: Option<RawCommandLine>,
}

#[derive(Debug, Default, Deserialize)]
struct RawAnalogSettings {
    axes: Option<Vec<RawAnalogTuning>>,
    sticks: Option<Vec<RawStickPairSettings>>,
    stick_rotations: Option<Vec<RawStickPairSettings>>,
    rotation: Option<Vec<RawStickPairSettings>>,
    swap_sticks: Option<bool>,
    swap: Option<bool>,
    digital_sticks: Option<Vec<RawDigitalStickMapping>>,
    digital: Option<Vec<RawDigitalStickMapping>>,
}

#[derive(Debug, Deserialize)]
struct RawAnalogTuning {
    code: String,
    deadzone: Option<f64>,
    sensitivity: Option<f64>,
    curve: Option<String>,
    curve_exponent: Option<f64>,
    exponent: Option<f64>,
    invert: Option<bool>,
    output_min: Option<i32>,
    output_max: Option<i32>,
    min: Option<i32>,
    max: Option<i32>,
    zones: Option<Vec<RawAnalogZoneMapping>>,
}

#[derive(Debug, Deserialize)]
struct RawAnalogZoneMapping {
    name: Option<String>,
    zone: Option<String>,
    to: Option<String>,
    target: Option<String>,
    output: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    threshold: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawStickPairSettings {
    x_axis: Option<String>,
    y_axis: Option<String>,
    x: Option<String>,
    y: Option<String>,
    rotation_degrees: Option<f64>,
    rotation: Option<f64>,
    rotate: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct RawDigitalStickMapping {
    x_axis: Option<String>,
    y_axis: Option<String>,
    x: Option<String>,
    y: Option<String>,
    threshold: Option<f64>,
    output_x: Option<String>,
    output_y: Option<String>,
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

/// Resolve grouped device matchers to source device paths.
///
/// Each matcher picks the first available device it matches, skipping any path
/// in `exclude_paths` and any device already chosen by an earlier matcher (so a
/// group of two same-type controllers resolves to two distinct devices).
pub fn resolve_group_paths(
    matchers: &[DeviceMatch],
    devices: &[DeviceInfo],
    exclude_paths: &[String],
) -> Vec<String> {
    let mut chosen: Vec<String> = Vec::new();
    for matcher in matchers {
        if let Some(device) = devices.iter().find(|device| {
            matcher.matches(device)
                && !exclude_paths.contains(&device.path)
                && !chosen.contains(&device.path)
        }) {
            chosen.push(device.path.clone());
        }
    }
    chosen
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

    pub fn stick_transforms(&self) -> Vec<StickTransformMapping> {
        let left_x = parse_event_code("abs:x").expect("built-in event code");
        let left_y = parse_event_code("abs:y").expect("built-in event code");
        let right_x = parse_event_code("abs:rx").expect("built-in event code");
        let right_y = parse_event_code("abs:ry").expect("built-in event code");

        if self.swap_sticks {
            let left_rotation = self
                .sticks
                .iter()
                .find(|stick| stick.x_axis == left_x && stick.y_axis == left_y)
                .map(|stick| stick.rotation_degrees)
                .unwrap_or(0.0);
            let right_rotation = self
                .sticks
                .iter()
                .find(|stick| stick.x_axis == right_x && stick.y_axis == right_y)
                .map(|stick| stick.rotation_degrees)
                .unwrap_or(0.0);

            return vec![
                StickTransformMapping {
                    source_x: left_x,
                    source_y: left_y,
                    output_x: right_x,
                    output_y: right_y,
                    rotation_degrees: left_rotation,
                },
                StickTransformMapping {
                    source_x: right_x,
                    source_y: right_y,
                    output_x: left_x,
                    output_y: left_y,
                    rotation_degrees: right_rotation,
                },
            ];
        }

        self.sticks
            .iter()
            .map(|stick| StickTransformMapping {
                source_x: stick.x_axis,
                source_y: stick.y_axis,
                output_x: stick.x_axis,
                output_y: stick.y_axis,
                rotation_degrees: stick.rotation_degrees,
            })
            .collect()
    }

    pub fn stick_transform_sources(&self) -> HashMap<EventCode, Vec<usize>> {
        let mut sources = HashMap::<EventCode, Vec<usize>>::new();
        for (index, transform) in self.stick_transforms().iter().enumerate() {
            sources.entry(transform.source_x).or_default().push(index);
            sources.entry(transform.source_y).or_default().push(index);
        }
        sources
    }

    pub fn digital_stick_sources(&self) -> HashMap<EventCode, Vec<usize>> {
        let mut sources = HashMap::<EventCode, Vec<usize>>::new();
        for (index, stick) in self.digital_sticks.iter().enumerate() {
            sources.entry(stick.x_axis).or_default().push(index);
            sources.entry(stick.y_axis).or_default().push(index);
        }
        sources
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

        adjusted = self.apply_curve(adjusted, range.centered);
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

    pub fn zone_position(&self, value: i32) -> f64 {
        let Some(range) = AxisRange::for_event(self.code) else {
            return 0.0;
        };
        let normalized = normalize_axis_value(range, value);
        let magnitude = if range.centered {
            normalized.abs()
        } else {
            normalized
        };
        let deadzone = self.deadzone.clamp(0.0, 0.99);
        if magnitude <= deadzone {
            0.0
        } else {
            ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
        }
    }

    pub fn active_zone_indexes(&self, value: i32) -> Vec<usize> {
        let position = self.zone_position(value);
        self.zones
            .iter()
            .enumerate()
            .filter_map(|(index, zone)| {
                if zone.contains(position) {
                    Some(index)
                } else {
                    None
                }
            })
            .collect()
    }

    fn apply_curve(&self, value: f64, centered: bool) -> f64 {
        let exponent = match self.curve {
            AnalogCurve::Linear => 1.0,
            AnalogCurve::Soft => 0.5,
            AnalogCurve::Aggressive => 2.0,
            AnalogCurve::Custom => self.curve_exponent,
        }
        .clamp(0.25, 4.0);

        if centered {
            value.signum() * value.abs().powf(exponent)
        } else {
            value.clamp(0.0, 1.0).powf(exponent)
        }
    }
}

impl AnalogZoneMapping {
    pub fn contains(&self, position: f64) -> bool {
        position >= self.min && (position < self.max || self.max >= 1.0 && position <= self.max)
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

fn normalize_axis_value(range: AxisRange, value: i32) -> f64 {
    if range.centered {
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
    let outputs = raw
        .outputs
        .iter()
        .map(|value| normalize_output_type(value))
        .collect::<Vec<_>>();

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
        groups: raw.group,
        outputs,
        process_match: ProcessMatch {
            patterns: raw
                .process
                .map(RawStringOrList::into_vec)
                .unwrap_or_default()
                .into_iter()
                .map(|pattern| pattern.trim().to_string())
                .filter(|pattern| !pattern.is_empty())
                .collect(),
        },
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
        if mapping.command.is_some() && action_value.is_none() {
            action = MappingAction::Command;
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
        let command =
            parse_command_settings(mapping.command, action, from, &mapping.from, context)?;
        let activator = parse_mapping_activator(mapping.activator, from, &mapping.from, context)?;

        let to = match action {
            MappingAction::Map => {
                let to = to_value.ok_or_else(|| {
                    anyhow!("mapping from {} in {context} requires to", mapping.from)
                })?;
                let to = parse_event_code(to)
                    .ok_or_else(|| anyhow!("unknown target event {to} in {context}"))?;
                if !virtual_output_supports(to) {
                    return Err(anyhow!(
                        "target event {} in {context} is not supported by the current virtual output",
                        to.name()
                    ));
                }
                if to.kind == EventKind::Relative
                    && !matches!(from.kind, EventKind::Absolute | EventKind::Relative)
                {
                    return Err(anyhow!(
                        "relative target event {} in {context} requires an absolute-axis source or a macro event with value",
                        to.name()
                    ));
                }
                if from.kind == EventKind::Relative
                    && to.kind == EventKind::Absolute
                    && (!is_mouse_motion_axis(from) || !is_mouse_stick_axis(to))
                {
                    return Err(anyhow!(
                        "mouse-to-stick mapping from {} in {context} must use rel:x or rel:y as the source and abs:x, abs:y, abs:rx, or abs:ry as the centered virtual stick target",
                        mapping.from
                    ));
                }
                if from.kind == EventKind::Relative
                    && !matches!(to.kind, EventKind::Absolute | EventKind::Relative)
                {
                    return Err(anyhow!(
                        "relative source event {} in {context} can only map to a relative mouse axis or centered virtual stick axis",
                        mapping.from
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
            MappingAction::Command => from,
        };

        let turbo = parse_turbo(mapping.turbo, mapping.turbo_interval_ms)?;
        if turbo.is_some() {
            if activator.kind != ActivatorKind::Press {
                return Err(anyhow!(
                    "turbo mapping from {} in {context} can only use press activator",
                    mapping.from
                ));
            }
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
            if action == MappingAction::Command {
                return Err(anyhow!(
                    "command mapping from {} in {context} cannot use turbo",
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
        if matches!(
            macro_settings.as_ref().map(|settings| settings.mode),
            Some(MacroMode::Hold)
        ) && activator.kind != ActivatorKind::Press
        {
            return Err(anyhow!(
                "hold macro from {} in {context} can only use press activator",
                mapping.from
            ));
        }
        parsed.push(Mapping {
            from,
            to,
            from_name: from.name(),
            to_name: to.name(),
            action,
            activator,
            turbo,
            macro_settings,
            command,
        });
    }
    Ok(parsed)
}

fn is_mouse_motion_axis(event: EventCode) -> bool {
    matches!(event.name().as_str(), "rel:x" | "rel:y")
}

fn is_mouse_stick_axis(event: EventCode) -> bool {
    matches!(
        event.name().as_str(),
        "abs:x" | "abs:y" | "abs:rx" | "abs:ry"
    )
}

fn parse_mapping_action(value: Option<&str>) -> Result<MappingAction> {
    let Some(value) = value else {
        return Ok(MappingAction::Map);
    };

    match normalize_mapping_keyword(value).as_str() {
        "map" | "remap" | "virtual" | "controller" => Ok(MappingAction::Map),
        "disable" | "disabled" | "mute" | "block" | "none" => Ok(MappingAction::Disable),
        "macro" | "combo" | "sequence" => Ok(MappingAction::Macro),
        "command" | "cmd" => Ok(MappingAction::Command),
        other => Err(anyhow!("unknown mapping action {other}")),
    }
}

fn parse_mapping_activator(
    raw: Option<RawMappingActivator>,
    from: EventCode,
    from_name: &str,
    context: &str,
) -> Result<ActivatorSettings> {
    let Some(raw) = raw else {
        return Ok(ActivatorSettings::default());
    };

    let (kind_value, delay_ms) = match raw {
        RawMappingActivator::Scalar(value) => (value, None),
        RawMappingActivator::Object(object) => {
            let kind = object
                .kind
                .or(object.r#type)
                .or(object.mode)
                .ok_or_else(|| anyhow!("activator from {from_name} in {context} requires kind"))?;
            (
                kind,
                object.delay_ms.or(object.timeout_ms).or(object.interval_ms),
            )
        }
    };

    let kind = parse_activator_kind(&kind_value)?;
    if kind != ActivatorKind::Press && from.kind != EventKind::Key {
        return Err(anyhow!(
            "activator from {from_name} in {context} requires a button source"
        ));
    }

    let delay_ms = match kind {
        ActivatorKind::Press | ActivatorKind::Release => {
            if let Some(delay_ms) = delay_ms {
                if delay_ms != 0 {
                    return Err(anyhow!(
                        "activator from {from_name} in {context} cannot use delay_ms with {kind_value}"
                    ));
                }
            }
            0
        }
        ActivatorKind::LongPress => delay_ms.unwrap_or(DEFAULT_LONG_PRESS_MS),
        ActivatorKind::DoublePress | ActivatorKind::TriplePress => {
            delay_ms.unwrap_or(DEFAULT_MULTI_PRESS_MS)
        }
    };
    if matches!(
        kind,
        ActivatorKind::LongPress | ActivatorKind::DoublePress | ActivatorKind::TriplePress
    ) && !(50..=5000).contains(&delay_ms)
    {
        return Err(anyhow!(
            "activator from {from_name} in {context} delay_ms must be between 50 and 5000"
        ));
    }

    Ok(ActivatorSettings { kind, delay_ms })
}

fn parse_activator_kind(value: &str) -> Result<ActivatorKind> {
    match normalize_mapping_keyword(value).as_str() {
        "press" | "single" | "single_press" | "start_press" | "down_press" | "on_press" => {
            Ok(ActivatorKind::Press)
        }
        "release" | "release_press" | "up_press" | "on_release" => Ok(ActivatorKind::Release),
        "long" | "long_press" | "hold_press" => Ok(ActivatorKind::LongPress),
        "double" | "double_press" => Ok(ActivatorKind::DoublePress),
        "triple" | "triple_press" => Ok(ActivatorKind::TriplePress),
        other => Err(anyhow!("unknown mapping activator {other}")),
    }
}

fn parse_command_settings(
    raw: Option<RawCommandSettings>,
    action: MappingAction,
    from: EventCode,
    from_name: &str,
    context: &str,
) -> Result<Option<CommandSettings>> {
    if action != MappingAction::Command {
        if raw.is_some() {
            return Err(anyhow!(
                "mapping from {from_name} in {context} has a command but action is not command"
            ));
        }
        return Ok(None);
    }

    if from.kind != EventKind::Key {
        return Err(anyhow!(
            "command mapping from {from_name} in {context} requires a button source"
        ));
    }

    let raw = raw
        .ok_or_else(|| anyhow!("command mapping from {from_name} in {context} requires command"))?;
    let settings = match raw {
        RawCommandSettings::Scalar(value) => {
            let action = parse_command_action(&value)?;
            if action == CommandAction::RunCommand {
                return Err(anyhow!(
                    "run command mapping from {from_name} in {context} requires command_line"
                ));
            }
            CommandSettings {
                action,
                command_line: Vec::new(),
            }
        }
        RawCommandSettings::Object(object) => parse_command_object(object, from_name, context)?,
    };

    Ok(Some(settings))
}

fn command_action_keyword(action: CommandAction) -> &'static str {
    match action {
        CommandAction::StopMacros => "stop_macros",
        CommandAction::RunCommand => "run_command",
        CommandAction::RemapOff => "remap_off",
    }
}

fn parse_command_action(value: &str) -> Result<CommandAction> {
    match normalize_mapping_keyword(value).as_str() {
        "stop_macros" | "stop_all_macros" | "cancel_macros" | "cancel_all_macros"
        | "break_macros" | "clear_macro_queue" => Ok(CommandAction::StopMacros),
        "run" | "run_command" | "launch" | "launch_app" | "launch_application" | "exec"
        | "execute" => Ok(CommandAction::RunCommand),
        "remap_off"
        | "turn_remap_off"
        | "stop_remap"
        | "emergency_off"
        | "emergency_remap_off"
        | "off" => Ok(CommandAction::RemapOff),
        other => Err(anyhow!("unknown command action {other}")),
    }
}

fn parse_command_object(
    object: RawCommandObject,
    from_name: &str,
    context: &str,
) -> Result<CommandSettings> {
    let RawCommandObject {
        action,
        name,
        command,
        command_line,
        args,
        argv,
        program,
        executable,
        shell,
        run,
    } = object;

    let has_run_fields = command_line.is_some()
        || args.is_some()
        || argv.is_some()
        || program.is_some()
        || executable.is_some()
        || shell.is_some()
        || run.is_some();

    let action = match action.or(name) {
        Some(value) => parse_command_action(&value)?,
        None if has_run_fields => CommandAction::RunCommand,
        None => {
            let value = command.ok_or_else(|| {
                anyhow!("command mapping from {from_name} in {context} requires command action")
            })?;
            return Ok(CommandSettings {
                action: parse_command_action(&value)?,
                command_line: Vec::new(),
            });
        }
    };

    let command_line = match action {
        CommandAction::StopMacros | CommandAction::RemapOff => {
            if has_run_fields {
                return Err(anyhow!(
                    "{} command mapping from {from_name} in {context} cannot use command_line",
                    command_action_keyword(action)
                ));
            }
            Vec::new()
        }
        CommandAction::RunCommand => parse_run_command_line(
            command,
            command_line,
            args,
            argv,
            program,
            executable,
            shell,
            run,
            from_name,
            context,
        )?,
    };

    Ok(CommandSettings {
        action,
        command_line,
    })
}

fn parse_run_command_line(
    command: Option<String>,
    command_line: Option<RawCommandLine>,
    args: Option<Vec<String>>,
    argv: Option<Vec<String>>,
    program: Option<String>,
    executable: Option<String>,
    shell: Option<String>,
    run: Option<RawCommandLine>,
    from_name: &str,
    context: &str,
) -> Result<Vec<String>> {
    let mut candidates = Vec::new();
    if let Some(line) = run {
        candidates.push(raw_command_line_to_args(line));
    }
    if let Some(line) = command_line {
        candidates.push(raw_command_line_to_args(line));
    }
    if let Some(command) = command {
        candidates.push(shell_command_line(command));
    }
    if let Some(shell) = shell {
        candidates.push(shell_command_line(shell));
    }
    let executable = program.or(executable);
    match (executable, argv, args) {
        (Some(_), Some(_), Some(_)) => {
            return Err(anyhow!(
                "run command mapping from {from_name} in {context} cannot combine argv and args with program"
            ));
        }
        (Some(program), Some(mut argv), None) | (Some(program), None, Some(mut argv)) => {
            let mut command_line = vec![program];
            command_line.append(&mut argv);
            candidates.push(command_line);
        }
        (Some(program), None, None) => candidates.push(vec![program]),
        (None, Some(argv), None) | (None, None, Some(argv)) => candidates.push(argv),
        (None, None, None) => {}
        (None, Some(_), Some(_)) => {
            return Err(anyhow!(
                "run command mapping from {from_name} in {context} cannot combine argv and args"
            ));
        }
    }

    if candidates.len() != 1 {
        return Err(anyhow!(
            "run command mapping from {from_name} in {context} requires exactly one command_line, args, argv, program, executable, shell, command, or run value"
        ));
    }

    let mut command_line = candidates.remove(0);
    command_line.retain(|part| !part.is_empty());
    if command_line.is_empty() {
        return Err(anyhow!(
            "run command mapping from {from_name} in {context} requires a non-empty command_line"
        ));
    }
    Ok(command_line)
}

fn raw_command_line_to_args(raw: RawCommandLine) -> Vec<String> {
    match raw {
        RawCommandLine::Args(args) => args,
        RawCommandLine::Shell(command) => shell_command_line(command),
    }
}

fn shell_command_line(command: String) -> Vec<String> {
    vec!["sh".to_string(), "-c".to_string(), command]
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

    let (mode, raw_events, raw_release_events) = match (raw, raw_events) {
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
            raw.release_events,
        ),
        (None, Some(raw_events)) => (MacroMode::Press, raw_events, None),
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
        events.extend(parse_macro_event(raw_event, index, from_name, context)?);
    }

    let release_events = match (mode, raw_release_events) {
        (MacroMode::Press, Some(_)) => {
            return Err(anyhow!(
                "press macro from {from_name} in {context} cannot use release_events"
            ))
        }
        (MacroMode::Press, None) => Vec::new(),
        (MacroMode::Hold, Some(raw_release_events)) => {
            if raw_release_events.is_empty() {
                return Err(anyhow!(
                    "hold macro from {from_name} in {context} requires at least one release event"
                ));
            }
            let mut parsed = Vec::new();
            for (index, raw_event) in raw_release_events.into_iter().enumerate() {
                parsed.extend(parse_macro_event(raw_event, index, from_name, context)?);
            }
            parsed
        }
        (MacroMode::Hold, None) => automatic_hold_release_events(&events),
    };

    let duration_ms = macro_events_duration_ms(&events);
    let release_duration_ms = macro_events_duration_ms(&release_events);

    Ok(Some(MacroSettings {
        mode,
        events,
        release_events,
        duration_ms,
        release_duration_ms,
    }))
}

fn parse_macro_mode(value: &str) -> Result<MacroMode> {
    match normalize_mapping_keyword(value).as_str() {
        "press" | "single_press" | "on_press" | "execute" | "execute_at_once" => {
            Ok(MacroMode::Press)
        }
        "hold" | "held" | "hold_until_release" | "while_held" => Ok(MacroMode::Hold),
        other => Err(anyhow!("unknown macro mode {other}")),
    }
}

fn automatic_hold_release_events(events: &[MacroEvent]) -> Vec<MacroEvent> {
    let mut key_order = Vec::new();
    let mut key_down = HashMap::new();
    let mut axis_order = Vec::new();
    let mut axis_values = HashMap::new();

    for event in events {
        let Some(code) = event.code else {
            continue;
        };

        match event.kind {
            MacroEventKind::Down => {
                if !key_order.contains(&code) {
                    key_order.push(code);
                }
                key_down.insert(code, true);
            }
            MacroEventKind::Up => {
                if !key_order.contains(&code) {
                    key_order.push(code);
                }
                key_down.insert(code, false);
            }
            MacroEventKind::Axis => {
                if !axis_order.contains(&code) {
                    axis_order.push(code);
                }
                axis_values.insert(code, event.value);
            }
            MacroEventKind::Tap
            | MacroEventKind::Relative
            | MacroEventKind::Pause
            | MacroEventKind::Cancel => {}
        }
    }

    let mut release_events = Vec::new();
    for code in key_order {
        if key_down.get(&code).copied().unwrap_or(false) {
            release_events.push(MacroEvent {
                kind: MacroEventKind::Up,
                code: Some(code),
                code_name: code.name(),
                value: 0,
                pause_ms: 0,
            });
        }
    }

    for code in axis_order {
        if axis_values.get(&code).copied().unwrap_or(0) != 0 {
            release_events.push(MacroEvent {
                kind: MacroEventKind::Axis,
                code: Some(code),
                code_name: code.name(),
                value: 0,
                pause_ms: 0,
            });
        }
    }

    release_events
}

fn macro_events_duration_ms(events: &[MacroEvent]) -> u64 {
    let mut duration = 0_u64;
    for event in events {
        if event.kind == MacroEventKind::Cancel {
            break;
        }
        duration = duration.saturating_add(match event.kind {
            MacroEventKind::Tap => MACRO_TAP_RELEASE_MS,
            MacroEventKind::Pause => event.pause_ms,
            MacroEventKind::Down
            | MacroEventKind::Up
            | MacroEventKind::Axis
            | MacroEventKind::Relative
            | MacroEventKind::Cancel => 0,
        });
    }
    duration
}

fn parse_macro_event(
    raw: RawMacroEvent,
    index: usize,
    from_name: &str,
    context: &str,
) -> Result<Vec<MacroEvent>> {
    let specified = [
        raw.down.is_some(),
        raw.up.is_some(),
        raw.tap.is_some(),
        raw.axis.is_some(),
        raw.stick.is_some(),
        raw.rel.is_some(),
        raw.pause_ms.is_some(),
        raw.cancel.is_some(),
        raw.break_event.is_some(),
        raw.stop.is_some(),
    ]
    .into_iter()
    .filter(|specified| *specified)
    .count();

    if specified != 1 {
        return Err(anyhow!(
            "macro event {} from {from_name} in {context} must set exactly one of down, up, tap, axis, stick, rel, pause_ms, cancel, break, or stop",
            index + 1
        ));
    }

    if raw.stick.is_none() && (raw.x.is_some() || raw.y.is_some()) {
        return Err(anyhow!(
            "macro event {} from {from_name} in {context} can only use x or y with stick",
            index + 1
        ));
    }

    if raw.cancel.is_some() || raw.break_event.is_some() || raw.stop.is_some() {
        let enabled = raw.cancel.or(raw.break_event).or(raw.stop).unwrap_or(false);
        if !enabled {
            return Err(anyhow!(
                "macro cancel event {} from {from_name} in {context} must be true",
                index + 1
            ));
        }
        return Ok(vec![MacroEvent {
            kind: MacroEventKind::Cancel,
            code: None,
            code_name: String::new(),
            value: 0,
            pause_ms: 0,
        }]);
    }

    if let Some(pause_ms) = raw.pause_ms {
        if pause_ms > 60_000 {
            return Err(anyhow!(
                "macro pause_ms for event {} from {from_name} in {context} must be at most 60000",
                index + 1
            ));
        }
        return Ok(vec![MacroEvent {
            kind: MacroEventKind::Pause,
            code: None,
            code_name: String::new(),
            value: 0,
            pause_ms,
        }]);
    }

    if let Some(code) = raw.down {
        let code = parse_macro_target(&code, MacroEventKind::Down, index, from_name, context)?;
        return Ok(vec![MacroEvent {
            kind: MacroEventKind::Down,
            code: Some(code),
            code_name: code.name(),
            value: 1,
            pause_ms: 0,
        }]);
    }

    if let Some(code) = raw.up {
        let code = parse_macro_target(&code, MacroEventKind::Up, index, from_name, context)?;
        return Ok(vec![MacroEvent {
            kind: MacroEventKind::Up,
            code: Some(code),
            code_name: code.name(),
            value: 0,
            pause_ms: 0,
        }]);
    }

    if let Some(code) = raw.tap {
        let code = parse_macro_target(&code, MacroEventKind::Tap, index, from_name, context)?;
        return Ok(vec![MacroEvent {
            kind: MacroEventKind::Tap,
            code: Some(code),
            code_name: code.name(),
            value: 1,
            pause_ms: 0,
        }]);
    }

    if let Some(stick) = raw.stick {
        if raw.value.is_some() {
            return Err(anyhow!(
                "macro stick event {} from {from_name} in {context} must use x or y, not value",
                index + 1
            ));
        }
        return parse_macro_stick_event(&stick, raw.x, raw.y, index, from_name, context);
    }

    if let Some(code) = raw.rel {
        let code = parse_macro_target(&code, MacroEventKind::Relative, index, from_name, context)?;
        let value = raw.value.ok_or_else(|| {
            anyhow!(
                "macro relative event {} from {from_name} in {context} requires value",
                index + 1
            )
        })?;
        if !(-32768..=32767).contains(&value) {
            return Err(anyhow!(
                "macro relative value for event {} from {from_name} in {context} must stay within -32768..32767",
                index + 1
            ));
        }
        return Ok(vec![MacroEvent {
            kind: MacroEventKind::Relative,
            code: Some(code),
            code_name: code.name(),
            value,
            pause_ms: 0,
        }]);
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

    Ok(vec![MacroEvent {
        kind: MacroEventKind::Axis,
        code: Some(code),
        code_name: code.name(),
        value,
        pause_ms: 0,
    }])
}

fn parse_macro_stick_event(
    stick: &str,
    x: Option<f64>,
    y: Option<f64>,
    index: usize,
    from_name: &str,
    context: &str,
) -> Result<Vec<MacroEvent>> {
    let (x_axis_name, y_axis_name) = match normalize_mapping_keyword(stick).as_str() {
        "left" | "left_stick" | "ls" | "l" => ("abs:x", "abs:y"),
        "right" | "right_stick" | "rs" | "r" => ("abs:rx", "abs:ry"),
        other => {
            return Err(anyhow!(
                "macro stick event {} from {from_name} in {context} has unknown stick {other}",
                index + 1
            ))
        }
    };

    if x.is_none() && y.is_none() {
        return Err(anyhow!(
            "macro stick event {} from {from_name} in {context} requires x or y",
            index + 1
        ));
    }

    let mut events = Vec::new();
    if let Some(x) = x {
        events.push(macro_stick_axis_event(
            x_axis_name,
            x,
            "x",
            index,
            from_name,
            context,
        )?);
    }
    if let Some(y) = y {
        events.push(macro_stick_axis_event(
            y_axis_name,
            y,
            "y",
            index,
            from_name,
            context,
        )?);
    }
    Ok(events)
}

fn macro_stick_axis_event(
    axis_name: &str,
    unit_value: f64,
    component_name: &str,
    index: usize,
    from_name: &str,
    context: &str,
) -> Result<MacroEvent> {
    if !unit_value.is_finite() || !(-1.0..=1.0).contains(&unit_value) {
        return Err(anyhow!(
            "macro stick {component_name} value for event {} from {from_name} in {context} must stay within -1.0..1.0",
            index + 1
        ));
    }

    let axis = parse_event_code(axis_name).expect("built-in stick axis names are valid");
    Ok(MacroEvent {
        kind: MacroEventKind::Axis,
        code: Some(axis),
        code_name: axis.name(),
        value: stick_unit_to_axis_value(axis, unit_value),
        pause_ms: 0,
    })
}

fn stick_unit_to_axis_value(axis: EventCode, unit_value: f64) -> i32 {
    let range = AxisRange::for_event(axis).expect("stick axes are absolute axes");
    let max_abs = range.max.unsigned_abs().max(range.min.unsigned_abs()) as f64;
    (unit_value.clamp(-1.0, 1.0) * max_abs)
        .round()
        .clamp(range.min as f64, range.max as f64) as i32
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
    if !virtual_output_supports(code) {
        return Err(anyhow!(
            "macro target event {} for event {} from {from_name} in {context} is not supported by the current virtual output",
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
        MacroEventKind::Relative if code.kind != EventKind::Relative => Err(anyhow!(
            "macro relative event {} from {from_name} in {context} must target a relative axis",
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
    let raw_sticks = raw.sticks.or(raw.stick_rotations).or(raw.rotation);
    let raw_digital_sticks = raw.digital_sticks.or(raw.digital);
    let swap_sticks = raw.swap_sticks.or(raw.swap).unwrap_or(false);
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
        let (curve, curve_exponent) = parse_analog_curve(
            axis.curve.as_deref(),
            axis.curve_exponent.or(axis.exponent),
            &axis.code,
        )?;

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
        let zones = parse_analog_zones(axis.zones, &axis.code)?;

        axes.push(AnalogTuning {
            code,
            code_name: code.name(),
            deadzone,
            sensitivity,
            curve,
            curve_exponent,
            invert: axis.invert.unwrap_or(false),
            output_min,
            output_max,
            zones,
        });
    }

    let sticks = parse_stick_pair_settings(raw_sticks)?;
    let digital_sticks = parse_digital_sticks(raw_digital_sticks)?;

    Ok(AnalogSettings {
        axes,
        sticks,
        swap_sticks,
        digital_sticks,
    })
}

fn parse_analog_curve(
    curve: Option<&str>,
    exponent: Option<f64>,
    axis_name: &str,
) -> Result<(AnalogCurve, f64)> {
    let curve = match curve.map(normalize_mapping_keyword).as_deref() {
        None if exponent.is_some() => AnalogCurve::Custom,
        None | Some("linear") | Some("default") => AnalogCurve::Linear,
        Some("soft") | Some("smooth") | Some("relaxed") => AnalogCurve::Soft,
        Some("aggressive") | Some("precise") | Some("exponential") => AnalogCurve::Aggressive,
        Some("custom") => AnalogCurve::Custom,
        Some(other) => return Err(anyhow!("unknown analog curve {other} for {axis_name}")),
    };

    let curve_exponent = match curve {
        AnalogCurve::Linear => {
            if exponent.is_some() {
                return Err(anyhow!(
                    "analog curve_exponent for {axis_name} requires curve: custom"
                ));
            }
            1.0
        }
        AnalogCurve::Soft => {
            if exponent.is_some() {
                return Err(anyhow!(
                    "analog curve_exponent for {axis_name} requires curve: custom"
                ));
            }
            0.5
        }
        AnalogCurve::Aggressive => {
            if exponent.is_some() {
                return Err(anyhow!(
                    "analog curve_exponent for {axis_name} requires curve: custom"
                ));
            }
            2.0
        }
        AnalogCurve::Custom => exponent.ok_or_else(|| {
            anyhow!("custom analog curve for {axis_name} requires curve_exponent")
        })?,
    };

    if !(0.25..=4.0).contains(&curve_exponent) {
        return Err(anyhow!(
            "analog curve_exponent for {axis_name} must be between 0.25 and 4.0"
        ));
    }

    Ok((curve, curve_exponent))
}

fn parse_analog_zones(
    raw_zones: Option<Vec<RawAnalogZoneMapping>>,
    axis_name: &str,
) -> Result<Vec<AnalogZoneMapping>> {
    let mut zones = Vec::new();
    for raw in raw_zones.unwrap_or_default() {
        let name = raw.name.or(raw.zone).unwrap_or_else(|| {
            if raw.min.is_some() || raw.max.is_some() || raw.threshold.is_some() {
                "custom".to_string()
            } else {
                "high".to_string()
            }
        });
        let normalized_name = normalize_mapping_keyword(&name);
        let (default_min, default_max) = analog_zone_default_range(&normalized_name)?;
        let min = raw.threshold.or(raw.min).unwrap_or(default_min);
        let max = raw.max.unwrap_or(default_max);
        if !(0.0..=1.0).contains(&min) || !(0.0..=1.0).contains(&max) || min >= max {
            return Err(anyhow!(
                "analog zone {normalized_name} for {axis_name} must use min/max between 0.0 and 1.0"
            ));
        }

        let target_value =
            raw.to.or(raw.target).or(raw.output).ok_or_else(|| {
                anyhow!("analog zone {normalized_name} for {axis_name} requires to")
            })?;
        let target = parse_event_code(&target_value)
            .ok_or_else(|| anyhow!("unknown analog zone target {target_value} for {axis_name}"))?;
        if !virtual_output_supports(target) || target.kind != EventKind::Key {
            return Err(anyhow!(
                "analog zone target {} for {axis_name} must be a virtual button, keyboard key, or mouse button",
                target.name()
            ));
        }
        if zones
            .iter()
            .any(|existing: &AnalogZoneMapping| existing.target == target)
        {
            return Err(anyhow!(
                "duplicate analog zone target {} for {axis_name}",
                target.name()
            ));
        }

        zones.push(AnalogZoneMapping {
            name: normalized_name,
            min,
            max,
            target,
            target_name: target.name(),
        });
    }
    Ok(zones)
}

fn analog_zone_default_range(name: &str) -> Result<(f64, f64)> {
    match name {
        "low" => Ok((0.01, 0.33)),
        "medium" | "mid" => Ok((0.33, 0.66)),
        "high" => Ok((0.66, 1.0)),
        "custom" => Ok((0.5, 1.0)),
        other => Err(anyhow!("unknown analog zone {other}")),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum StickSide {
    Left,
    Right,
}

fn parse_stick_pair_settings(
    raw_sticks: Option<Vec<RawStickPairSettings>>,
) -> Result<Vec<StickPairSettings>> {
    let mut sticks = Vec::new();
    for (index, raw) in raw_sticks.unwrap_or_default().into_iter().enumerate() {
        let context = format!("stick {}", index + 1);
        let x_axis_name = raw
            .x_axis
            .or(raw.x)
            .ok_or_else(|| anyhow!("{context} requires x_axis"))?;
        let y_axis_name = raw
            .y_axis
            .or(raw.y)
            .ok_or_else(|| anyhow!("{context} requires y_axis"))?;
        let x_axis = parse_stick_horizontal_axis(&x_axis_name, &context)?;
        let y_axis = parse_stick_vertical_axis(&y_axis_name, &context)?;
        let side = stick_side_for_pair(x_axis, y_axis).ok_or_else(|| {
            anyhow!("{context} must use a matching left or right stick axis pair")
        })?;
        if sticks.iter().any(|existing: &StickPairSettings| {
            stick_side_for_pair(existing.x_axis, existing.y_axis) == Some(side)
        }) {
            return Err(anyhow!("{context} duplicates a stick rotation pair"));
        }

        let rotation_degrees = raw
            .rotation_degrees
            .or(raw.rotation)
            .or(raw.rotate)
            .unwrap_or(0.0);
        if !(-180.0..=180.0).contains(&rotation_degrees) {
            return Err(anyhow!(
                "{context} rotation_degrees must be between -180 and 180"
            ));
        }

        sticks.push(StickPairSettings {
            x_axis,
            x_axis_name: x_axis.name(),
            y_axis,
            y_axis_name: y_axis.name(),
            rotation_degrees,
        });
    }
    Ok(sticks)
}

fn parse_stick_horizontal_axis(value: &str, context: &str) -> Result<EventCode> {
    let code = parse_event_code(value).ok_or_else(|| anyhow!("unknown {context} axis {value}"))?;
    match code.name().as_str() {
        "abs:x" | "abs:rx" => Ok(code),
        _ => Err(anyhow!("{context} x_axis must be abs:x or abs:rx")),
    }
}

fn parse_stick_vertical_axis(value: &str, context: &str) -> Result<EventCode> {
    let code = parse_event_code(value).ok_or_else(|| anyhow!("unknown {context} axis {value}"))?;
    match code.name().as_str() {
        "abs:y" | "abs:ry" => Ok(code),
        _ => Err(anyhow!("{context} y_axis must be abs:y or abs:ry")),
    }
}

fn stick_side_for_pair(x_axis: EventCode, y_axis: EventCode) -> Option<StickSide> {
    match (x_axis.name().as_str(), y_axis.name().as_str()) {
        ("abs:x", "abs:y") => Some(StickSide::Left),
        ("abs:rx", "abs:ry") => Some(StickSide::Right),
        _ => None,
    }
}

fn parse_digital_sticks(
    raw_sticks: Option<Vec<RawDigitalStickMapping>>,
) -> Result<Vec<DigitalStickMapping>> {
    let mut sticks = Vec::new();
    for (index, raw) in raw_sticks.unwrap_or_default().into_iter().enumerate() {
        let context = format!("digital stick {}", index + 1);
        let x_axis_name = raw
            .x_axis
            .or(raw.x)
            .ok_or_else(|| anyhow!("{context} requires x_axis"))?;
        let y_axis_name = raw
            .y_axis
            .or(raw.y)
            .ok_or_else(|| anyhow!("{context} requires y_axis"))?;
        let x_axis = parse_centered_analog_axis(&x_axis_name, &context)?;
        let y_axis = parse_centered_analog_axis(&y_axis_name, &context)?;
        if x_axis == y_axis {
            return Err(anyhow!("{context} requires different x_axis and y_axis"));
        }

        let threshold = raw.threshold.unwrap_or(0.5);
        if !(0.01..=1.0).contains(&threshold) {
            return Err(anyhow!("{context} threshold must be between 0.01 and 1.0"));
        }

        let output_x_name = raw.output_x.unwrap_or_else(|| "abs:hat0x".to_string());
        let output_y_name = raw.output_y.unwrap_or_else(|| "abs:hat0y".to_string());
        let output_x = parse_hat_output_axis(&output_x_name, "abs:hat0x", &context)?;
        let output_y = parse_hat_output_axis(&output_y_name, "abs:hat0y", &context)?;

        sticks.push(DigitalStickMapping {
            x_axis,
            x_axis_name: x_axis.name(),
            y_axis,
            y_axis_name: y_axis.name(),
            threshold,
            output_x,
            output_x_name: output_x.name(),
            output_y,
            output_y_name: output_y.name(),
        });
    }
    Ok(sticks)
}

fn parse_centered_analog_axis(value: &str, context: &str) -> Result<EventCode> {
    let code = parse_event_code(value).ok_or_else(|| anyhow!("unknown {context} axis {value}"))?;
    let range = AxisRange::for_event(code)
        .ok_or_else(|| anyhow!("{context} axis {} must be an absolute axis", code.name()))?;
    if !range.centered {
        return Err(anyhow!("{context} axis {} must be centered", code.name()));
    }
    if !virtual_xbox_supports(code) {
        return Err(anyhow!(
            "{context} axis {} is not supported by the current virtual pad",
            code.name()
        ));
    }
    Ok(code)
}

fn parse_hat_output_axis(value: &str, expected: &str, context: &str) -> Result<EventCode> {
    let code =
        parse_event_code(value).ok_or_else(|| anyhow!("unknown {context} output axis {value}"))?;
    if code.kind != EventKind::Absolute || code.name() != expected {
        return Err(anyhow!("{context} output axis must be {expected}"));
    }
    Ok(code)
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
        parse_profile_bytes, ActivatorKind, AnalogCurve, AnalogTuning, CommandAction,
        LayerActivationMode, MacroEventKind, MacroMode, MappingAction, DEFAULT_LONG_PRESS_MS,
        DEFAULT_MULTI_PRESS_MS, DEFAULT_TURBO_INTERVAL_MS, MAIN_LAYER_ID, MAX_SHIFT_LAYERS,
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
    fn parses_mapping_activators() {
        let profile = parse_profile_bytes(
            br#"
id: activators
mappings:
  - from: btn:south
    to: btn:east
  - from: btn:east
    to: btn:south
    activator: release
  - from: btn:west
    to: btn:north
    activator:
      kind: long_press
  - from: btn:north
    to: btn:west
    activator:
      kind: double_press
      timeout_ms: 225
  - from: btn:tl
    to: btn:tr
    activator:
      kind: triple_press
      interval_ms: 400
  - from: btn:start
    to: btn:select
    activator: double_press
"#,
            Path::new("activators.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings[0].activator.kind, ActivatorKind::Press);
        assert_eq!(profile.mappings[0].activator.delay_ms, 0);
        assert_eq!(profile.mappings[1].activator.kind, ActivatorKind::Release);
        assert_eq!(profile.mappings[1].activator.delay_ms, 0);
        assert_eq!(profile.mappings[2].activator.kind, ActivatorKind::LongPress);
        assert_eq!(
            profile.mappings[2].activator.delay_ms,
            DEFAULT_LONG_PRESS_MS
        );
        assert_eq!(
            profile.mappings[3].activator.kind,
            ActivatorKind::DoublePress
        );
        assert_eq!(profile.mappings[3].activator.delay_ms, 225);
        assert_eq!(
            profile.mappings[4].activator.kind,
            ActivatorKind::TriplePress
        );
        assert_eq!(profile.mappings[4].activator.delay_ms, 400);
        assert_eq!(
            profile.mappings[5].activator.kind,
            ActivatorKind::DoublePress
        );
        assert_eq!(
            profile.mappings[5].activator.delay_ms,
            DEFAULT_MULTI_PRESS_MS
        );
    }

    #[test]
    fn rejects_invalid_mapping_activators() {
        let axis_source = parse_profile_bytes(
            br#"
id: axis-activator
mappings:
  - from: abs:x
    to: abs:y
    activator: long_press
"#,
            Path::new("axis-activator.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            axis_source.contains("requires a button source"),
            "{axis_source}"
        );

        let turbo = parse_profile_bytes(
            br#"
id: turbo-activator
mappings:
  - from: btn:south
    to: btn:east
    activator: double_press
    turbo: true
"#,
            Path::new("turbo-activator.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(turbo.contains("can only use press activator"), "{turbo}");

        let hold_macro = parse_profile_bytes(
            br#"
id: hold-macro-activator
mappings:
  - from: btn:tl
    action: macro
    activator: release
    macro:
      mode: hold
      events:
        - down: btn:south
"#,
            Path::new("hold-macro-activator.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            hold_macro.contains("can only use press activator"),
            "{hold_macro}"
        );

        let press_delay = parse_profile_bytes(
            br#"
id: press-delay
mappings:
  - from: btn:south
    to: btn:east
    activator:
      kind: press
      delay_ms: 50
"#,
            Path::new("press-delay.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(press_delay.contains("cannot use delay_ms"), "{press_delay}");

        let delay_range = parse_profile_bytes(
            br#"
id: delay-range
mappings:
  - from: btn:south
    to: btn:east
    activator:
      kind: long_press
      delay_ms: 10
"#,
            Path::new("delay-range.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            delay_range.contains("delay_ms must be between"),
            "{delay_range}"
        );
    }

    #[test]
    fn parses_command_mappings() {
        let profile = parse_profile_bytes(
            br#"
id: commands
mappings:
  - from: btn:select
    action: command
    command: stop_macros
  - from: btn:mode
    command:
      action: cancel_all_macros
  - from: btn:start
    command:
      action: run
      args: ["printf", "hello"]
  - from: btn:tl
    command:
      action: run
      program: notify-send
      args: ["PadProxy", "mapping fired"]
"#,
            Path::new("commands.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings.len(), 4);
        assert_eq!(profile.mappings[0].action, MappingAction::Command);
        assert_eq!(profile.mappings[0].to_name, "btn:select");
        assert!(profile.mapping_table().is_empty());
        assert_eq!(
            profile.mappings[0].command.as_ref().unwrap().action,
            CommandAction::StopMacros
        );
        assert_eq!(
            profile.mappings[1].command.as_ref().unwrap().action,
            CommandAction::StopMacros
        );
        let run_command = profile.mappings[2].command.as_ref().unwrap();
        assert_eq!(run_command.action, CommandAction::RunCommand);
        assert_eq!(run_command.command_line, vec!["printf", "hello"]);
        let program_command = profile.mappings[3].command.as_ref().unwrap();
        assert_eq!(program_command.action, CommandAction::RunCommand);
        assert_eq!(
            program_command.command_line,
            vec!["notify-send", "PadProxy", "mapping fired"]
        );
    }

    #[test]
    fn parses_remap_off_command_mappings() {
        let profile = parse_profile_bytes(
            br#"
id: emergency
mappings:
  - from: btn:mode
    action: command
    command: remap_off
  - from: btn:select
    command:
      action: turn_remap_off
"#,
            Path::new("emergency.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings.len(), 2);
        for mapping in &profile.mappings {
            assert_eq!(mapping.action, MappingAction::Command);
            let command = mapping.command.as_ref().unwrap();
            assert_eq!(command.action, CommandAction::RemapOff);
            assert!(command.command_line.is_empty());
        }
    }

    #[test]
    fn parses_additional_virtual_outputs() {
        let profile = parse_profile_bytes(
            br#"
id: dual-out
output:
  type: xbox360
outputs:
  - DualShock 4
  - switchpro
mappings: []
"#,
            Path::new("dual-out.yaml"),
        )
        .unwrap();
        assert_eq!(profile.output_type, "xbox360");
        assert_eq!(profile.outputs, vec!["ds4", "switchpro"]);
    }

    #[test]
    fn resolves_grouped_device_paths() {
        use super::{resolve_group_paths, DeviceMatch};
        use crate::devices::DeviceInfo;

        fn device(path: &str, name: &str, vendor: u16) -> DeviceInfo {
            DeviceInfo {
                id: format!("id:{path}"),
                name: name.to_string(),
                path: path.to_string(),
                device_kind: "physical".to_string(),
                phys: String::new(),
                uniq: String::new(),
                bus: 0,
                vendor,
                product: 0,
                version: 0,
                capabilities: Vec::new(),
            }
        }

        let devices = vec![
            device("/dev/input/event1", "Wireless Controller", 0x054c),
            device("/dev/input/event2", "Wireless Controller", 0x054c),
            device("/dev/input/event3", "Keyboard", 0x1234),
        ];

        let profile = parse_profile_bytes(
            br#"
id: grouped
match:
  name: "Wireless Controller"
group:
  - name: "Wireless Controller"
  - name: "Keyboard"
mappings: []
"#,
            Path::new("grouped.yaml"),
        )
        .unwrap();
        assert_eq!(profile.groups.len(), 2);

        // The primary controller (event1) is excluded; the group resolves the
        // second controller and the keyboard, each distinct.
        let paths = resolve_group_paths(
            &profile.groups,
            &devices,
            &["/dev/input/event1".to_string()],
        );
        assert_eq!(paths, vec!["/dev/input/event2", "/dev/input/event3"]);

        // With no available second controller, an unmatched matcher is skipped.
        let only_keyboard = vec![device("/dev/input/event3", "Keyboard", 0x1234)];
        let matchers = vec![DeviceMatch {
            name: Some("Wireless Controller".to_string()),
            ..Default::default()
        }];
        assert!(resolve_group_paths(&matchers, &only_keyboard, &[]).is_empty());
    }

    #[test]
    fn rejects_remap_off_with_command_line() {
        let error = parse_profile_bytes(
            br#"
id: bad-emergency
mappings:
  - from: btn:mode
    command:
      action: remap_off
      args: ["echo", "nope"]
"#,
            Path::new("bad-emergency.yaml"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("remap_off"));
    }

    #[test]
    fn parses_keyboard_mouse_mapping_targets_and_relative_macros() {
        let profile = parse_profile_bytes(
            br#"
id: keyboard-mouse
mappings:
  - from: btn:south
    to: key:space
  - from: btn:east
    to: mouse:left
  - from: abs:x
    to: rel:x
  - from: btn:start
    action: macro
    macro:
      events:
        - tap: key:enter
        - tap: mouse:right
        - rel: rel:wheel
          value: -1
"#,
            Path::new("keyboard-mouse.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings[0].to_name, "key:space");
        assert_eq!(profile.mappings[1].to_name, "mouse:left");
        assert_eq!(profile.mappings[2].from_name, "abs:x");
        assert_eq!(profile.mappings[2].to_name, "rel:x");
        let macro_settings = profile.mappings[3].macro_settings.as_ref().unwrap();
        assert_eq!(macro_settings.events[0].code_name, "key:enter");
        assert_eq!(macro_settings.events[1].code_name, "mouse:right");
        assert_eq!(macro_settings.events[2].kind, MacroEventKind::Relative);
        assert_eq!(macro_settings.events[2].code_name, "rel:wheel");
        assert_eq!(macro_settings.events[2].value, -1);
    }

    #[test]
    fn parses_mouse_to_stick_mappings() {
        let profile = parse_profile_bytes(
            br#"
id: mouse-to-stick
mappings:
  - from: rel:x
    to: abs:rx
  - from: rel:y
    to: abs:ry
"#,
            Path::new("mouse-to-stick.yaml"),
        )
        .unwrap();

        assert_eq!(profile.mappings[0].from_name, "rel:x");
        assert_eq!(profile.mappings[0].to_name, "abs:rx");
        assert_eq!(profile.mappings[1].from_name, "rel:y");
        assert_eq!(profile.mappings[1].to_name, "abs:ry");

        let wheel_error = parse_profile_bytes(
            br#"
id: bad-mouse-to-stick-source
mappings:
  - from: rel:wheel
    to: abs:x
"#,
            Path::new("bad-mouse-to-stick-source.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(wheel_error.contains("rel:x or rel:y"), "{wheel_error}");

        let target_error = parse_profile_bytes(
            br#"
id: bad-mouse-to-stick-target
mappings:
  - from: rel:x
    to: abs:z
"#,
            Path::new("bad-mouse-to-stick-target.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            target_error.contains("centered virtual stick"),
            "{target_error}"
        );

        let button_error = parse_profile_bytes(
            br#"
id: bad-relative-button-target
mappings:
  - from: rel:x
    to: btn:south
"#,
            Path::new("bad-relative-button-target.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(button_error.contains("relative source"), "{button_error}");
    }

    #[test]
    fn rejects_relative_direct_mapping_targets() {
        let error = parse_profile_bytes(
            br#"
id: relative-target
mappings:
  - from: btn:south
    to: rel:x
"#,
            Path::new("relative-target.yaml"),
        )
        .unwrap_err()
        .to_string();

        assert!(error.contains("absolute-axis source"), "{error}");
    }

    #[test]
    fn rejects_invalid_command_mappings() {
        let missing_command = parse_profile_bytes(
            br#"
id: missing-command
mappings:
  - from: btn:south
    action: command
"#,
            Path::new("missing-command.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            missing_command.contains("requires command"),
            "{missing_command}"
        );

        let axis_source = parse_profile_bytes(
            br#"
id: command-axis-source
mappings:
  - from: abs:x
    action: command
    command: stop_macros
"#,
            Path::new("command-axis-source.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            axis_source.contains("requires a button source"),
            "{axis_source}"
        );

        let turbo_error = parse_profile_bytes(
            br#"
id: command-turbo
mappings:
  - from: btn:south
    action: command
    command: stop_macros
    turbo: true
"#,
            Path::new("command-turbo.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(turbo_error.contains("cannot use turbo"), "{turbo_error}");

        let unknown = parse_profile_bytes(
            br#"
id: unknown-command
mappings:
  - from: btn:south
    action: command
    command: launch_missiles
"#,
            Path::new("unknown-command.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(unknown.contains("unknown command action"), "{unknown}");

        let missing_run_command_line = parse_profile_bytes(
            br#"
id: missing-run-command-line
mappings:
  - from: btn:south
    action: command
    command: run
"#,
            Path::new("missing-run-command-line.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            missing_run_command_line.contains("requires command_line"),
            "{missing_run_command_line}"
        );
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
        assert!(macro_settings.release_events.is_empty());
        assert_eq!(macro_settings.events[0].kind, MacroEventKind::Down);
        assert_eq!(macro_settings.events[0].code_name, "btn:south");
        assert_eq!(macro_settings.events[1].kind, MacroEventKind::Pause);
        assert_eq!(macro_settings.events[1].pause_ms, 40);
        assert_eq!(macro_settings.events[3].kind, MacroEventKind::Tap);
        assert_eq!(macro_settings.events[4].kind, MacroEventKind::Axis);
        assert_eq!(macro_settings.events[4].code_name, "abs:x");
        assert_eq!(macro_settings.events[4].value, 12000);
        assert_eq!(macro_settings.duration_ms, 80);
        assert_eq!(macro_settings.release_duration_ms, 0);
    }

    #[test]
    fn parses_stick_deflection_macro_events() {
        let profile = parse_profile_bytes(
            br#"
id: stick-macro
mappings:
  - from: btn:start
    action: macro
    macro:
      mode: press
      events:
        - stick: left
          x: 0.5
          y: -0.25
        - pause_ms: 25
        - stick: right
          x: -1.0
        - stick: right
          x: 0.0
"#,
            Path::new("stick-macro.yaml"),
        )
        .unwrap();

        let macro_settings = profile.mappings[0].macro_settings.as_ref().unwrap();
        assert_eq!(macro_settings.events.len(), 5);
        assert_eq!(macro_settings.events[0].kind, MacroEventKind::Axis);
        assert_eq!(macro_settings.events[0].code_name, "abs:x");
        assert_eq!(macro_settings.events[0].value, 16384);
        assert_eq!(macro_settings.events[1].code_name, "abs:y");
        assert_eq!(macro_settings.events[1].value, -8192);
        assert_eq!(macro_settings.events[3].code_name, "abs:rx");
        assert_eq!(macro_settings.events[3].value, -32768);
        assert_eq!(macro_settings.events[4].code_name, "abs:rx");
        assert_eq!(macro_settings.events[4].value, 0);
        assert_eq!(macro_settings.duration_ms, 25);
    }

    #[test]
    fn parses_macro_cancel_events() {
        let profile = parse_profile_bytes(
            br#"
id: macro-cancel
mappings:
  - from: btn:start
    action: macro
    macro:
      events:
        - down: btn:south
        - pause_ms: 100
        - cancel: true
        - tap: btn:east
  - from: btn:select
    action: macro
    macro:
      events:
        - break: true
  - from: btn:mode
    action: macro
    macro:
      events:
        - stop: true
"#,
            Path::new("macro-cancel.yaml"),
        )
        .unwrap();

        let macro_settings = profile.mappings[0].macro_settings.as_ref().unwrap();
        assert_eq!(macro_settings.events.len(), 4);
        assert_eq!(macro_settings.events[2].kind, MacroEventKind::Cancel);
        assert_eq!(macro_settings.duration_ms, 100);
        assert_eq!(
            profile.mappings[1].macro_settings.as_ref().unwrap().events[0].kind,
            MacroEventKind::Cancel
        );
        assert_eq!(
            profile.mappings[2].macro_settings.as_ref().unwrap().events[0].kind,
            MacroEventKind::Cancel
        );

        let error = parse_profile_bytes(
            br#"
id: bad-macro-cancel
mappings:
  - from: btn:start
    action: macro
    macro:
      events:
        - cancel: false
"#,
            Path::new("bad-macro-cancel.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("must be true"), "{error}");
    }

    #[test]
    fn parses_hold_macro_release_events() {
        let explicit = parse_profile_bytes(
            br#"
id: hold-explicit
mappings:
  - from: btn:tl
    action: macro
    macro:
      mode: hold
      events:
        - down: btn:south
        - axis: abs:x
          value: 12000
      release_events:
        - axis: abs:x
          value: 0
        - up: btn:south
"#,
            Path::new("hold-explicit.yaml"),
        )
        .unwrap();

        let macro_settings = explicit.mappings[0].macro_settings.as_ref().unwrap();
        assert_eq!(macro_settings.mode, MacroMode::Hold);
        assert_eq!(macro_settings.events.len(), 2);
        assert_eq!(macro_settings.release_events.len(), 2);
        assert_eq!(macro_settings.duration_ms, 0);
        assert_eq!(macro_settings.release_duration_ms, 0);
        assert_eq!(macro_settings.release_events[0].kind, MacroEventKind::Axis);
        assert_eq!(macro_settings.release_events[0].code_name, "abs:x");
        assert_eq!(macro_settings.release_events[0].value, 0);
        assert_eq!(macro_settings.release_events[1].kind, MacroEventKind::Up);
        assert_eq!(macro_settings.release_events[1].code_name, "btn:south");

        let automatic = parse_profile_bytes(
            br#"
id: hold-automatic
mappings:
  - from: btn:tr
    macro:
      mode: hold
      events:
        - tap: btn:east
        - down: btn:south
        - axis: abs:y
          value: -15000
"#,
            Path::new("hold-automatic.yaml"),
        )
        .unwrap();

        let macro_settings = automatic.mappings[0].macro_settings.as_ref().unwrap();
        assert_eq!(macro_settings.mode, MacroMode::Hold);
        assert_eq!(macro_settings.release_events.len(), 2);
        assert_eq!(macro_settings.release_events[0].kind, MacroEventKind::Up);
        assert_eq!(macro_settings.release_events[0].code_name, "btn:south");
        assert_eq!(macro_settings.release_events[1].kind, MacroEventKind::Axis);
        assert_eq!(macro_settings.release_events[1].code_name, "abs:y");
        assert_eq!(macro_settings.release_events[1].value, 0);
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

        let bad_stick_value = parse_profile_bytes(
            br#"
id: bad-stick-macro
mappings:
  - from: btn:start
    action: macro
    macro:
      events:
        - stick: left
          x: 1.5
"#,
            Path::new("bad-stick-macro.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            bad_stick_value.contains("must stay within -1.0..1.0"),
            "{bad_stick_value}"
        );

        let press_release_error = parse_profile_bytes(
            br#"
id: press-release
mappings:
  - from: btn:start
    action: macro
    macro:
      mode: press
      events:
        - tap: btn:south
      release_events:
        - up: btn:south
"#,
            Path::new("press-release.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(
            press_release_error.contains("cannot use release_events"),
            "{press_release_error}"
        );
    }

    #[test]
    fn parses_analog_axis_tuning() {
        let profile = parse_profile_bytes(
            br#"
id: analog
analog:
  swap_sticks: true
  axes:
    - code: abs:x
      deadzone: 0.2
      sensitivity: 1.5
      curve: aggressive
      invert: true
      output_min: -20000
      output_max: 20000
    - code: abs:z
      deadzone: 0.1
      curve: custom
      curve_exponent: 0.75
      max: 200
      zones:
        - name: low
          to: btn:south
        - name: high
          min: 0.70
          to: key:space
  sticks:
    - x_axis: abs:x
      y_axis: abs:y
      rotation_degrees: 15
  digital_sticks:
    - x_axis: abs:x
      y_axis: abs:y
      threshold: 0.45
"#,
            Path::new("analog.yaml"),
        )
        .unwrap();

        assert_eq!(profile.analog.axes.len(), 2);
        assert_eq!(profile.analog.axes[0].code_name, "abs:x");
        assert_eq!(profile.analog.axes[0].deadzone, 0.2);
        assert_eq!(profile.analog.axes[0].sensitivity, 1.5);
        assert_eq!(profile.analog.axes[0].curve, AnalogCurve::Aggressive);
        assert_eq!(profile.analog.axes[0].curve_exponent, 2.0);
        assert!(profile.analog.axes[0].invert);
        assert_eq!(profile.analog.axes[0].output_min, -20000);
        assert_eq!(profile.analog.axes[1].code_name, "abs:z");
        assert_eq!(profile.analog.axes[1].curve, AnalogCurve::Custom);
        assert_eq!(profile.analog.axes[1].curve_exponent, 0.75);
        assert_eq!(profile.analog.axes[1].zones.len(), 2);
        assert_eq!(profile.analog.axes[1].zones[0].name, "low");
        assert_eq!(profile.analog.axes[1].zones[0].target_name, "btn:south");
        assert_eq!(profile.analog.axes[1].zones[1].min, 0.70);
        assert_eq!(profile.analog.axes[1].zones[1].target_name, "key:space");
        assert_eq!(profile.analog.tuning_table().len(), 2);
        assert!(profile.analog.swap_sticks);
        assert_eq!(profile.analog.sticks.len(), 1);
        assert_eq!(profile.analog.sticks[0].x_axis_name, "abs:x");
        assert_eq!(profile.analog.sticks[0].y_axis_name, "abs:y");
        assert_eq!(profile.analog.sticks[0].rotation_degrees, 15.0);
        let transforms = profile.analog.stick_transforms();
        assert_eq!(transforms.len(), 2);
        assert_eq!(transforms[0].source_x.name(), "abs:x");
        assert_eq!(transforms[0].output_x.name(), "abs:rx");
        assert_eq!(transforms[0].rotation_degrees, 15.0);
        assert_eq!(transforms[1].source_x.name(), "abs:rx");
        assert_eq!(transforms[1].output_x.name(), "abs:x");
        assert_eq!(profile.analog.digital_sticks.len(), 1);
        assert_eq!(profile.analog.digital_sticks[0].x_axis_name, "abs:x");
        assert_eq!(profile.analog.digital_sticks[0].y_axis_name, "abs:y");
        assert_eq!(profile.analog.digital_sticks[0].threshold, 0.45);
        assert_eq!(profile.analog.digital_sticks[0].output_x_name, "abs:hat0x");
        assert_eq!(profile.analog.digital_sticks[0].output_y_name, "abs:hat0y");
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

        let error = parse_profile_bytes(
            br#"
id: bad-curve
analog:
  axes:
    - code: abs:x
      curve: custom
"#,
            Path::new("bad-curve.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("requires curve_exponent"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-curve-exponent
analog:
  axes:
    - code: abs:x
      curve: linear
      curve_exponent: 2.0
"#,
            Path::new("bad-curve-exponent.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("requires curve: custom"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-zone-target
analog:
  axes:
    - code: abs:z
      zones:
        - name: high
          to: rel:x
"#,
            Path::new("bad-zone-target.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("virtual button"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-zone-range
analog:
  axes:
    - code: abs:z
      zones:
        - name: custom
          min: 0.8
          max: 0.2
          to: btn:south
"#,
            Path::new("bad-zone-range.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("min/max"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-stick-axis
analog:
  sticks:
    - x_axis: abs:z
      y_axis: abs:y
"#,
            Path::new("bad-stick-axis.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("x_axis"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-stick-pair
analog:
  sticks:
    - x_axis: abs:x
      y_axis: abs:ry
"#,
            Path::new("bad-stick-pair.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("matching left or right"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-stick-rotation
analog:
  sticks:
    - x_axis: abs:x
      y_axis: abs:y
      rotation_degrees: 360
"#,
            Path::new("bad-stick-rotation.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("rotation_degrees"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-digital-stick-trigger
analog:
  digital_sticks:
    - x_axis: abs:z
      y_axis: abs:y
"#,
            Path::new("bad-digital-stick-trigger.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("must be centered"), "{error}");

        let error = parse_profile_bytes(
            br#"
id: bad-digital-stick-threshold
analog:
  digital_sticks:
    - x_axis: abs:x
      y_axis: abs:y
      threshold: 1.5
"#,
            Path::new("bad-digital-stick-threshold.yaml"),
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("threshold"), "{error}");
    }

    #[test]
    fn applies_analog_tuning_to_sticks_and_triggers() {
        let stick = AnalogTuning {
            code: parse_event_code("abs:x").unwrap(),
            code_name: "abs:x".to_string(),
            deadzone: 0.2,
            sensitivity: 1.5,
            curve: AnalogCurve::Linear,
            curve_exponent: 1.0,
            invert: true,
            output_min: -20000,
            output_max: 20000,
            zones: Vec::new(),
        };
        assert_eq!(stick.apply(3000), 0);
        assert!(stick.apply(16_384) < -18_000);
        assert_eq!(stick.apply(32_767), -20000);

        let trigger = AnalogTuning {
            code: parse_event_code("abs:z").unwrap(),
            code_name: "abs:z".to_string(),
            deadzone: 0.1,
            sensitivity: 1.0,
            curve: AnalogCurve::Linear,
            curve_exponent: 1.0,
            invert: false,
            output_min: 0,
            output_max: 200,
            zones: vec![
                super::AnalogZoneMapping {
                    name: "low".to_string(),
                    min: 0.01,
                    max: 0.33,
                    target: parse_event_code("btn:south").unwrap(),
                    target_name: "btn:south".to_string(),
                },
                super::AnalogZoneMapping {
                    name: "high".to_string(),
                    min: 0.66,
                    max: 1.0,
                    target: parse_event_code("key:space").unwrap(),
                    target_name: "key:space".to_string(),
                },
            ],
        };
        assert_eq!(trigger.apply(10), 0);
        assert_eq!(trigger.apply(255), 200);
        assert!(trigger.active_zone_indexes(10).is_empty());
        assert_eq!(trigger.active_zone_indexes(80), vec![0]);
        assert_eq!(trigger.active_zone_indexes(255), vec![1]);

        let aggressive = AnalogTuning {
            code: parse_event_code("abs:x").unwrap(),
            code_name: "abs:x".to_string(),
            deadzone: 0.0,
            sensitivity: 1.0,
            curve: AnalogCurve::Aggressive,
            curve_exponent: 2.0,
            invert: false,
            output_min: -32768,
            output_max: 32767,
            zones: Vec::new(),
        };
        assert!(aggressive.apply(16_384) < 10_000);

        let soft = AnalogTuning {
            curve: AnalogCurve::Soft,
            curve_exponent: 0.5,
            ..aggressive.clone()
        };
        assert!(soft.apply(16_384) > 20_000);
    }
}
