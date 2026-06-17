use crate::event_code::{event_from_input, virtual_xbox_supports, EventCode, EventKind};
use crate::outputs::{output_device, supported_output_ids};
use crate::profiles::{
    AnalogTuning, LayerActivation, LayerActivationMode, MacroEventKind, MacroSettings, Mapping,
    MappingAction, Profile,
};
use anyhow::{anyhow, Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, Device, EventType, InputEvent, InputId,
    KeyCode, UinputAbsSetup,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

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
    macro_queue: VecDeque<ScheduledMacroEvent>,
    turbo_states: HashMap<EventCode, TurboState>,
    analog_tuning: HashMap<EventCode, AnalogTuning>,
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
    turbo_interval: Option<Duration>,
    macro_settings: Option<MacroSettings>,
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

        let mut virtual_pad = create_virtual_xbox_pad().context("failed to create virtual pad")?;
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
            macro_queue: VecDeque::new(),
            turbo_states: HashMap::new(),
            analog_tuning,
            virtual_nodes,
        })
    }

    pub fn virtual_nodes(&self) -> &[String] {
        &self.virtual_nodes
    }

    pub fn pump_once(&mut self) -> Result<()> {
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

        self.push_due_macro_events(&mut output);
        self.push_due_turbo_events(&mut output);

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
        if source_event.kind == EventKind::Key {
            self.push_mapped_key_event(source_event, value, output);
        } else {
            self.push_mapped_absolute_event(source_event, value, output);
        }
    }

    fn push_mapped_key_event(
        &mut self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        if value == 0 {
            if self.pressed_macro_sources.remove(&source_event) {
                return;
            }

            let Some(target_event) = self.pressed_key_targets.remove(&source_event) else {
                let resolved = self.resolved_mapping(source_event);
                if resolved.action == MappingAction::Map
                    && (self.profile.passthrough || resolved.mapped)
                    && virtual_xbox_supports(resolved.target)
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
        {
            return;
        }

        let resolved = self.resolved_mapping(source_event);
        if resolved.action == MappingAction::Macro {
            if let Some(macro_settings) = resolved.macro_settings.as_ref() {
                self.pressed_macro_sources.insert(source_event);
                self.enqueue_macro_events(macro_settings);
            }
            return;
        }

        if resolved.action != MappingAction::Map
            || (!self.profile.passthrough && !resolved.mapped)
            || !virtual_xbox_supports(resolved.target)
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

    fn push_mapped_absolute_event(
        &self,
        source_event: EventCode,
        value: i32,
        output: &mut Vec<InputEvent>,
    ) {
        let resolved = self.resolved_mapping(source_event);
        if resolved.action != MappingAction::Map
            || (!self.profile.passthrough && !resolved.mapped)
            || !virtual_xbox_supports(resolved.target)
        {
            return;
        }
        let value = self.apply_analog_tuning(source_event, resolved.target, value);
        push_input_event(output, resolved.target, value);
    }

    fn enqueue_macro_events(&mut self, macro_settings: &MacroSettings) {
        const TAP_RELEASE_MS: u64 = 20;

        let now = Instant::now();
        let mut offset = Duration::ZERO;

        for event in &macro_settings.events {
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
                MacroEventKind::Down | MacroEventKind::Up | MacroEventKind::Axis => {
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

    fn queue_macro_event(&mut self, scheduled: ScheduledMacroEvent) {
        let index = self
            .macro_queue
            .iter()
            .position(|existing| scheduled.due < existing.due)
            .unwrap_or(self.macro_queue.len());
        self.macro_queue.insert(index, scheduled);
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
            push_input_event(output, scheduled.event, value);
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

    fn resolved_mapping(&self, source_event: EventCode) -> ResolvedMapping {
        let layer = &self.layers[self.active_layer_index()];
        layer
            .mappings
            .get(&source_event)
            .map(|mapping| ResolvedMapping {
                action: mapping.action,
                target: mapping.to,
                mapped: true,
                turbo_interval: mapping
                    .turbo
                    .as_ref()
                    .map(|turbo| Duration::from_millis(turbo.interval_ms)),
                macro_settings: mapping.macro_settings.clone(),
            })
            .unwrap_or(ResolvedMapping {
                action: MappingAction::Map,
                target: source_event,
                mapped: false,
                turbo_interval: None,
                macro_settings: None,
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

fn create_virtual_xbox_pad() -> Result<VirtualDevice> {
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

    builder.build().map_err(Into::into)
}

fn axis(code: AbsoluteAxisCode, min: i32, max: i32, flat: i32) -> UinputAbsSetup {
    UinputAbsSetup::new(code, AbsInfo::new(0, min, max, 0, flat, 0))
}
