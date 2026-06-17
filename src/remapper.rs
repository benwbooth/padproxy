use crate::event_code::{virtual_xbox_supports, EventCode, EventKind};
use crate::profiles::Profile;
use anyhow::{anyhow, Context, Result};
use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, Device, EventType, InputEvent, InputId,
    KeyCode, UinputAbsSetup,
};
use std::process::Command;
use std::thread;
use std::time::Duration;

pub struct LaunchOptions {
    pub profile: Profile,
    pub source_device_path: String,
    pub command: Vec<String>,
}

pub fn launch_with_remap(options: LaunchOptions) -> Result<i32> {
    if options.command.is_empty() {
        return Err(anyhow!("launch command is empty"));
    }
    if options.profile.output_type != "xbox360" {
        return Err(anyhow!(
            "only xbox360 virtual output is currently implemented"
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
        });

    if options.profile.grab_source {
        source.grab().context("failed to grab source device")?;
    }

    if let Some(nodes) = virtual_nodes.filter(|nodes| !nodes.is_empty()) {
        eprintln!("PadProxy virtual pad: {}", nodes.join(", "));
    }

    let mappings = options.profile.mapping_table();
    let mut child = Command::new(&options.command[0])
        .args(&options.command[1..])
        .spawn()
        .with_context(|| format!("failed to launch {}", options.command[0]))?;

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.code().unwrap_or(1));
        }

        match source.fetch_events() {
            Ok(events) => {
                let mut output = Vec::new();
                for event in events {
                    if event.event_type() == EventType::SYNCHRONIZATION {
                        continue;
                    }

                    let Some(source_event) = event_from_input(event) else {
                        continue;
                    };

                    let target_event = mappings.get(&source_event).copied().unwrap_or(source_event);
                    if (!options.profile.passthrough && !mappings.contains_key(&source_event))
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
                    virtual_pad.emit(&output)?;
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error).context("failed reading source events"),
        }
    }
}

fn event_from_input(event: InputEvent) -> Option<EventCode> {
    if event.event_type() == EventType::KEY {
        Some(EventCode {
            kind: EventKind::Key,
            code: event.code(),
        })
    } else if event.event_type() == EventType::ABSOLUTE {
        Some(EventCode {
            kind: EventKind::Absolute,
            code: event.code(),
        })
    } else {
        None
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
