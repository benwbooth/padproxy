use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use padproxy_core::autodetect::{
    decide_watch, detect_profile, match_profile, running_process_names, WatchDecision,
};
use padproxy_core::blocklist::{blocklist_path, load_blocklist};
use padproxy_core::bluetooth::power_off;
use padproxy_core::devices::DeviceInfo;
use padproxy_core::leds::{list_leds, set_led_brightness, set_led_color, LedInfo};
use padproxy_core::linux::{list_devices, resolve_device, resolve_device_info};
use padproxy_core::outputs::output_devices;
use padproxy_core::power::{list_batteries, BatteryInfo};
use padproxy_core::presets::{export_profile_yaml, install_profile};
use padproxy_core::profiles::{default_profile_dirs, load_profiles, Profile};
use padproxy_core::remapper::{
    launch_with_remap, LaunchOptions, RemapOptions, RemapRuntime, SlotRequest,
};
use padproxy_core::service::ServiceState;
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
    ListLeds,
    SetLed {
        #[arg(long)]
        led: String,
        #[arg(long)]
        brightness: Option<u32>,
        /// Space-separated RGB channel intensities, e.g. "255 0 0".
        #[arg(long)]
        color: Option<String>,
    },
    ListBlocklist,
    PowerOff {
        #[arg(long)]
        controller: String,
    },
    MobileServer {
        #[arg(long, default_value_t = 9999)]
        port: u16,
        #[arg(long, default_value = "xbox360")]
        output: String,
    },
    Detect,
    Watch {
        #[arg(long)]
        controller: String,
        #[arg(long, default_value_t = 2000)]
        interval_ms: u64,
    },
    Serve {
        #[arg(long)]
        socket: Option<PathBuf>,
    },
    Ctl {
        #[arg(long)]
        socket: Option<PathBuf>,
        #[arg(long)]
        request: String,
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
    ExportProfile {
        #[arg(long)]
        profile: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    ImportProfile {
        #[arg(long)]
        path: PathBuf,
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
        /// Profile id/name. When omitted, the profile is auto-selected by
        /// matching the launched command against profiles' `process:` patterns.
        #[arg(long)]
        profile: Option<String>,
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
        Command::ListLeds => {
            let leds = list_leds();
            if leds.is_empty() {
                eprintln!("No LED devices reported.");
            }
            for led in leds {
                let brightness = led
                    .brightness
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "?".to_string());
                let max = led
                    .max_brightness
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "?".to_string());
                let color = led
                    .multi_intensity
                    .map(|value| format!("\trgb={value}"))
                    .unwrap_or_default();
                println!("{}\t{brightness}/{max}{color}", led.name);
            }
            Ok(())
        }
        Command::SetLed {
            led,
            brightness,
            color,
        } => set_led(&led, brightness, color.as_deref()),
        Command::ListBlocklist => {
            let blocklist = load_blocklist();
            if blocklist.is_empty() {
                eprintln!("Blocklist is empty ({}).", blocklist_path().display());
            }
            for pattern in &blocklist.patterns {
                println!("{pattern}");
            }
            Ok(())
        }
        Command::PowerOff { controller } => {
            let device = select_device(&controller)?;
            power_off(&device)?;
            eprintln!("Powered off {}", device_label(&device));
            Ok(())
        }
        Command::MobileServer { port, output } => mobile_server(port, &output),
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
        Command::Serve { socket } => run_serve(socket),
        Command::Ctl { socket, request } => run_ctl(socket, &request),
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
        Command::ExportProfile { profile, output } => export_profile(&profile, output.as_deref()),
        Command::ImportProfile { path } => import_profile(&path),
        Command::Launch {
            profile,
            controller,
            command,
        } => {
            let profile = resolve_launch_profile(profile.as_deref(), &command)?;
            let source_device_path = resolve_device_path(&controller)?;
            eprintln!("Launching with profile {}", profile.id);
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

/// Resolve the profile for a `launch`: explicit id/name when given, otherwise
/// match the launched command against profiles' `process:` patterns.
fn resolve_launch_profile(profile: Option<&str>, command: &[String]) -> Result<Profile> {
    if let Some(selector) = profile {
        return find_profile(selector);
    }

    let program = command
        .first()
        .ok_or_else(|| anyhow!("launch command is empty"))?;
    let profiles = load_profiles(&default_profile_dirs())?;
    match match_profile(&profiles, &[program.clone()]) {
        Some((matched, _)) => Ok(matched.clone()),
        None => Err(anyhow!(
            "no profile's process patterns matched {program}; pass --profile to choose one"
        )),
    }
}

fn mobile_server(port: u16, output: &str) -> Result<()> {
    padproxy_core::mobile::run_server(port, output)
}

fn set_led(led: &str, brightness: Option<u32>, color: Option<&str>) -> Result<()> {
    if brightness.is_none() && color.is_none() {
        return Err(anyhow!("set-led requires --brightness and/or --color"));
    }
    if let Some(brightness) = brightness {
        set_led_brightness(led, brightness)?;
        eprintln!("Set LED {led} brightness to {brightness}");
    }
    if let Some(color) = color {
        let channels = color
            .split_whitespace()
            .map(|value| value.parse::<u32>())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| anyhow!("--color must be space-separated integers, e.g. \"255 0 0\""))?;
        set_led_color(led, &channels)?;
        eprintln!("Set LED {led} color to {color}");
    }
    Ok(())
}

fn find_profile(selector: &str) -> Result<Profile> {
    load_profiles(&default_profile_dirs())?
        .into_iter()
        .find(|profile| profile.id == selector || profile.name == selector)
        .ok_or_else(|| anyhow!("no profile matched {selector}"))
}

fn export_profile(selector: &str, output: Option<&Path>) -> Result<()> {
    let profile = find_profile(selector)?;
    let yaml = export_profile_yaml(&profile)?;
    if let Some(output) = output {
        fs::write(output, &yaml)
            .with_context(|| format!("failed to write {}", output.display()))?;
        eprintln!("Exported profile {} to {}", profile.id, output.display());
    } else {
        print!("{yaml}");
    }
    Ok(())
}

fn import_profile(path: &Path) -> Result<()> {
    let yaml =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let installed = install_profile(&yaml)?;
    eprintln!("Imported profile to {}", installed.display());
    Ok(())
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
        let blocklist = load_blocklist();
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
        match decide_watch(&profiles, &names, active_id, &blocklist) {
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

fn default_socket_path() -> PathBuf {
    if let Some(runtime_dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(runtime_dir).join("padproxy.sock");
    }
    PathBuf::from("/tmp").join(format!("padproxy-{}.sock", std::process::id()))
}

/// Run the local control API over a Unix socket.
fn run_serve(socket: Option<PathBuf>) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;

    let socket_path = socket.unwrap_or_else(default_socket_path);
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)
            .with_context(|| format!("failed to remove stale socket {}", socket_path.display()))?;
    }
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let listener = UnixListener::bind(&socket_path)
        .with_context(|| format!("failed to bind {}", socket_path.display()))?;
    eprintln!(
        "PadProxy control API listening on {}. Press Ctrl-C to stop.",
        socket_path.display()
    );

    let mut state = ServiceState::new();

    for stream in listener.incoming() {
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                eprintln!("PadProxy connection error: {error}");
                continue;
            }
        };
        let reader_stream = stream.try_clone()?;
        let reader = BufReader::new(reader_stream);
        for line in reader.lines() {
            let line = match line {
                Ok(line) => line,
                Err(_) => break,
            };
            if line.trim().is_empty() {
                continue;
            }
            let response = state.handle_json(&line);
            if writeln!(stream, "{response}").is_err() {
                break;
            }
        }
    }

    Ok(())
}

