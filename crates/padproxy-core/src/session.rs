//! Background remap session management.
//!
//! A [`RemapSession`] owns a worker thread that builds a [`RemapRuntime`] and
//! pumps it until stopped, reporting lifecycle messages over a channel. Both the
//! GUI and the control daemon use this so remap control lives in one place.

use crate::profiles::Profile;
use crate::remapper::{RemapOptions, RemapRuntime};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

/// Lifecycle messages emitted by a running [`RemapSession`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemapMessage {
    /// The remap started; carries the virtual device node paths.
    Running(Vec<String>),
    /// The remap stopped cleanly (stop requested or `remap_off`).
    Stopped,
    /// The remap failed to start or errored while running.
    Failed(String),
}

/// A running remap on a background worker thread.
pub struct RemapSession {
    stop: Arc<AtomicBool>,
    receiver: mpsc::Receiver<RemapMessage>,
    thread: Option<JoinHandle<()>>,
}

impl RemapSession {
    /// Start a remap for `profile` on `source_device_path`. The runtime is
    /// constructed on the worker thread; watch [`poll`](Self::poll) for the
    /// initial `Running` or `Failed` message.
    pub fn start(profile: Profile, source_device_path: String) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let (sender, receiver) = mpsc::channel();

        let thread = std::thread::spawn(move || {
            let mut runtime = match RemapRuntime::start(RemapOptions {
                profile,
                source_device_path,
            }) {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = sender.send(RemapMessage::Failed(error.to_string()));
                    return;
                }
            };

            let _ = sender.send(RemapMessage::Running(runtime.virtual_nodes().to_vec()));

            while !worker_stop.load(Ordering::Relaxed) && !runtime.stop_requested() {
                if let Err(error) = runtime.pump_once() {
                    let _ = sender.send(RemapMessage::Failed(error.to_string()));
                    return;
                }
            }

            let _ = sender.send(RemapMessage::Stopped);
        });

        Self {
            stop,
            receiver,
            thread: Some(thread),
        }
    }

    /// Drain any pending lifecycle messages without blocking.
    pub fn poll(&self) -> Vec<RemapMessage> {
        self.receiver.try_iter().collect()
    }

    /// Block until the next lifecycle message, returning `None` if the worker
    /// has exited and the channel is closed.
    pub fn recv(&self) -> Option<RemapMessage> {
        self.receiver.recv().ok()
    }

    /// Signal the worker to stop and join it.
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    /// Returns true once the worker thread has finished.
    pub fn is_finished(&self) -> bool {
        self.thread
            .as_ref()
            .map(|thread| thread.is_finished())
            .unwrap_or(true)
    }
}

impl Drop for RemapSession {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::{RemapMessage, RemapSession};
    use crate::profiles::parse_profile_bytes;
    use std::path::Path;

    #[test]
    fn start_on_missing_device_reports_failure() {
        let profile = parse_profile_bytes(
            b"id: t\noutput:\n  type: xbox360\nmappings: []\n",
            Path::new("t.yaml"),
        )
        .unwrap();
        let session =
            RemapSession::start(profile, "/dev/input/padproxy-does-not-exist".to_string());

        // The worker should report a startup failure (device cannot be opened).
        let message = session.recv().expect("a lifecycle message");
        match message {
            RemapMessage::Failed(error) => assert!(!error.is_empty()),
            other => panic!("expected failure, got {other:?}"),
        }
    }
}
