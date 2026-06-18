use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use padproxy_core::autodetect::{
    decide_watch, detect_profile, match_profile, running_process_names, WatchDecision,
};
use padproxy_core::devices::DeviceInfo;
use padproxy_core::linux::{list_devices, resolve_device};
use padproxy_core::outputs::output_devices;
use padproxy_core::power::{list_batteries, BatteryInfo};
use padproxy_core::profiles::{default_profile_dirs, load_profiles, Profile};
use padproxy_core::remapper::{launch_with_remap, LaunchOptions, RemapOptions, RemapRuntime};
use padproxy_core::slots::{
    load_slot_store, save_slot_store, validate_slot, SlotStore, SLOT_COUNT,
};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
    ListBatteries,
    Detect,
    Watch {
        #[arg(long)]
        controller: String,
        #[arg(long, default_value_t = 2000)]
        interval_ms: u64,
    },
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
    Diagnostics {
        #[arg(long)]
        output: Option<PathBuf>,
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
        Command::Detect => {
            let profiles = load_profiles(&default_profile_dirs())?;
            match detect_profile(&profiles) {
                Some((profile, detail)) => {
                    println!("{}\t{}\t{}", profile.id, profile.name, detail.process_name);
                    Ok(())
                }
                None => {
                    eprintln!("No profile matched a running process.");
                    std::process::exit(1);
                }
            }
        }
        Command::ListBatteries => {
            let batteries = list_batteries();
            if batteries.is_empty() {
                eprintln!("No device batteries reported.");
            }
            for battery in batteries {
                let capacity = battery
                    .capacity
                    .map(|value| format!("{value}%"))
                    .or(battery.capacity_level)
                    .unwrap_or_else(|| "unknown".to_string());
                let status = battery.status.unwrap_or_else(|| "Unknown".to_string());
                let model = battery.model.unwrap_or_else(|| battery.name.clone());
                println!("{}\t{}\t{}\t{}", battery.name, model, capacity, status);
            }
            Ok(())
        }
        Command::Watch {
            controller,
            interval_ms,
        } => run_watch(&controller, interval_ms),
        Command::ListSlots { controller } => list_slots(controller.as_deref()),
        Command::AssignSlot {
            controller,
            slot,
            profile,
        } => assign_slot(&controller, slot, &profile),
        Command::SelectSlot { controller, slot } => select_slot(&controller, slot),
        Command::ClearSlot { controller, slot } => clear_slot(&controller, slot),
        Command::ApplySlot { controller, slot } => apply_slot(&controller, slot),
        Command::Diagnostics { output } => export_diagnostics(output.as_deref()),
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

    while !runtime.stop_requested() {
        runtime.pump_once()?;
    }

    eprintln!("PadProxy remap turned off by a remap_off command mapping.");
    Ok(())
}