/// Send a single JSON request to a running control API and print the response.
fn run_ctl(socket: Option<PathBuf>, request: &str) -> Result<()> {
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;

    let socket_path = socket.unwrap_or_else(default_socket_path);
    let mut stream = UnixStream::connect(&socket_path)
        .with_context(|| format!("failed to connect to {}", socket_path.display()))?;
    writeln!(stream, "{}", request.trim()).context("failed to send request")?;

    let reader_stream = stream.try_clone()?;
    let mut reader = BufReader::new(reader_stream);
    let mut response = String::new();
    reader
        .read_line(&mut response)
        .context("failed to read response")?;
    print!("{response}");
    Ok(())
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
    let mut current = match slot {
        Some(slot) => {
            validate_slot(slot)?;
            store.select_slot(&device.id, slot)?;
            save_slot_store(&store)?;
            slot
        }
        None => store.selected_slot(&device.id),
    };

    // Run the selected slot's profile, restarting when a slot-switch command
    // mapping fires, until a remap_off (or Ctrl-C) ends the session.
    loop {
        let profile_id = store
            .profile_for_slot(&device.id, current)
            .ok_or_else(|| anyhow!("slot {current} on {} is empty", device_label(&device)))?
            .to_string();
        let profile = find_profile(&profile_id)?;
        eprintln!(
            "Applying slot {current} on {} with profile {}",
            device_label(&device),
            profile.id
        );

        let mut runtime = RemapRuntime::start(RemapOptions {
            profile,
            source_device_path: device.path.clone(),
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

        match runtime.take_slot_request() {
            Some(request) => {
                drop(runtime);
                current = next_slot(&store, &device.id, current, request);
                store.select_slot(&device.id, current)?;
                save_slot_store(&store)?;
                eprintln!("Switched to slot {current}");
            }
            None => {
                eprintln!("PadProxy remap turned off.");
                return Ok(());
            }
        }
    }
}

/// Compute the next slot for a slot-switch request, skipping empty slots for
/// next/prev cycling.
fn next_slot(store: &SlotStore, device_id: &str, current: u8, request: SlotRequest) -> u8 {
    match request {
        SlotRequest::Select(slot) if validate_slot(slot).is_ok() => slot,
        SlotRequest::Select(_) => current,
        SlotRequest::Next | SlotRequest::Prev => {
            let forward = matches!(request, SlotRequest::Next);
            for step in 1..=SLOT_COUNT {
                let candidate = if forward {
                    (current - 1 + step) % SLOT_COUNT + 1
                } else {
                    (current - 1 + SLOT_COUNT - step) % SLOT_COUNT + 1
                };
                if store.profile_for_slot(device_id, candidate).is_some() {
                    return candidate;
                }
            }
            current
        }
    }
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
    resolve_device_info(selector)
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
    leds: Vec<LedInfo>,
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
        leds: list_leds(),
        outputs,
        profiles,
        slots: load_slot_store()?,
    })
}
