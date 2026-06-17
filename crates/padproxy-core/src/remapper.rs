use crate::event_code::{event_from_input, virtual_xbox_supports, EventCode};
use crate::outputs::{output_device, supported_output_ids};
use crate::profiles::Profile;
use anyhow::{anyhow, Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, Device, EventType, InputEvent, InputId,
    KeyCode, UinputAbsSetup,
};
use std::collections::HashMap;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::{thread, time::Duration};

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
    mappings: HashMap<EventCode, EventCode>,
    virtual_nodes: Vec<String>,
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

        let mappings = options.profile.mapping_table();

        Ok(Self {
            profile: options.profile,
            source,
            virtual_pad,
            mappings,
            virtual_nodes,
        })
    }

    pub fn virtual_nodes(&self) -> &[String] {
        &self.virtual_nodes
    }

    pub fn pump_once(&mut self) -> Result<()> {
        match self.source.fetch_events() {
            Ok(events) => {
                let mut output = Vec::new();
                for event in events {
                    if event.event_type() == EventType::SYNCHRONIZATION {
                        continue;
                    }

                    let Some(source_event) = event_from_input(event) else {
                        continue;
                    };

                    let target_event = self
                        .mappings
                        .get(&source_event)
                        .copied()
                        .unwrap_or(source_event);
                    if (!self.profile.passthrough && !self.mappings.contains_key(&source_event))
                        || !virtual_xbox_supports(target_event)
                    {
                        continue;
                    }

                    output.push(InputEvent::new(
                        target_event.event_type().0,
                        target_event.code,
                        event.value(),
                    ));
                }

                if !output.is_empty() {
                    self.virtual_pad.emit(&output)?;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error).context("failed reading source events"),
        }
        Ok(())
    }
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
