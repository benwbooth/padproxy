use anyhow::{anyhow, Result};
use clap::{Parser, Subcommand};
use padproxy_core::linux::{list_devices, resolve_device};
use padproxy_core::profiles::{default_profile_dirs, load_profiles, Profile};
use padproxy_core::remapper::{launch_with_remap, LaunchOptions};

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    ListDevices,
    ListProfiles,
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
                    "{}\t{}\t{}\tid={}",
                    device.path,
                    device.hardware_id(),
                    device.name,
                    device.id
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
    }
}

fn find_profile(selector: &str) -> Result<Profile> {
    load_profiles(&default_profile_dirs())?
        .into_iter()
        .find(|profile| profile.id == selector || profile.name == selector)
        .ok_or_else(|| anyhow!("no profile matched {selector}"))
}
