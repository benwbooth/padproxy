use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use padproxy_core::linux::{list_devices, resolve_device};
use padproxy_core::outputs::output_devices;
use padproxy_core::profiles::{default_profile_dirs, load_profiles, Profile};
use padproxy_core::remapper::{launch_with_remap, LaunchOptions, RemapOptions, RemapRuntime};

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
        Command::Launch {
            profile,
            controller,
            command,
        } => {
            let profile = find_profile(&profile)?;
            let source_device_path = resolve_device(&controller)?;
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
    let source_device_path = resolve_device(controller)?;
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
