#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("QtQml/qqmlregistration.h");
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, devices)]
        #[qproperty(QString, profiles)]
        #[qproperty(QString, status)]
        #[qproperty(QString, profile_yaml)]
        #[qproperty(QString, editing_profile_path)]
        #[qproperty(QString, capture_status)]
        type PadProxyController = super::PadProxyControllerRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);

        #[qinvokable]
        fn new_profile(self: Pin<&mut Self>);

        #[qinvokable]
        fn edit_profile(self: Pin<&mut Self>, path: QString);

        #[qinvokable]
        fn save_profile(self: Pin<&mut Self>, yaml: QString);

        #[qinvokable]
        fn start_capture(self: Pin<&mut Self>, path: QString) -> QString;

        #[qinvokable]
        fn poll_capture_event(self: Pin<&mut Self>) -> QString;

        #[qinvokable]
        fn stop_capture(self: Pin<&mut Self>);
    }
}

use core::pin::Pin;
use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use padproxy_core::capture::CaptureReader;
use std::path::{Path, PathBuf};

pub fn init_qt_static_modules() {
    cxx_qt::init_crate!(cxx_qt);
    cxx_qt::init_crate!(cxx_qt_lib);
    cxx_qt::init_crate!(padproxy_gui);
    cxx_qt::init_qml_module!("com.benwbooth.padproxy");
}

#[derive(Default)]
pub struct PadProxyControllerRust {
    devices: QString,
    profiles: QString,
    status: QString,
    profile_yaml: QString,
    editing_profile_path: QString,
    capture_status: QString,
    capture_device_path: String,
    capture_reader: Option<CaptureReader>,
}

impl qobject::PadProxyController {
    pub fn refresh(self: Pin<&mut Self>) {
        refresh_into(self);
    }

    pub fn new_profile(self: Pin<&mut Self>) {
        let mut this = self;
        this.as_mut().set_profile_yaml(QString::from(
            "id: my-layout\n\
name: My layout\n\
description: Custom controller mapping.\n\
match:\n\
  name: \"*\"\n\
output:\n\
  type: xbox360\n\
passthrough: true\n\
grab_source: true\n\
mappings:\n\
  - from: btn:west\n\
    to: btn:east\n\
  - from: btn:east\n\
    to: btn:west\n",
        ));
        this.as_mut()
            .set_editing_profile_path(QString::from(user_profile_dir().display().to_string()));
        this.as_mut().set_status(QString::from(
            "Editing a new user profile. Save writes to ~/.config/padproxy/profiles.d.",
        ));
    }

    pub fn edit_profile(self: Pin<&mut Self>, path: QString) {
        let mut this = self;
        let path = path.to_string();
        match std::fs::read_to_string(&path) {
            Ok(yaml) => {
                this.as_mut().set_profile_yaml(QString::from(yaml));
                this.as_mut().set_editing_profile_path(QString::from(path));
                this.as_mut().set_status(QString::from(
                    "Editing profile YAML. Save writes a user profile copy.",
                ));
            }
            Err(error) => {
                this.as_mut().set_status(QString::from(format!(
                    "Failed to read profile {}: {}",
                    path, error
                )));
            }
        }
    }

    pub fn save_profile(self: Pin<&mut Self>, yaml: QString) {
        let mut this = self;
        let yaml = yaml.to_string();
        match save_user_profile(&yaml) {
            Ok(path) => {
                this.as_mut()
                    .set_editing_profile_path(QString::from(path.display().to_string()));
                this.as_mut()
                    .set_profile_yaml(QString::from(ensure_trailing_newline(yaml)));
                refresh_into(this.as_mut());
                this.as_mut().set_status(QString::from(format!(
                    "Saved profile to {}",
                    path.display()
                )));
            }
            Err(error) => {
                this.as_mut()
                    .set_status(QString::from(format!("Failed to save profile: {error}")));
            }
        }
    }

