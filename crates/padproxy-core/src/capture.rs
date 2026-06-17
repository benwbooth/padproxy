use crate::event_code::{capture_event_code, virtual_xbox_supports};
use anyhow::{Context, Result};
use evdev::Device;

pub struct CaptureReader {
    device: Device,
}

impl CaptureReader {
    pub fn open(path: &str) -> Result<Self> {
        let device = Device::open(path).with_context(|| format!("failed to open {path}"))?;
        device
            .set_nonblocking(true)
            .with_context(|| format!("failed to make {path} nonblocking"))?;
        let mut reader = Self { device };
        reader.drain_stale_events()?;
        Ok(reader)
    }

    pub fn poll(&mut self) -> Result<Option<String>> {
        match self.device.fetch_events() {
            Ok(events) => {
                for event in events {
                    let Some(code) = capture_event_code(event) else {
                        continue;
                    };
                    if virtual_xbox_supports(code) {
                        return Ok(Some(code.name()));
                    }
                }
                Ok(None)
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(error).context("failed reading capture events"),
        }
    }

    fn drain_stale_events(&mut self) -> Result<()> {
        loop {
            match self.device.fetch_events() {
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
                Err(error) => return Err(error).context("failed draining capture events"),
            }
        }
    }
}
