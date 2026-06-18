use crate::event_code::{event_from_input, virtual_output_supports, EventCode, EventKind};
use crate::outputs::{output_device, supported_output_ids};
use crate::profiles::{
    ActivatorKind, ActivatorSettings, AnalogTuning, CommandAction, CommandSettings,
    DigitalStickMapping, LayerActivation, LayerActivationMode, MacroEvent, MacroEventKind,
    MacroMode, MacroSettings, Mapping, MappingAction, Profile, StickTransformMapping,
};
use anyhow::{anyhow, Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, Device, EventType, InputEvent, InputId,
    KeyCode, RelativeAxisCode, UinputAbsSetup,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

const RELATIVE_AXIS_INTERVAL: Duration = Duration::from_millis(8);
const RELATIVE_AXIS_MAX_STEP: f64 = 20.0;
const DEFAULT_RELATIVE_AXIS_DEADZONE: f64 = 0.12;

pub struct LaunchOptions {
    pub profile: Profile,
    pub source_device_path: String,
    pub command: Vec<String>,
}

pub struct RemapOptions {
    pub profile: Profile,
    pub source_device_path: String,
}

pub struct RemapReport {
    pub virtual_nodes: Vec<String>,
}

pub struct RemapRuntime {
    profile: Profile,
    source: Device,
    virtual_pad: VirtualDevice,
    layers: Vec<RuntimeLayer>,
    held_layers: HashSet<usize>,
    toggled_layers: HashSet<usize>,
    pressed_activators: HashSet<(usize, EventCode)>,
    pressed_key_targets: HashMap<EventCode, EventCode>,
    pressed_macro_sources: HashSet<EventCode>,
    pressed_command_sources: HashSet<EventCode>,
    held_macro_releases: HashMap<EventCode, Vec<MacroEvent>>,
    macro_queue: VecDeque<ScheduledMacroEvent>,
    macro_output_keys: HashSet<EventCode>,
    macro_output_axes: HashMap<EventCode, i32>,
    turbo_states: HashMap<EventCode, TurboState>,
    pending_activators: HashMap<EventCode, PendingActivator>,
    analog_tuning: HashMap<EventCode, AnalogTuning>,
    active_analog_zones: HashSet<(EventCode, usize)>,
    stick_transform_sources: HashMap<EventCode, Vec<usize>>,
    stick_transform_states: Vec<StickTransformState>,
    digital_stick_sources: HashMap<EventCode, Vec<usize>>,
    digital_stick_states: Vec<DigitalStickState>,
    relative_axis_states: HashMap<EventCode, RelativeAxisState>,
    command_children: Vec<Child>,
    virtual_nodes: Vec<String>,
}

struct RuntimeLayer {
    activation: Option<LayerActivation>,
    mappings: HashMap<EventCode, Mapping>,
}

#[derive(Clone)]
struct ResolvedMapping {
    action: MappingAction,
    target: EventCode,
    mapped: bool,
    activator: ActivatorSettings,
    turbo_interval: Option<Duration>,
    macro_settings: Option<MacroSettings>,
    command: Option<CommandSettings>,
}

struct TurboState {
    target: EventCode,
    interval: Duration,
    next_toggle: Instant,
    output_down: bool,
}

struct ScheduledMacroEvent {
    due: Instant,
    event: EventCode,
    value: i32,
}

struct StickTransformState {
    mapping: StickTransformMapping,
    x_value: i32,
    y_value: i32,
    output_x_value: i32,
    output_y_value: i32,
}

struct DigitalStickState {
    mapping: DigitalStickMapping,
    x_value: i32,
    y_value: i32,
    output_x_value: i32,
    output_y_value: i32,
}

struct RelativeAxisState {
    target: EventCode,
    unit_value: f64,
    remainder: f64,
    next_emit: Instant,
}

struct PendingActivator {
    resolved: ResolvedMapping,
    count: u8,
    required_count: u8,
    due: Option<Instant>,
}

impl StickTransformState {
    fn new(mapping: StickTransformMapping) -> Self {
        Self {
            mapping,
            x_value: 0,
            y_value: 0,
            output_x_value: 0,
            output_y_value: 0,
        }
    }
}

impl DigitalStickState {
    fn new(mapping: DigitalStickMapping) -> Self {
        Self {
            mapping,
            x_value: 0,
            y_value: 0,
            output_x_value: 0,
            output_y_value: 0,
        }
    }
}

pub fn launch_with_remap(options: LaunchOptions) -> Result<i32> {
    if options.command.is_empty() {
        return Err(anyhow!("launch command is empty"));
    }

    let mut runtime = RemapRuntime::start(RemapOptions {
        profile: options.profile,
        source_device_path: options.source_device_path,
    })?;
    if !runtime.virtual_nodes().is_empty() {
        eprintln!(
            "PadProxy virtual pad: {}",
            runtime.virtual_nodes().join(", ")
        );
    }

    let mut child = Command::new(&options.command[0])
        .args(&options.command[1..])
        .spawn()
        .with_context(|| format!("failed to launch {}", options.command[0]))?;

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.code().unwrap_or(1));
        }

        runtime.pump_once()?;
    }
}

