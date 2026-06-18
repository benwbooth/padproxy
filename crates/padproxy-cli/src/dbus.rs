//! DBus front end for the PadProxy control API.
//!
//! Exposes the same JSON request/response protocol as the Unix-socket control
//! API on the session bus, so desktop tools can drive PadProxy over DBus.

use anyhow::{Context, Result};
use padproxy_core::service::ServiceState;
use std::sync::Mutex;

struct PadProxyDbus {
    state: Mutex<ServiceState>,
}

#[zbus::interface(name = "com.benwbooth.PadProxy1")]
impl PadProxyDbus {
    /// Handle one JSON control request and return the JSON response (the same
    /// protocol as the `serve`/`ctl` socket API).
    fn request(&self, json: String) -> String {
        let mut state = self.state.lock().expect("padproxy dbus state poisoned");
        state.handle_json(&json)
    }
}

/// Run the DBus control service on the session bus until interrupted.
pub fn serve_dbus() -> Result<()> {
    let service = PadProxyDbus {
        state: Mutex::new(ServiceState::new()),
    };

    let _connection = zbus::blocking::connection::Builder::session()
        .context("failed to connect to the session bus")?
        .name("com.benwbooth.PadProxy")
        .context("failed to request the bus name com.benwbooth.PadProxy")?
        .serve_at("/com/benwbooth/PadProxy", service)
        .context("failed to serve the PadProxy object")?
        .build()
        .context("failed to build the DBus connection")?;

    eprintln!(
        "PadProxy DBus service active on com.benwbooth.PadProxy \
         (/com/benwbooth/PadProxy, method com.benwbooth.PadProxy1.Request). Press Ctrl-C to stop."
    );
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