    pub fn start_capture(self: Pin<&mut Self>, path: QString) -> QString {
        let mut this = self;
        let path = path.to_string();
        match CaptureReader::open(&path) {
            Ok(reader) => {
                {
                    let mut rust = this.as_mut().rust_mut();
                    rust.capture_device_path = path.clone();
                    rust.capture_reader = Some(reader);
                }
                this.as_mut().set_capture_status(QString::from(format!(
                    "Listening on {}",
                    display_device_path(&path)
                )));
                QString::from("ok")
            }
            Err(error) => {
                {
                    let mut rust = this.as_mut().rust_mut();
                    rust.capture_device_path.clear();
                    rust.capture_reader = None;
                }
                this.as_mut()
                    .set_capture_status(QString::from(format!("Hook mode failed: {error}")));
                QString::from("")
            }
        }
    }

    pub fn poll_capture_event(self: Pin<&mut Self>) -> QString {
        let mut this = self;
        let result = {
            let mut rust = this.as_mut().rust_mut();
            match rust.capture_reader.as_mut() {
                Some(reader) => reader.poll(),
                None => Ok(None),
            }
        };

        match result {
            Ok(Some(event)) => {
                this.as_mut()
                    .set_capture_status(QString::from(format!("Captured {event}")));
                QString::from(event)
            }
            Ok(None) => QString::from(""),
            Err(error) => {
                {
                    let mut rust = this.as_mut().rust_mut();
                    rust.capture_device_path.clear();
                    rust.capture_reader = None;
                }
                this.as_mut()
                    .set_capture_status(QString::from(format!("Hook mode failed: {error}")));
                QString::from("")
            }
        }
    }

    pub fn stop_capture(self: Pin<&mut Self>) {
        let mut this = self;
        {
            let mut rust = this.as_mut().rust_mut();
            rust.capture_device_path.clear();
            rust.capture_reader = None;
        }
        this.as_mut()
            .set_capture_status(QString::from("Hook mode off"));
    }
}

fn refresh_json() -> anyhow::Result<(String, String, String)> {
    let devices = padproxy_core::linux::list_devices()?;
    let profiles =
        padproxy_core::profiles::load_profiles(&padproxy_core::profiles::default_profile_dirs())?;
    let status = format!(
        "Loaded {} controller(s), {} profile(s)",
        devices.len(),
        profiles.len()
    );

    Ok((
        serde_json::to_string(&devices)?,
        serde_json::to_string(&profiles)?,
        status,
    ))
}

fn refresh_into(mut controller: Pin<&mut qobject::PadProxyController>) {
    match refresh_json() {
        Ok((devices, profiles, status)) => {
            controller
                .as_mut()
                .set_devices(QString::from(devices.as_str()));
            controller
                .as_mut()
                .set_profiles(QString::from(profiles.as_str()));
            controller
                .as_mut()
                .set_status(QString::from(status.as_str()));
        }
        Err(error) => {
            controller.as_mut().set_devices(QString::from("[]"));
            controller.as_mut().set_profiles(QString::from("[]"));
            controller
                .as_mut()
                .set_status(QString::from(format!("Refresh failed: {error}")));
        }
    }
}

fn save_user_profile(yaml: &str) -> anyhow::Result<PathBuf> {
    let yaml = ensure_trailing_newline(yaml.to_string());
    let profile =
        padproxy_core::profiles::parse_profile_bytes(yaml.as_bytes(), Path::new("profile.yaml"))?;
    let id = profile_file_stem(&profile.id)?;
    let dir = user_profile_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{id}.yaml"));
    std::fs::write(&path, yaml)?;
    Ok(path)
}

fn user_profile_dir() -> PathBuf {
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("padproxy/profiles.d");
    }

    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".config/padproxy/profiles.d");
    }

    PathBuf::from(".").join("profiles")
}

fn profile_file_stem(id: &str) -> anyhow::Result<String> {
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
        anyhow::bail!("profile id must contain at least one letter or number");
    }

    Ok(stem)
}

fn ensure_trailing_newline(mut text: String) -> String {
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn display_device_path(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}