pub fn run_remap_until_stop(options: RemapOptions, stop: &AtomicBool) -> Result<RemapReport> {
    let mut runtime = RemapRuntime::start(options)?;
    let report = RemapReport {
        virtual_nodes: runtime.virtual_nodes().to_vec(),
    };

    while !stop.load(Ordering::Relaxed) {
        runtime.pump_once()?;
    }

    Ok(report)
}

impl RemapRuntime {
    pub fn start(options: RemapOptions) -> Result<Self> {
        if output_device(&options.profile.output_type)
            .map(|output| !output.supported)
            .unwrap_or(true)
        {
            return Err(anyhow!(
                "virtual output {} is not implemented; supported outputs: {}",
                options.profile.output_type,
                supported_output_ids().join(", ")
            ));
        }

        let mut source = Device::open(&options.source_device_path)
            .with_context(|| format!("failed to open {}", options.source_device_path))?;
        source
            .set_nonblocking(true)
            .context("failed to make source device nonblocking")?;

        let mut virtual_pad =
            create_virtual_xbox_pad(&options.profile).context("failed to create virtual pad")?;
        let virtual_nodes = virtual_pad
            .enumerate_dev_nodes_blocking()
            .ok()
            .map(|nodes| {
                nodes
                    .filter_map(Result::ok)
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        if options.profile.grab_source {
            source.grab().context("failed to grab source device")?;
        }

        let layers = options
            .profile
            .layers
            .iter()
            .map(|layer| RuntimeLayer {
                activation: layer.activation.clone(),
                mappings: layer.mapping_behavior_table(),
            })
            .collect();
        let analog_tuning = options.profile.analog.tuning_table();
        let stick_transform_sources = options.profile.analog.stick_transform_sources();
        let stick_transform_states = options
            .profile
            .analog
            .stick_transforms()
            .into_iter()
            .map(StickTransformState::new)
            .collect();
        let digital_stick_sources = options.profile.analog.digital_stick_sources();
        let digital_stick_states = options
            .profile
            .analog
            .digital_sticks
            .iter()
            .cloned()
            .map(DigitalStickState::new)
            .collect();

        Ok(Self {
            profile: options.profile,
            source,
            virtual_pad,
            layers,
            held_layers: HashSet::new(),
            toggled_layers: HashSet::new(),
            pressed_activators: HashSet::new(),
            pressed_key_targets: HashMap::new(),
            pressed_macro_sources: HashSet::new(),
            pressed_command_sources: HashSet::new(),
            held_macro_releases: HashMap::new(),
            macro_queue: VecDeque::new(),
            macro_output_keys: HashSet::new(),
            macro_output_axes: HashMap::new(),
            turbo_states: HashMap::new(),
            pending_activators: HashMap::new(),
            analog_tuning,
            active_analog_zones: HashSet::new(),
            stick_transform_sources,
            stick_transform_states,
            digital_stick_sources,
            digital_stick_states,
            relative_axis_states: HashMap::new(),
            command_children: Vec::new(),
            virtual_nodes,
        })
    }

    pub fn virtual_nodes(&self) -> &[String] {
        &self.virtual_nodes
    }

    pub fn pump_once(&mut self) -> Result<()> {
        self.reap_command_children();

        let had_events;
        let events = match self.source.fetch_events() {
            Ok(events) => {
                had_events = true;
                events.collect::<Vec<_>>()
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                had_events = false;
                Vec::new()
            }
            Err(error) => return Err(error).context("failed reading source events"),
        };

        let mut output = Vec::new();
        for event in events {
            if event.event_type() == EventType::SYNCHRONIZATION {
                continue;
            }

            let Some(source_event) = event_from_input(event) else {
                continue;
            };

            if self.update_layer_activation(source_event, event.value()) {
                continue;
            }

            self.push_mapped_event(source_event, event.value(), &mut output);
        }

        self.push_due_activator_events(&mut output);
        self.push_due_macro_events(&mut output);
        self.push_due_turbo_events(&mut output);
        self.push_due_relative_axis_events(&mut output);

        if !output.is_empty() {
            self.virtual_pad.emit(&output)?;
        } else if !had_events {
            thread::sleep(Duration::from_millis(5));
        }

        Ok(())
    }

    fn update_layer_activation(&mut self, source_event: EventCode, value: i32) -> bool {
        let mut consumed = false;
        for (index, layer) in self.layers.iter().enumerate().skip(1) {
            let Some(activation) = &layer.activation else {
                continue;
            };
            if activation.control != source_event {
                continue;
            }

            consumed |= activation.consume;
            match activation.mode {
                LayerActivationMode::Hold => {
                    if value == 0 {
                        self.held_layers.remove(&index);
                    } else {
                        self.held_layers.insert(index);
                    }
                }
                LayerActivationMode::Toggle => {
                    if value == 0 {
                        self.pressed_activators.remove(&(index, source_event));
                    } else if self.pressed_activators.insert((index, source_event)) {
                        if !self.toggled_layers.remove(&index) {
                            self.toggled_layers.insert(index);
                        }
                    }
                }
            }
        }
        consumed
    }

    fn push_mapped_event(
        &mut self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        match source_event.kind {
            EventKind::Key => self.push_mapped_key_event(source_event, value, output),
            EventKind::Absolute => self.push_mapped_absolute_event(source_event, value, output),
            EventKind::Relative => {}
        }
    }

    fn push_mapped_key_event(
        &mut self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        let resolved = self.resolved_mapping(source_event);
        if resolved.activator.kind != ActivatorKind::Press {
            self.push_activated_key_event(source_event, value, resolved, output);
            return;
        }

        if value == 0 {
            if self.pressed_command_sources.remove(&source_event) {
                return;
            }

            if self.pressed_macro_sources.remove(&source_event) {
                if let Some(release_events) = self.held_macro_releases.remove(&source_event) {
                    self.enqueue_macro_sequence(&release_events);
                }
                return;
            }

            let Some(target_event) = self.pressed_key_targets.remove(&source_event) else {
                if resolved.action == MappingAction::Map
                    && (self.profile.passthrough || resolved.mapped)
                    && virtual_output_supports(resolved.target)
                {
                    push_input_event(output, resolved.target, 0);
                }
                return;
            };

            if let Some(turbo) = self.turbo_states.remove(&source_event) {
                if turbo.output_down {
                    push_input_event(output, target_event, 0);
                }
            } else {
                push_input_event(output, target_event, 0);
            }
            return;
        }

        if self.pressed_key_targets.contains_key(&source_event)
            || self.pressed_macro_sources.contains(&source_event)
            || self.pressed_command_sources.contains(&source_event)
        {
            return;
        }

        self.push_press_mapping_action(source_event, resolved, output);
    }

    fn push_press_mapping_action(
        &mut self,
        source_event: EventCode,
        resolved: ResolvedMapping,
        output: &mut Vec<InputEvent>,
    ) {
        if resolved.action == MappingAction::Macro {
            if let Some(macro_settings) = resolved.macro_settings {
                self.pressed_macro_sources.insert(source_event);
                self.enqueue_macro_sequence(&macro_settings.events);
                if macro_settings.mode == MacroMode::Hold {
                    self.held_macro_releases
                        .insert(source_event, macro_settings.release_events);
                }
            }
            return;
        }

        if resolved.action == MappingAction::Command {
            if let Some(command) = resolved.command.as_ref() {
                self.pressed_command_sources.insert(source_event);
                self.execute_command(command, output);
            }
            return;
        }

        if resolved.action != MappingAction::Map
            || (!self.profile.passthrough && !resolved.mapped)
            || !virtual_output_supports(resolved.target)
        {
            return;
        }

        self.pressed_key_targets
            .insert(source_event, resolved.target);
        push_input_event(output, resolved.target, 1);

        if let Some(interval) = resolved.turbo_interval {
            self.turbo_states.insert(
                source_event,
                TurboState {
                    target: resolved.target,
                    interval,
                    next_toggle: Instant::now() + interval,
                    output_down: true,
                },
            );
        }
    }

    fn push_activated_key_event(
        &mut self,
        source_event: EventCode,
        value: i32,
        resolved: ResolvedMapping,
        output: &mut Vec<InputEvent>,
    ) {
        match resolved.activator.kind {
            ActivatorKind::Press => unreachable!("press activator uses the normal press path"),
            ActivatorKind::Release => {
                if value == 0 {
                    if let Some(pending) = self.pending_activators.remove(&source_event) {
                        self.trigger_activated_mapping(source_event, pending.resolved, output);
                    }
                } else {
                    self.pending_activators.insert(
                        source_event,
                        PendingActivator {
                            resolved,
                            count: 1,
                            required_count: 1,
                            due: None,
                        },
                    );
                }
            }
            ActivatorKind::LongPress => {
                if value == 0 {
                    self.pending_activators.remove(&source_event);
                    return;
                }
                self.pending_activators
                    .entry(source_event)
                    .or_insert_with(|| PendingActivator {
                        due: Some(
                            Instant::now() + Duration::from_millis(resolved.activator.delay_ms),
                        ),
                        resolved,
                        count: 1,
                        required_count: 1,
                    });
            }
            ActivatorKind::DoublePress | ActivatorKind::TriplePress => {
                if value == 0 {
                    return;
                }

                let required_count = match resolved.activator.kind {
                    ActivatorKind::DoublePress => 2,
                    ActivatorKind::TriplePress => 3,
                    _ => unreachable!("multi-press branch only handles multi-press activators"),
                };
                let due = Instant::now() + Duration::from_millis(resolved.activator.delay_ms);
                let mut should_trigger = false;
                if let Some(pending) = self.pending_activators.get_mut(&source_event) {
                    if pending.resolved.activator.kind == resolved.activator.kind {
                        pending.count = pending.count.saturating_add(1);
                    } else {
                        pending.count = 1;
                    }
                    pending.required_count = required_count;
                    pending.due = Some(due);
                    pending.resolved = resolved;
                    should_trigger = pending.count >= pending.required_count;
                } else {
                    self.pending_activators.insert(
                        source_event,
                        PendingActivator {
                            resolved,
                            count: 1,
                            required_count,
                            due: Some(due),
                        },
                    );
                }

                if should_trigger {
                    if let Some(pending) = self.pending_activators.remove(&source_event) {
                        self.trigger_activated_mapping(source_event, pending.resolved, output);
                    }
                }
            }
        }
    }

    fn trigger_activated_mapping(
        &mut self,
        source_event: EventCode,
        resolved: ResolvedMapping,
        output: &mut Vec<InputEvent>,
    ) {
        match resolved.action {
            MappingAction::Macro => {
                if let Some(macro_settings) = resolved.macro_settings {
                    self.enqueue_macro_sequence(&macro_settings.events);
                }
            }
            MappingAction::Command => {
                if let Some(command) = resolved.command.as_ref() {
                    self.execute_command(command, output);
                }
            }
            MappingAction::Disable => {}
            MappingAction::Map => {
                if (!self.profile.passthrough && !resolved.mapped)
                    || !virtual_output_supports(resolved.target)
                {
                    return;
                }
                push_input_event(output, resolved.target, 1);
                push_input_event(output, resolved.target, 0);
            }
        }

        self.pending_activators.remove(&source_event);
    }

    fn push_mapped_absolute_event(
        &mut self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        let resolved = self.resolved_mapping(source_event);
        self.push_digital_stick_events(source_event, value, output);
        self.push_analog_zone_events(source_event, resolved.target, value, output);
        let transformed = resolved.action == MappingAction::Map
            && resolved.target.kind == EventKind::Absolute
            && self.push_stick_transform_events(source_event, value, output);
        if transformed {
            self.relative_axis_states.remove(&source_event);
            return;
        }
        if resolved.action != MappingAction::Map
            || (!self.profile.passthrough && !resolved.mapped)
            || !virtual_output_supports(resolved.target)
        {
            self.relative_axis_states.remove(&source_event);
            return;
        }
        if resolved.target.kind == EventKind::Relative {
            self.update_relative_axis_state(source_event, resolved.target, value);
            return;
        }
        let value = self.apply_analog_tuning(source_event, resolved.target, value);
        push_input_event(output, resolved.target, value);
    }

    fn push_stick_transform_events(
        &mut self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) -> bool {
        let Some(indexes) = self.stick_transform_sources.get(&source_event).cloned() else {
            return false;
        };

        for index in indexes {
            let Some(state) = self.stick_transform_states.get_mut(index) else {
                continue;
            };
            if source_event == state.mapping.source_x {
                state.x_value = value;
            }
            if source_event == state.mapping.source_y {
                state.y_value = value;
            }

            let x_unit = stick_axis_unit_value(
                state.mapping.source_x,
                state.x_value,
                self.analog_tuning.get(&state.mapping.source_x),
            );
            let y_unit = stick_axis_unit_value(
                state.mapping.source_y,
                state.y_value,
                self.analog_tuning.get(&state.mapping.source_y),
            );
            let (rotated_x, rotated_y) =
                rotate_stick_units(x_unit, y_unit, state.mapping.rotation_degrees);
            let next_x = unit_to_absolute_axis_value(state.mapping.output_x, rotated_x);
            let next_y = unit_to_absolute_axis_value(state.mapping.output_y, rotated_y);

            if next_x != state.output_x_value {
                state.output_x_value = next_x;
                push_input_event(output, state.mapping.output_x, next_x);
            }
            if next_y != state.output_y_value {
                state.output_y_value = next_y;
                push_input_event(output, state.mapping.output_y, next_y);
            }
        }

        true
    }

    fn push_digital_stick_events(
        &mut self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        let Some(indexes) = self.digital_stick_sources.get(&source_event).cloned() else {
            return;
        };

        for index in indexes {
            let Some(state) = self.digital_stick_states.get_mut(index) else {
                continue;
            };
            if source_event == state.mapping.x_axis {
                state.x_value = value;
            }
            if source_event == state.mapping.y_axis {
                state.y_value = value;
            }

            let x_unit = digital_axis_unit_value(
                state.mapping.x_axis,
                state.x_value,
                self.analog_tuning.get(&state.mapping.x_axis),
            );
            let y_unit = digital_axis_unit_value(
                state.mapping.y_axis,
                state.y_value,
                self.analog_tuning.get(&state.mapping.y_axis),
            );
            let next_x = digital_axis_direction(x_unit, state.mapping.threshold);
            let next_y = digital_axis_direction(y_unit, state.mapping.threshold);

            if next_x != state.output_x_value {
                state.output_x_value = next_x;
                push_input_event(output, state.mapping.output_x, next_x);
            }
            if next_y != state.output_y_value {
                state.output_y_value = next_y;
                push_input_event(output, state.mapping.output_y, next_y);
            }
        }
    }

    fn update_relative_axis_state(
        &mut self,
        source_event: EventCode,
        target_event: EventCode,
        value: i32,
    ) {
        let unit_value =
            relative_axis_unit_value(source_event, value, self.analog_tuning.get(&source_event));
        if unit_value.abs() <= f64::EPSILON {
            self.relative_axis_states.remove(&source_event);
            return;
        }

        let now = Instant::now();
        self.relative_axis_states
            .entry(source_event)
            .and_modify(|state| {
                if state.target != target_event {
                    state.remainder = 0.0;
                    state.next_emit = now;
                }
                state.target = target_event;
                state.unit_value = unit_value;
            })
            .or_insert(RelativeAxisState {
                target: target_event,
                unit_value,
                remainder: 0.0,
                next_emit: now,
            });
    }

    fn push_analog_zone_events(
        &mut self,
        source_event: EventCode,
        target_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        let Some(tuning) = self
            .analog_tuning
            .get(&target_event)
            .or_else(|| self.analog_tuning.get(&source_event))
        else {
            return;
        };
        if tuning.zones.is_empty() {
            return;
        }

        let axis = tuning.code;
        let active_indexes = tuning
            .active_zone_indexes(value)
            .into_iter()
            .collect::<HashSet<_>>();
        let zones = tuning.zones.clone();
        for (index, zone) in zones.iter().enumerate() {
            let key = (axis, index);
            let active = active_indexes.contains(&index);
            let was_active = self.active_analog_zones.contains(&key);
            if active && !was_active {
                self.active_analog_zones.insert(key);
                push_input_event(output, zone.target, 1);
            } else if !active && was_active {
                self.active_analog_zones.remove(&key);
                push_input_event(output, zone.target, 0);
            }
        }
    }

    fn enqueue_macro_sequence(&mut self, events: &[MacroEvent]) {
        const TAP_RELEASE_MS: u64 = 20;

        let now = Instant::now();
        let mut offset = Duration::ZERO;

        for event in events {
            match event.kind {
                MacroEventKind::Pause => {
                    offset += Duration::from_millis(event.pause_ms);
                }
                MacroEventKind::Tap => {
                    if let Some(code) = event.code {
                        self.queue_macro_event(ScheduledMacroEvent {
                            due: now + offset,
                            event: code,
                            value: 1,
                        });
                        offset += Duration::from_millis(TAP_RELEASE_MS);
                        self.queue_macro_event(ScheduledMacroEvent {
                            due: now + offset,
                            event: code,
                            value: 0,
                        });
                    }
                }
                MacroEventKind::Down
                | MacroEventKind::Up
                | MacroEventKind::Axis
                | MacroEventKind::Relative => {
                    if let Some(code) = event.code {
                        self.queue_macro_event(ScheduledMacroEvent {
                            due: now + offset,
                            event: code,
                            value: event.value,
                        });
                    }
                }
            }
        }
    }

    fn execute_command(&mut self, settings: &CommandSettings, output: &mut Vec<InputEvent>) {
        match settings.action {
            CommandAction::StopMacros => self.stop_all_macros(output),
            CommandAction::RunCommand => self.spawn_command(&settings.command_line),
        }
    }

    fn spawn_command(&mut self, command_line: &[String]) {
        if command_line.is_empty() {
            eprintln!("PadProxy run command mapping has an empty command_line");
            return;
        }

        match Command::new(&command_line[0])
            .args(&command_line[1..])
            .spawn()
        {
            Ok(child) => self.command_children.push(child),
            Err(error) => eprintln!(
                "PadProxy failed to run command mapping {}: {error}",
                command_line.join(" ")
            ),
        }
    }

    fn reap_command_children(&mut self) {
        let mut index = 0;
        while index < self.command_children.len() {
            match self.command_children[index].try_wait() {
                Ok(Some(_)) => {
                    let _ = self.command_children.remove(index);
                }
                Ok(None) => index += 1,
                Err(error) => {
                    eprintln!("PadProxy failed to reap command mapping process: {error}");
                    let _ = self.command_children.remove(index);
                }
            }
        }
    }

    fn stop_all_macros(&mut self, output: &mut Vec<InputEvent>) {
        self.macro_queue.clear();
        self.pressed_macro_sources.clear();
        self.held_macro_releases.clear();

        let mapped_key_targets = self
            .pressed_key_targets
            .values()
            .copied()
            .collect::<HashSet<_>>();
        for key in self.macro_output_keys.drain() {
            if !mapped_key_targets.contains(&key) {
                push_input_event(output, key, 0);
            }
        }
        for (axis, value) in self.macro_output_axes.drain() {
            if value != 0 {
                push_input_event(output, axis, 0);
            }
        }
    }

    fn queue_macro_event(&mut self, scheduled: ScheduledMacroEvent) {
        let index = self
            .macro_queue
            .iter()
            .position(|existing| scheduled.due < existing.due)
            .unwrap_or(self.macro_queue.len());
        self.macro_queue.insert(index, scheduled);
    }

    fn push_due_activator_events(&mut self, output: &mut Vec<InputEvent>) {
        let now = Instant::now();
        let due_sources = self
            .pending_activators
            .iter()
            .filter_map(|(source, pending)| {
                if matches!(pending.due, Some(due) if now >= due) {
                    Some(*source)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for source_event in due_sources {
            let Some(pending) = self.pending_activators.remove(&source_event) else {
                continue;
            };
            if pending.resolved.activator.kind == ActivatorKind::LongPress {
                self.trigger_activated_mapping(source_event, pending.resolved, output);
            }
        }
    }

    fn push_due_macro_events(&mut self, output: &mut Vec<InputEvent>) {
        let now = Instant::now();
        loop {
            let Some(front) = self.macro_queue.front() else {
                break;
            };
            if now < front.due {
                break;
            }

            let scheduled = self
                .macro_queue
                .pop_front()
                .expect("front existed before pop");
            let value = if scheduled.event.kind == EventKind::Absolute {
                self.apply_analog_tuning(scheduled.event, scheduled.event, scheduled.value)
            } else {
                scheduled.value
            };
            self.record_macro_output(scheduled.event, value);
            push_input_event(output, scheduled.event, value);
        }
    }

    fn record_macro_output(&mut self, event: EventCode, value: i32) {
        match event.kind {
            EventKind::Key => {
                if value == 0 {
                    self.macro_output_keys.remove(&event);
                } else {
                    self.macro_output_keys.insert(event);
                }
            }
            EventKind::Absolute => {
                if value == 0 {
                    self.macro_output_axes.remove(&event);
                } else {
                    self.macro_output_axes.insert(event, value);
                }
            }
            EventKind::Relative => {}
        }
    }

    fn push_due_turbo_events(&mut self, output: &mut Vec<InputEvent>) {
        let now = Instant::now();
        for state in self.turbo_states.values_mut() {
            if now < state.next_toggle {
                continue;
            }

            state.output_down = !state.output_down;
            state.next_toggle = now + state.interval;
            push_input_event(output, state.target, if state.output_down { 1 } else { 0 });
        }
    }

    fn push_due_relative_axis_events(&mut self, output: &mut Vec<InputEvent>) {
        let now = Instant::now();
        for state in self.relative_axis_states.values_mut() {
            if now < state.next_emit {
                continue;
            }

            let value = relative_axis_step(state.unit_value, &mut state.remainder);
            state.next_emit = now + RELATIVE_AXIS_INTERVAL;
            if value != 0 {
                push_input_event(output, state.target, value);
            }
        }
    }

    fn resolved_mapping(&self, source_event: EventCode) -> ResolvedMapping {
        let layer = &self.layers[self.active_layer_index()];
        layer
            .mappings
            .get(&source_event)
            .map(|mapping| ResolvedMapping {
                action: mapping.action,
                target: mapping.to,
                mapped: true,
                activator: mapping.activator.clone(),
                turbo_interval: mapping
                    .turbo
                    .as_ref()
                    .map(|turbo| Duration::from_millis(turbo.interval_ms)),
                macro_settings: mapping.macro_settings.clone(),
                command: mapping.command.clone(),
            })
            .unwrap_or(ResolvedMapping {
                action: MappingAction::Map,
                target: source_event,
                mapped: false,
                activator: ActivatorSettings::default(),
                turbo_interval: None,
                macro_settings: None,
                command: None,
            })
    }

    fn active_layer_index(&self) -> usize {
        for index in (1..self.layers.len()).rev() {
            if self.held_layers.contains(&index) || self.toggled_layers.contains(&index) {
                return index;
            }
        }
        0
    }

    fn apply_analog_tuning(
        &self,
        source_event: EventCode,
        target_event: EventCode,
        value: i32,
    ) -> i32 {
        self.analog_tuning
            .get(&target_event)
            .or_else(|| self.analog_tuning.get(&source_event))
            .map(|tuning| tuning.apply(value))
            .unwrap_or(value)
    }
}

fn push_input_event(output: &mut Vec<InputEvent>, event: EventCode, value: i32) {
    output.push(InputEvent::new(event.event_type().0, event.code, value));
}

fn stick_axis_unit_value(
    source_event: EventCode,
    value: i32,
    tuning: Option<&AnalogTuning>,
) -> f64 {
    let value = tuning.map(|tuning| tuning.apply(value)).unwrap_or(value);
    normalized_absolute_axis_value(source_event, value).unwrap_or(0.0)
}

fn rotate_stick_units(x: f64, y: f64, rotation_degrees: f64) -> (f64, f64) {
    let radians = rotation_degrees.to_radians();
    let sin = radians.sin();
    let cos = radians.cos();
    (
        (x * cos - y * sin).clamp(-1.0, 1.0),
        (x * sin + y * cos).clamp(-1.0, 1.0),
    )
}

fn unit_to_absolute_axis_value(event: EventCode, unit_value: f64) -> i32 {
    let unit = unit_value.clamp(-1.0, 1.0);
    match event.name().as_str() {
        "abs:hat0x" | "abs:hat0y" => unit.round() as i32,
        "abs:z" | "abs:rz" => (unit.clamp(0.0, 1.0) * 255.0).round() as i32,
        _ => (unit * 32768.0).round().clamp(-32768.0, 32767.0) as i32,
    }
}

fn relative_axis_unit_value(
    source_event: EventCode,
    value: i32,
    tuning: Option<&AnalogTuning>,
) -> f64 {
    let value = tuning.map(|tuning| tuning.apply(value)).unwrap_or(value);
    let Some(unit) = normalized_absolute_axis_value(source_event, value) else {
        return 0.0;
    };
    let deadzone = if tuning.is_some() {
        0.0
    } else {
        DEFAULT_RELATIVE_AXIS_DEADZONE
    };
    let magnitude = unit.abs();
    if magnitude <= deadzone {
        0.0
    } else {
        unit.signum() * ((magnitude - deadzone) / (1.0 - deadzone)).clamp(0.0, 1.0)
    }
}

fn normalized_absolute_axis_value(event: EventCode, value: i32) -> Option<f64> {
    if event.kind != EventKind::Absolute {
        return None;
    }

    match event.name().as_str() {
        "abs:z" | "abs:rz" => Some((value as f64 / 255.0).clamp(0.0, 1.0)),
        "abs:hat0x" | "abs:hat0y" => Some((value as f64).clamp(-1.0, 1.0)),
        _ => {
            let max_abs = 32768.0;
            Some((value as f64 / max_abs).clamp(-1.0, 1.0))
        }
    }
}

fn relative_axis_step(unit_value: f64, remainder: &mut f64) -> i32 {
    let raw = unit_value.clamp(-1.0, 1.0) * RELATIVE_AXIS_MAX_STEP + *remainder;
    let step = if raw.is_sign_negative() {
        raw.ceil()
    } else {
        raw.floor()
    };
    *remainder = raw - step;
    step as i32
}

fn digital_axis_unit_value(
    source_event: EventCode,
    value: i32,
    tuning: Option<&AnalogTuning>,
) -> f64 {
    let value = tuning.map(|tuning| tuning.apply(value)).unwrap_or(value);
    normalized_absolute_axis_value(source_event, value).unwrap_or(0.0)
}

fn digital_axis_direction(unit_value: f64, threshold: f64) -> i32 {
    let threshold = threshold.clamp(0.01, 1.0);
    if unit_value >= threshold {
        1
    } else if unit_value <= -threshold {
        -1
    } else {
        0
    }
}

fn create_virtual_xbox_pad(profile: &Profile) -> Result<VirtualDevice> {
    let mut keys = AttributeSet::<KeyCode>::new();
    for key in [
        KeyCode::BTN_SOUTH,
        KeyCode::BTN_EAST,
        KeyCode::BTN_NORTH,
        KeyCode::BTN_WEST,
        KeyCode::BTN_TL,
        KeyCode::BTN_TR,
        KeyCode::BTN_TL2,
        KeyCode::BTN_TR2,
        KeyCode::BTN_SELECT,
        KeyCode::BTN_START,
        KeyCode::BTN_MODE,
        KeyCode::BTN_THUMBL,
        KeyCode::BTN_THUMBR,
    ] {
        keys.insert(key);
    }
    let mut relative_axes = AttributeSet::<RelativeAxisCode>::new();
    for event in profile_output_events(profile) {
        match event.kind {
            EventKind::Key => {
                keys.insert(KeyCode(event.code));
            }
            EventKind::Relative => {
                relative_axes.insert(RelativeAxisCode(event.code));
            }
            EventKind::Absolute => {}
        }
    }

    let mut builder = VirtualDevice::builder()?
        .name("PadProxy Virtual Xbox 360 Controller")
        .input_id(InputId::new(BusType::BUS_USB, 0x045e, 0x028e, 0x0114))
        .with_keys(&keys)?;

    for setup in [
        axis(AbsoluteAxisCode::ABS_X, -32768, 32767, 4096),
        axis(AbsoluteAxisCode::ABS_Y, -32768, 32767, 4096),
        axis(AbsoluteAxisCode::ABS_RX, -32768, 32767, 4096),
        axis(AbsoluteAxisCode::ABS_RY, -32768, 32767, 4096),
        axis(AbsoluteAxisCode::ABS_Z, 0, 255, 0),
        axis(AbsoluteAxisCode::ABS_RZ, 0, 255, 0),
        axis(AbsoluteAxisCode::ABS_HAT0X, -1, 1, 0),
        axis(AbsoluteAxisCode::ABS_HAT0Y, -1, 1, 0),
    ] {
        builder = builder.with_absolute_axis(&setup)?;
    }

    if relative_axes.iter().next().is_some() {
        builder = builder.with_relative_axes(&relative_axes)?;
    }

    builder.build().map_err(Into::into)
}

fn axis(code: AbsoluteAxisCode, min: i32, max: i32, flat: i32) -> UinputAbsSetup {
    UinputAbsSetup::new(code, AbsInfo::new(0, min, max, 0, flat, 0))
}

fn profile_output_events(profile: &Profile) -> HashSet<EventCode> {
    let mut events = HashSet::new();
    for layer in &profile.layers {
        for mapping in &layer.mappings {
            match mapping.action {
                MappingAction::Map => {
                    events.insert(mapping.to);
                }
                MappingAction::Macro => {
                    if let Some(macro_settings) = &mapping.macro_settings {
                        events.extend(macro_settings.events.iter().filter_map(|event| event.code));
                        events.extend(
                            macro_settings
                                .release_events
                                .iter()
                                .filter_map(|event| event.code),
                        );
                    }
                }
                MappingAction::Disable | MappingAction::Command => {}
            }
        }
    }
    for axis in &profile.analog.axes {
        events.extend(axis.zones.iter().map(|zone| zone.target));
    }
    for transform in profile.analog.stick_transforms() {
        events.insert(transform.output_x);
        events.insert(transform.output_y);
    }
    for stick in &profile.analog.digital_sticks {
        events.insert(stick.output_x);
        events.insert(stick.output_y);
    }
    events
}

#[cfg(test)]
mod tests {
    use super::{
        digital_axis_direction, digital_axis_unit_value, profile_output_events, relative_axis_step,
        relative_axis_unit_value, rotate_stick_units, unit_to_absolute_axis_value,
    };
    use crate::event_code::parse_event_code;
    use crate::profiles::parse_profile_bytes;
    use std::path::Path;

    #[test]
    fn analog_zone_targets_are_virtual_output_capabilities() {
        let profile = parse_profile_bytes(
            br#"
id: zone-output
analog:
  axes:
    - code: abs:z
      zones:
        - name: high
          to: key:space
mappings: []
"#,
            Path::new("zone-output.yaml"),
        )
        .unwrap();

        let events = profile_output_events(&profile);
        assert!(events.contains(&parse_event_code("key:space").unwrap()));
    }

    #[test]
    fn direct_relative_axis_targets_are_virtual_output_capabilities() {
        let profile = parse_profile_bytes(
            br#"
id: mouse-output
mappings:
  - from: abs:x
    to: rel:x
  - from: abs:y
    to: rel:y
"#,
            Path::new("mouse-output.yaml"),
        )
        .unwrap();

        let events = profile_output_events(&profile);
        assert!(events.contains(&parse_event_code("rel:x").unwrap()));
        assert!(events.contains(&parse_event_code("rel:y").unwrap()));
    }

    #[test]
    fn digital_stick_outputs_are_virtual_output_capabilities() {
        let profile = parse_profile_bytes(
            br#"
id: digital-stick-output
analog:
  digital_sticks:
    - x_axis: abs:x
      y_axis: abs:y
mappings: []
"#,
            Path::new("digital-stick-output.yaml"),
        )
        .unwrap();

        let events = profile_output_events(&profile);
        assert!(events.contains(&parse_event_code("abs:hat0x").unwrap()));
        assert!(events.contains(&parse_event_code("abs:hat0y").unwrap()));
    }

    #[test]
    fn stick_transform_outputs_are_virtual_output_capabilities() {
        let profile = parse_profile_bytes(
            br#"
id: stick-transform-output
analog:
  swap_sticks: true
  sticks:
    - x_axis: abs:x
      y_axis: abs:y
      rotation_degrees: 30
mappings: []
"#,
            Path::new("stick-transform-output.yaml"),
        )
        .unwrap();

        let events = profile_output_events(&profile);
        assert!(events.contains(&parse_event_code("abs:x").unwrap()));
        assert!(events.contains(&parse_event_code("abs:y").unwrap()));
        assert!(events.contains(&parse_event_code("abs:rx").unwrap()));
        assert!(events.contains(&parse_event_code("abs:ry").unwrap()));
    }

    #[test]
    fn analog_values_become_relative_mouse_steps() {
        let source = parse_event_code("abs:x").unwrap();
        assert_eq!(relative_axis_unit_value(source, 0, None), 0.0);
        assert_eq!(relative_axis_unit_value(source, 3000, None), 0.0);

        let right = relative_axis_unit_value(source, 32767, None);
        assert!(right > 0.99, "{right}");
        let left = relative_axis_unit_value(source, -32768, None);
        assert!(left < -0.99, "{left}");

        let mut remainder = 0.0;
        assert_eq!(relative_axis_step(1.0, &mut remainder), 20);
        assert_eq!(relative_axis_step(-1.0, &mut remainder), -20);

        let mut fractional_remainder = 0.0;
        assert_eq!(relative_axis_step(0.03, &mut fractional_remainder), 0);
        assert_eq!(relative_axis_step(0.03, &mut fractional_remainder), 1);
    }

    #[test]
    fn analog_values_become_digital_directions() {
        let source = parse_event_code("abs:x").unwrap();
        assert_eq!(digital_axis_unit_value(source, 0, None), 0.0);
        assert_eq!(digital_axis_direction(0.49, 0.5), 0);
        assert_eq!(digital_axis_direction(0.50, 0.5), 1);
        assert_eq!(digital_axis_direction(-0.50, 0.5), -1);
    }

    #[test]
    fn stick_units_rotate_and_scale_to_axes() {
        let (x, y) = rotate_stick_units(1.0, 0.0, 90.0);
        assert!(x.abs() < 0.000_001, "{x}");
        assert!(y > 0.999, "{y}");

        let (x, y) = rotate_stick_units(0.0, 1.0, -90.0);
        assert!(x > 0.999, "{x}");
        assert!(y.abs() < 0.000_001, "{y}");

        assert_eq!(
            unit_to_absolute_axis_value(parse_event_code("abs:x").unwrap(), 1.0),
            32767
        );
        assert_eq!(
            unit_to_absolute_axis_value(parse_event_code("abs:y").unwrap(), -1.0),
            -32768
        );
    }
}
