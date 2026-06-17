#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, devices)]
        #[qproperty(QString, profiles)]
        #[qproperty(QString, status)]
        type PadProxyController = super::PadProxyControllerRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);
    }
}

use core::pin::Pin;
use cxx_qt_lib::QString;

pub fn init_qt_static_modules() {
    cxx_qt::init_crate!(cxx_qt);
    cxx_qt::init_crate!(cxx_qt_lib);
    cxx_qt::init_crate!(padproxy);
    cxx_qt::init_qml_module!("com.benwbooth.padproxy");
}

#[derive(Default)]
pub struct PadProxyControllerRust {
    devices: QString,
    profiles: QString,
    status: QString,
}

impl qobject::PadProxyController {
    pub fn refresh(self: Pin<&mut Self>) {
        let mut this = self;
        match refresh_json() {
            Ok((devices, profiles, status)) => {
                this.as_mut().set_devices(QString::from(devices.as_str()));
                this.as_mut().set_profiles(QString::from(profiles.as_str()));
                this.as_mut().set_status(QString::from(status.as_str()));
            }
            Err(error) => {
                this.as_mut().set_devices(QString::from("[]"));
                this.as_mut().set_profiles(QString::from("[]"));
                this.as_mut()
                    .set_status(QString::from(format!("Refresh failed: {error}").as_str()));
            }
        }
    }
}

fn refresh_json() -> anyhow::Result<(String, String, String)> {
    let devices = crate::linux::list_devices()?;
    let profiles = crate::profiles::load_profiles(&crate::profiles::default_profile_dirs())?;
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
