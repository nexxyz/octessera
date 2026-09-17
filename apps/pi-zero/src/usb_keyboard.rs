#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(target_os = "linux")]
#[path = "usb_keyboard_evdev.rs"]
mod evdev;
#[path = "usb_keyboard_mapping.rs"]
mod mapping;
#[cfg(any(target_os = "linux", test))]
#[path = "usb_keyboard_worker.rs"]
mod worker;

#[cfg(test)]
pub(crate) use mapping::map_key_event;
#[cfg(any(target_os = "linux", test))]
pub(crate) use mapping::KeyboardKey;
pub(crate) use mapping::{KeyboardInput, KeyboardState};

use playback_runtime::HostMessage;
use serde_json::Value;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub(crate) const ACTIVE_READ_WAIT: Duration = Duration::from_millis(8);
pub(crate) const DISCOVERY_WAIT: Duration = Duration::from_millis(250);

const ALLOWED_HDMI_MODES: [&str; 4] = [
    "live-grid",
    "plain-grid",
    "active-behavior",
    "cycle-behaviors",
];

#[derive(Clone)]
pub(crate) struct KeyboardCaptureControl {
    shared: Arc<KeyboardCaptureShared>,
}

struct KeyboardCaptureShared {
    board_host_enabled: bool,
    state: Mutex<KeyboardCaptureState>,
    wake: Condvar,
}

#[derive(Default)]
struct KeyboardCaptureState {
    enabled: bool,
    shutdown: bool,
}

impl KeyboardCaptureControl {
    pub(crate) fn new(board_host_enabled: bool) -> Self {
        Self {
            shared: Arc::new(KeyboardCaptureShared {
                board_host_enabled,
                state: Mutex::new(KeyboardCaptureState::default()),
                wake: Condvar::new(),
            }),
        }
    }

    pub(crate) fn observe_snapshot(&self, snapshot: &Value) {
        let enabled = capture_enabled_for_snapshot(self.shared.board_host_enabled, snapshot);
        let mut state = self.shared.state.lock().unwrap();
        if state.enabled != enabled {
            state.enabled = enabled;
            self.shared.wake.notify_one();
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.shared.state.lock().unwrap().enabled
    }

    fn is_shutdown(&self) -> bool {
        self.shared.state.lock().unwrap().shutdown
    }

    fn stop(&self) {
        let mut state = self.shared.state.lock().unwrap();
        state.shutdown = true;
        state.enabled = false;
        self.shared.wake.notify_one();
    }

    fn wait_for_change(&self, enabled: bool, timeout: Option<Duration>) {
        let mut state = self.shared.state.lock().unwrap();
        if state.shutdown || state.enabled != enabled {
            return;
        }
        if let Some(timeout) = timeout {
            let _ = self
                .shared
                .wake
                .wait_timeout_while(state, timeout, |state| {
                    !state.shutdown && state.enabled == enabled
                })
                .unwrap();
        } else {
            while !state.shutdown && state.enabled == enabled {
                state = self.shared.wake.wait(state).unwrap();
            }
        }
    }
}

pub(crate) struct KeyboardCapture {
    control: KeyboardCaptureControl,
    worker: Option<JoinHandle<()>>,
}

impl KeyboardCapture {
    pub(crate) fn spawn(input_tx: Sender<HostMessage>, board_host_enabled: bool) -> Self {
        let control = KeyboardCaptureControl::new(board_host_enabled);
        let worker_control = control.clone();
        let worker = thread::Builder::new()
            .name("octessera-usb-keyboard".into())
            .spawn(move || run_worker(worker_control, input_tx))
            .expect("USB keyboard worker should start");
        Self {
            control,
            worker: Some(worker),
        }
    }

    pub(crate) fn control(&self) -> KeyboardCaptureControl {
        self.control.clone()
    }

    pub(crate) fn shutdown(mut self) -> Result<(), String> {
        self.control.stop();
        let worker = self
            .worker
            .take()
            .ok_or_else(|| "USB keyboard worker was already stopped".to_string())?;
        worker
            .join()
            .map_err(|_| "USB keyboard worker panicked during shutdown".to_string())
    }
}

impl Drop for KeyboardCapture {
    fn drop(&mut self) {
        self.control.stop();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub(crate) fn capture_enabled_for_snapshot(board_host_enabled: bool, snapshot: &Value) -> bool {
    board_host_enabled
        && snapshot
            .get("hdmi")
            .and_then(Value::as_object)
            .and_then(|hdmi| hdmi.get("mode"))
            .and_then(Value::as_str)
            .is_some_and(|mode| ALLOWED_HDMI_MODES.contains(&mode))
}

pub(crate) fn sorted_event_nodes(paths: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut nodes = paths
        .into_iter()
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            let number = name.strip_prefix("event")?.parse::<u32>().ok()?;
            Some((number, path))
        })
        .collect::<Vec<_>>();
    nodes.sort_by_key(|(number, _)| *number);
    nodes.into_iter().map(|(_, path)| path).collect()
}

fn host_message(input: KeyboardInput) -> HostMessage {
    match input {
        KeyboardInput::EncoderTurn { id, delta } => {
            Some(crate::input::encoder_turn_message(id, delta))
        }
        KeyboardInput::EncoderPress { id } => Some(crate::input::encoder_press_message(id)),
        KeyboardInput::ButtonA(pressed) => crate::input::neokey_message(0, pressed),
        KeyboardInput::ButtonS(pressed) => crate::input::neokey_message(1, pressed),
        KeyboardInput::ButtonShift(pressed) => crate::input::neokey_message(2, pressed),
        KeyboardInput::ButtonFn(pressed) => crate::input::neokey_message(3, pressed),
    }
    .expect("keyboard input uses a valid native key")
}

fn emit(input_tx: &Sender<HostMessage>, input: KeyboardInput) -> bool {
    input_tx.send(host_message(input)).is_ok()
}

fn release_held_inputs(state: &mut KeyboardState, input_tx: &Sender<HostMessage>) -> bool {
    for input in state.held_releases().into_iter().flatten() {
        if !emit(input_tx, input) {
            return false;
        }
    }
    state.clear();
    true
}

#[cfg(target_os = "linux")]
fn run_worker(control: KeyboardCaptureControl, input_tx: Sender<HostMessage>) {
    worker::run(control, input_tx);
}

#[cfg(not(target_os = "linux"))]
fn run_worker(control: KeyboardCaptureControl, _input_tx: Sender<HostMessage>) {
    while !control.is_shutdown() {
        control.wait_for_change(control.is_enabled(), None);
    }
}

#[cfg(test)]
#[path = "usb_keyboard_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "usb_keyboard_worker_tests.rs"]
mod worker_tests;
