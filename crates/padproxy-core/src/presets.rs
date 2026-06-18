//! Profile preset import/export and user-profile installation.
//!
//! Profiles are shareable YAML presets. This module installs validated presets
//! into the user profile directory and exports existing profiles back to YAML,
//! so configs can be exchanged (the local half of reWASD's community presets).

use crate::profiles::{parse_profile_bytes, Profile};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// The directory where user-owned profiles are written.
pub fn user_profile_dir() -> PathBuf {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("padproxy/profiles.d");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/padproxy/profiles.d");
    }

    PathBuf::from(".").join("profiles")
}

/// Convert a profile id into a safe file stem (alphanumeric plus `-_.`).
pub fn profile_file_stem(id: &str) -> Result<String> {
    let stem = id
        .chars()
        .map(|value| {
            if value.is_ascii_alphanumeric() || matches!(value, '-' | '_' | '.') {
                value
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches(['-', '.', '_'])
        .to_string();

    if stem.is_empty() {
        bail!("profile id must contain at least one letter or number");
    }

    Ok(stem)
}

/// Validate and install a profile from YAML into the user profile directory.
/// Returns the path written.
pub fn install_profile(yaml: &str) -> Result<PathBuf> {
    install_profile_into(&user_profile_dir(), yaml)
}

/// Validate and install a profile from YAML into a specific directory.
pub fn install_profile_into(dir: &Path, yaml: &str) -> Result<PathBuf> {
    let yaml = ensure_trailing_newline(yaml);
    let profile = parse_profile_bytes(yaml.as_bytes(), Path::new("profile.yaml"))
        .context("preset is not a valid profile")?;
    let stem = profile_file_stem(&profile.id)?;
    std::fs::create_dir_all(dir).with_context(|| format!("failed to create {}", dir.display()))?;
    let path = dir.join(format!("{stem}.yaml"));
    std::fs::write(&path, yaml).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(path)
}

/// Read a profile's source YAML for export.
pub fn export_profile_yaml(profile: &Profile) -> Result<String> {
    std::fs::read_to_string(&profile.source_path)
        .with_context(|| format!("failed to read {}", profile.source_path.display()))
}

fn ensure_trailing_newline(value: &str) -> String {
    if value.ends_with('\n') {
        value.to_string()
    } else {
        format!("{value}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{install_profile_into, profile_file_stem};

    #[test]
    fn sanitizes_profile_ids_into_file_stems() {
        assert_eq!(profile_file_stem("nes-2button").unwrap(), "nes-2button");
        assert_eq!(profile_file_stem("My Game!/v2").unwrap(), "My-Game--v2");
        assert!(profile_file_stem("///").is_err());
    }

    #[test]
    fn installs_valid_preset_named_by_id() {
        let dir = std::env::temp_dir().join(format!("padproxy-preset-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let yaml = "id: imported-game\nprocess: [coolgame]\nmappings: []";
        let path = install_profile_into(&dir, yaml).unwrap();
        assert_eq!(path, dir.join("imported-game.yaml"));

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.ends_with('\n'));
        assert!(written.contains("imported-game"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_invalid_preset() {
        let dir = std::env::temp_dir().join(format!("padproxy-preset-bad-{}", std::process::id()));
        let result = install_profile_into(&dir, "this: [is not: valid: yaml");
        assert!(result.is_err());
    }
}
