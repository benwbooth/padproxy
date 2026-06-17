use crate::devices::DeviceInfo;
use crate::event_code::{parse_event_code, virtual_xbox_supports, EventCode};
use crate::outputs::normalize_output_type;
use anyhow::{anyhow, Context, Result};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct DeviceMatch {
    pub name: Option<String>,
    pub serial: Option<String>,
    pub phys: Option<String>,
    pub vendor_id: Option<IdValue>,
    pub product_id: Option<IdValue>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdValue(u16);

#[derive(Clone, Debug, Serialize)]
pub struct Mapping {
    pub from: EventCode,
    pub to: EventCode,
    pub from_name: String,
    pub to_name: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub device_match: DeviceMatch,
    pub output_type: String,
    pub passthrough: bool,
    pub grab_source: bool,
    pub source_path: PathBuf,
    pub mappings: Vec<Mapping>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    r#match: DeviceMatch,
    output: Option<RawOutput>,
    passthrough: Option<bool>,
    grab_source: Option<bool>,
    mappings: Option<Vec<RawMapping>>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawOutput {
    Scalar(String),
    Object { r#type: Option<String> },
}

#[derive(Debug, Deserialize)]
struct RawMapping {
    from: String,
    to: String,
}

impl IdValue {
    pub fn as_u16(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for IdValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IdValueVisitor;

        impl Visitor<'_> for IdValueVisitor {
            type Value = IdValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a 16-bit integer or hex string like 0x045e")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u16::try_from(value)
                    .map(IdValue)
                    .map_err(|_| E::custom(format!("device id {value} is outside u16 range")))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 {
                    return Err(E::custom(format!("device id {value} must be non-negative")));
                }
                self.visit_u64(value as u64)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let trimmed = value.trim();
                let parsed = if let Some(hex) = trimmed
                    .strip_prefix("0x")
                    .or_else(|| trimmed.strip_prefix("0X"))
                {
                    u16::from_str_radix(hex, 16)
                } else {
                    trimmed.parse()
                };

                parsed
                    .map(IdValue)
                    .map_err(|_| E::custom(format!("invalid device id {value:?}")))
            }
        }

        deserializer.deserialize_any(IdValueVisitor)
    }
}

impl Serialize for IdValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.0)
    }
}

impl DeviceMatch {
    pub fn matches(&self, device: &DeviceInfo) -> bool {
        if let Some(name) = &self.name {
            if name != "*"
                && !device
                    .name
                    .to_ascii_lowercase()
                    .contains(&name.to_ascii_lowercase())
            {
                return false;
            }
        }
        if let Some(serial) = &self.serial {
            if &device.uniq != serial {
                return false;
            }
        }
        if let Some(phys) = &self.phys {
            if &device.phys != phys {
                return false;
            }
        }
        if let Some(vendor_id) = self.vendor_id {
            if device.vendor != vendor_id.as_u16() {
                return false;
            }
        }
        if let Some(product_id) = self.product_id {
            if device.product != product_id.as_u16() {
                return false;
            }
        }
        true
    }
}

impl Profile {
    pub fn mapping_table(&self) -> HashMap<EventCode, EventCode> {
        self.mappings
            .iter()
            .map(|mapping| (mapping.from, mapping.to))
            .collect()
    }
}

pub fn default_profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let mut have_configured_profile_dir = false;

    if let Some(dir) = std::env::var_os("PADPROXY_PROFILE_DIR") {
        for path in std::env::split_paths(&dir) {
            push_profile_dir(&mut dirs, path);
            have_configured_profile_dir = true;
        }
    }

    if !have_configured_profile_dir {
        push_profile_dir(
            &mut dirs,
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("profiles"),
        );
    }

    if let Some(home) = std::env::var_os("HOME") {
        push_profile_dir(
            &mut dirs,
            PathBuf::from(home).join(".config/padproxy/profiles.d"),
        );
    }

    push_profile_dir(&mut dirs, PathBuf::from("/etc/padproxy/profiles.d"));
    dirs
}

fn push_profile_dir(dirs: &mut Vec<PathBuf>, dir: PathBuf) {
    if !dirs.iter().any(|existing| existing == &dir) {
        dirs.push(dir);
    }
}

pub fn load_profiles(dirs: &[PathBuf]) -> Result<Vec<Profile>> {
    let mut profiles = HashMap::new();

    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }

        for entry in
            fs::read_dir(dir).with_context(|| format!("failed to list {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let extension = path.extension().and_then(|value| value.to_str());
            if !matches!(extension, Some("yaml" | "yml")) {
                continue;
            }
            let profile = load_profile(&path)?;
            profiles.insert(profile.id.clone(), profile);
        }
    }

    let mut profiles = profiles.into_values().collect::<Vec<_>>();
    profiles.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(profiles)
}

pub fn load_profile(path: &Path) -> Result<Profile> {
    let bytes = fs::read(path).with_context(|| format!("failed to read {}", path.display()))?;
    parse_profile_bytes(&bytes, path)
}

pub fn parse_profile_bytes(bytes: &[u8], source_path: &Path) -> Result<Profile> {
    let raw: RawProfile = serde_yaml::from_slice(&bytes)
        .with_context(|| format!("failed to parse {}", source_path.display()))?;

    let id = raw.id.unwrap_or_else(|| {
        source_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    });
    let name = raw.name.unwrap_or_else(|| id.clone());
    let output_type = match raw.output {
        Some(RawOutput::Scalar(value)) => value,
        Some(RawOutput::Object { r#type }) => r#type.unwrap_or_else(|| "xbox360".to_string()),
        None => "xbox360".to_string(),
    };
    let output_type = normalize_output_type(&output_type);

    let mut mappings = Vec::new();
    for mapping in raw.mappings.unwrap_or_default() {
        let from = parse_event_code(&mapping.from)
            .ok_or_else(|| anyhow!("unknown source event {}", mapping.from))?;
        let to = parse_event_code(&mapping.to)
            .ok_or_else(|| anyhow!("unknown target event {}", mapping.to))?;
        if !virtual_xbox_supports(to) {
            return Err(anyhow!(
                "target event {} is not supported by the current virtual pad",
                mapping.to
            ));
        }
        mappings.push(Mapping {
            from,
            to,
            from_name: from.name(),
            to_name: to.name(),
        });
    }

    Ok(Profile {
        id,
        name,
        description: raw.description.unwrap_or_default(),
        device_match: raw.r#match,
        output_type,
        passthrough: raw.passthrough.unwrap_or(true),
        grab_source: raw.grab_source.unwrap_or(true),
        source_path: source_path.to_path_buf(),
        mappings,
    })
}
