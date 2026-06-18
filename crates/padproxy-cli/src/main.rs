use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use padproxy_core::devices::DeviceInfo;
use padproxy_core::linux::{list_devices, resolve_device};
use padproxy_core::outputs::output_devices;
use padproxy_core::profiles::{default_profile_dirs, load_profiles, Profile};
use padproxy_core::remapper::{launch_with_remap, LaunchOptions, RemapOptions, RemapRuntime};
use padproxy_core::slots::{
    load_slot_store, save_slot_store, validate_slot, SlotStore, SLOT_COUNT,
};
use std::path::Path;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    ListDevices,
    ListOutputs,
    ListProfiles,
    ListSlots {
        #[arg(long)]
        controller: Option<String>,
    },
    AssignSlot {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        slot: u8,
        #[arg(long)]
        profile: String,
    },
    SelectSlot {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        slot: u8,
    },
    ClearSlot {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        slot: u8,
    },
    ApplySlot {
        #[arg(long)]
        controller: String,
        #[arg(long)]
        slot: Option<u8>,
    },
    Remap {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        controller: String,
    },
    Apply {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        controller: String,
    },
    Launch {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        controller: String,
        #[arg(last = true, required = true)]
        command: Vec<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::ListDevices => {
            for device in list_devices()? {
                println!(
                    "{}\t{}\t{}\t{}\tid={}",
                    device.path,
                    device.device_kind,
                    device.hardware_id(),
                    device.name,
                    device.id
                );
            }
            Ok(())
        }
        Command::ListOutputs => {
            for output in output_devices() {
                let status = if output.supported {
                    "supported"
                } else {
                    "planned"
                };
                println!(
                    "{}\t{}\t{}\t{}",
                    output.id, status, output.label, output.note
                );
            }
            Ok(())
        }
        Command::ListProfiles => {
            for profile in load_profiles(&default_profile_dirs())? {
                println!(
                    "{}\t{}\t{}",
                    profile.id,
                    profile.name,
                    profile.source_path.display()
                );
            }
            Ok(())
        }
        Command::ListSlots { controller } => list_slots(controller.as_deref()),
        Command::AssignSlot {
            controller,
            slot,
            profile,
        } => assign_slot(&controller, slot, &profile),
        Command::SelectSlot { controller, slot } => select_slot(&controller, slot),
        Command::ClearSlot { controller, slot } => clear_slot(&controller, slot),
        Command::ApplySlot { controller, slot } => apply_slot(&controller, slot),
        Command::Launch {
            profile,
            controller,
            command,
        } => {
            let profile = find_profile(&profile)?;
            let source_device_path = resolve_device_path(&controller)?;
            let code = launch_with_remap(LaunchOptions {
                profile,
                source_device_path,
                command,
            })?;
            std::process::exit(code);
        }
        Command::Remap {
            profile,
            controller,
        }
        | Command::Apply {
            profile,
            controller,
        } => run_foreground_remap(&profile, &controller),
    }
}

fn find_profile(selector: &str) -> Result<Profile> {
    load_profiles(&default_profile_dirs())?
        .into_iter()
        .find(|profile| profile.id == selector || profile.name == selector)
        .ok_or_else(|| anyhow!("no profile matched {selector}"))
}

fn run_foreground_remap(profile: &str, controller: &str) -> Result<()> {
    let profile = find_profile(profile)?;
    let source_device_path = resolve_device_path(controller)?;
    run_foreground_profile(profile, source_device_path)
}

fn run_foreground_profile(profile: Profile, source_device_path: String) -> Result<()> {
    let mut runtime = RemapRuntime::start(RemapOptions {
        profile,
        source_device_path,
    })?;
    if !runtime.virtual_nodes().is_empty() {
        eprintln!(
            "PadProxy virtual pad: {}",
            runtime.virtual_nodes().join(", ")
        );
    }
    eprintln!("PadProxy remap is running. Press Ctrl-C to stop.");

    loop {
        runtime.pump_once()?;
    }
}