/// Watch running processes and apply the matching profile automatically,
/// switching when the foreground game changes and stopping when it exits.
fn run_watch(controller: &str, interval_ms: u64) -> Result<()> {
    let source_device_path = resolve_device_path(controller)?;
    let poll = Duration::from_millis(interval_ms.max(50));
    eprintln!(
        "PadProxy is watching for known game processes on {source_device_path}. Press Ctrl-C to stop."
    );

    let mut active: Option<(RemapRuntime, String)> = None;
    // Profile turned off by a remap_off command; do not re-apply until its
    // process stops matching.
    let mut suppressed: Option<String> = None;

    loop {
        let profiles = load_profiles(&default_profile_dirs())?;
        let names = running_process_names();

        // Clear suppression once the suppressed profile no longer matches.
        if let Some(id) = &suppressed {
            let still_matching = match_profile(&profiles, &names)
                .map(|(profile, _)| &profile.id == id)
                .unwrap_or(false);
            if !still_matching {
                suppressed = None;
            }
        }

        let active_id = active.as_ref().map(|(_, id)| id.as_str());
        match decide_watch(&profiles, &names, active_id) {
            WatchDecision::Keep => {}
            WatchDecision::Stop => {
                if active.take().is_some() {
                    eprintln!("PadProxy stopped remap; matched process exited.");
                }
            }
            WatchDecision::Switch(id) => {
                if suppressed.as_deref() != Some(id.as_str()) {
                    let profile = profiles
                        .into_iter()
                        .find(|profile| profile.id == id)
                        .ok_or_else(|| anyhow!("profile {id} disappeared during watch"))?;
                    let runtime = RemapRuntime::start(RemapOptions {
                        profile,
                        source_device_path: source_device_path.clone(),
                    })?;
                    eprintln!("PadProxy applied profile {id}.");
                    active = Some((runtime, id));
                }
            }
        }

        // Pump the active remap until the next poll, watching for remap_off.
        let deadline = Instant::now() + poll;
        if let Some((runtime, id)) = active.as_mut() {
            while Instant::now() < deadline && !runtime.stop_requested() {
                runtime.pump_once()?;
            }
            if runtime.stop_requested() {
                eprintln!("PadProxy remap turned off by a remap_off command mapping.");
                suppressed = Some(id.clone());
                active = None;
            }
        } else {
            std::thread::sleep(poll);
        }
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

#[derive(Serialize)]
struct DiagnosticsReport {
    version: &'static str,
    generated_at_unix_ms: u128,
    platform: DiagnosticsPlatform,
    paths: DiagnosticsPaths,
    devices: Vec<DeviceInfo>,
    batteries: Vec<BatteryInfo>,
    outputs: Vec<DiagnosticsOutput>,
    profiles: Vec<DiagnosticsProfile>,
    slots: SlotStore,
}

#[derive(Serialize)]
struct DiagnosticsPlatform {
    os: &'static str,
    arch: &'static str,
}

#[derive(Serialize)]
struct DiagnosticsPaths {
    profile_dirs: Vec<String>,
    slot_store: String,
}

#[derive(Serialize)]
struct DiagnosticsOutput {
    id: String,
    label: String,
    supported: bool,
    note: String,
}

#[derive(Serialize)]
struct DiagnosticsProfile {
    id: String,
    name: String,
    description: String,
    output_type: String,
    source_path: String,
    layer_count: usize,
    mapping_count: usize,
}

fn export_diagnostics(output_path: Option<&Path>) -> Result<()> {
    let report = collect_diagnostics()?;
    let json = serde_json::to_string_pretty(&report)?;

    if let Some(output_path) = output_path {
        fs::write(output_path, json)?;
        eprintln!("Wrote diagnostics to {}", output_path.display());
    } else {
        println!("{json}");
    }

    Ok(())
}

fn collect_diagnostics() -> Result<DiagnosticsReport> {
    let profile_dirs = default_profile_dirs();
    let profiles = load_profiles(&profile_dirs)?
        .into_iter()
        .map(|profile| DiagnosticsProfile {
            id: profile.id,
            name: profile.name,
            description: profile.description,
            output_type: profile.output_type,
            source_path: profile.source_path.display().to_string(),
            layer_count: profile.layers.len(),
            mapping_count: profile
                .layers
                .iter()
                .map(|layer| layer.mappings.len())
                .sum(),
        })
        .collect();
    let outputs = output_devices()
        .iter()
        .map(|output| DiagnosticsOutput {
            id: output.id.to_string(),
            label: output.label.to_string(),
            supported: output.supported,
            note: output.note.to_string(),
        })
        .collect();

    Ok(DiagnosticsReport {
        version: env!("CARGO_PKG_VERSION"),
        generated_at_unix_ms: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
        platform: DiagnosticsPlatform {
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
        },
        paths: DiagnosticsPaths {
            profile_dirs: profile_dirs
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            slot_store: padproxy_core::slots::slot_store_path()
                .display()
                .to_string(),
        },
        devices: list_devices()?,
        batteries: list_batteries(),
        outputs,
        profiles,
        slots: load_slot_store()?,
    })
}