fn list_slots(controller: Option<&str>) -> Result<()> {
    let store = load_slot_store()?;
    if let Some(controller) = controller {
        let device = select_device(controller)?;
        print_device_slots(&store, &device.id, Some(&device.name));
        return Ok(());
    }

    for device_id in store.devices.keys() {
        print_device_slots(&store, device_id, None);
    }
    Ok(())
}

fn assign_slot(controller: &str, slot: u8, profile: &str) -> Result<()> {
    validate_slot(slot)?;
    let device = select_device(controller)?;
    let profile = find_profile(profile)?;
    let mut store = load_slot_store()?;
    store.assign_profile(&device.id, slot, &profile.id)?;
    save_slot_store(&store)?;
    eprintln!(
        "Assigned slot {slot} on {} to profile {}",
        device_label(&device),
        profile.id
    );
    Ok(())
}

fn select_slot(controller: &str, slot: u8) -> Result<()> {
    validate_slot(slot)?;
    let device = select_device(controller)?;
    let mut store = load_slot_store()?;
    store.select_slot(&device.id, slot)?;
    save_slot_store(&store)?;
    eprintln!("Selected slot {slot} on {}", device_label(&device));
    Ok(())
}

fn clear_slot(controller: &str, slot: u8) -> Result<()> {
    validate_slot(slot)?;
    let device = select_device(controller)?;
    let mut store = load_slot_store()?;
    store.clear_slot(&device.id, slot)?;
    save_slot_store(&store)?;
    eprintln!("Cleared slot {slot} on {}", device_label(&device));
    Ok(())
}

fn apply_slot(controller: &str, slot: Option<u8>) -> Result<()> {
    let device = select_device(controller)?;
    let mut store = load_slot_store()?;
    let selected_slot = match slot {
        Some(slot) => {
            validate_slot(slot)?;
            store.select_slot(&device.id, slot)?;
            save_slot_store(&store)?;
            slot
        }
        None => store.selected_slot(&device.id),
    };
    let profile_id = store
        .profile_for_slot(&device.id, selected_slot)
        .ok_or_else(|| anyhow!("slot {selected_slot} on {} is empty", device_label(&device)))?
        .to_string();
    let profile = find_profile(&profile_id)?;

    eprintln!(
        "Applying slot {selected_slot} on {} with profile {}",
        device_label(&device),
        profile.id
    );
    run_foreground_profile(profile, device.path)
}

fn print_device_slots(store: &SlotStore, device_id: &str, name: Option<&str>) {
    let selected = store.selected_slot(device_id);
    for slot in 1..=SLOT_COUNT {
        let profile = store.profile_for_slot(device_id, slot).unwrap_or("-");
        let marker = if slot == selected { "*" } else { " " };
        if let Some(name) = name {
            println!("{device_id}\t{name}\t{marker}\tslot={slot}\tprofile={profile}");
        } else {
            println!("{device_id}\t{marker}\tslot={slot}\tprofile={profile}");
        }
    }
}

fn resolve_device_path(selector: &str) -> Result<String> {
    select_device(selector)
        .map(|device| device.path)
        .or_else(|_| resolve_device(selector))
}

fn select_device(selector: &str) -> Result<DeviceInfo> {
    list_devices()?
        .into_iter()
        .find(|device| device.id == selector || device.name == selector || device.path == selector)
        .or_else(|| fallback_absolute_device(selector))
        .ok_or_else(|| anyhow!("no input device matched {selector}"))
}

fn fallback_absolute_device(selector: &str) -> Option<DeviceInfo> {
    if !Path::new(selector).is_absolute() {
        return None;
    }

    Some(DeviceInfo {
        id: format!("path:{selector}"),
        name: selector.to_string(),
        path: selector.to_string(),
        device_kind: "path".to_string(),
        phys: String::new(),
        uniq: String::new(),
        bus: 0,
        vendor: 0,
        product: 0,
        version: 0,
        capabilities: Vec::new(),
    })
}

fn device_label(device: &DeviceInfo) -> String {
    if device.name == device.path {
        device.id.clone()
    } else {
        format!("{} ({})", device.name, device.id)
    }
}
