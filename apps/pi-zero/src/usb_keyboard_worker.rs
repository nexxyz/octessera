use super::{emit, release_held_inputs, KeyboardCaptureControl, KeyboardKey, KeyboardState};
use playback_runtime::HostMessage;
use std::io;
use std::sync::mpsc::Sender;

pub(super) enum Acquisition<Device> {
    None,
    GrabFailed,
    Ready(Device),
}

pub(super) trait KeyboardAcquirer {
    type Device: KeyboardDevice;

    fn discover_first(&mut self) -> Acquisition<Self::Device>;
}

pub(super) trait KeyboardDevice {
    fn read_events(&mut self) -> io::Result<Vec<(KeyboardKey, i32)>>;
    fn ungrab(&mut self);
}

pub(super) fn accepts_keyboard(
    usb_bus: bool,
    mut supports_key: impl FnMut(KeyboardKey) -> bool,
) -> bool {
    usb_bus && REQUIRED_KEYS.iter().copied().all(&mut supports_key)
}

const REQUIRED_KEYS: [KeyboardKey; 21] = [
    KeyboardKey::Left,
    KeyboardKey::Up,
    KeyboardKey::Right,
    KeyboardKey::Down,
    KeyboardKey::Q,
    KeyboardKey::W,
    KeyboardKey::E,
    KeyboardKey::A,
    KeyboardKey::S,
    KeyboardKey::D,
    KeyboardKey::Z,
    KeyboardKey::X,
    KeyboardKey::C,
    KeyboardKey::Enter,
    KeyboardKey::Backspace,
    KeyboardKey::Escape,
    KeyboardKey::Space,
    KeyboardKey::LeftShift,
    KeyboardKey::RightShift,
    KeyboardKey::LeftControl,
    KeyboardKey::RightControl,
];

#[cfg(target_os = "linux")]
pub(super) fn run(control: KeyboardCaptureControl, input_tx: Sender<HostMessage>) {
    run_with_acquirer(control, input_tx, super::evdev::EvdevAcquirer);
}

pub(super) fn run_with_acquirer<A>(
    control: KeyboardCaptureControl,
    input_tx: Sender<HostMessage>,
    mut acquirer: A,
) where
    A: KeyboardAcquirer,
{
    let mut device = None;
    let mut state = KeyboardState::default();
    loop {
        if control.is_shutdown() {
            finish_capture(&mut device, &mut state, &input_tx);
            return;
        }
        if !control.is_enabled() {
            if !finish_capture(&mut device, &mut state, &input_tx) {
                return;
            }
            control.wait_for_change(false, None);
            continue;
        }
        if device.is_none() {
            match acquirer.discover_first() {
                Acquisition::Ready(next) => device = Some(next),
                Acquisition::None | Acquisition::GrabFailed => {
                    control.wait_for_change(true, Some(super::DISCOVERY_WAIT));
                    continue;
                }
            }
        }
        let Some(device_ref) = device.as_mut() else {
            continue;
        };
        let mut capture_interrupted = false;
        match device_ref.read_events() {
            Ok(events) => {
                for (key, value) in events {
                    match handle_event(&control, &input_tx, &mut state, key, value) {
                        EventResult::Continue => {}
                        EventResult::Disabled => {
                            capture_interrupted = true;
                            break;
                        }
                        EventResult::ReceiverDisconnected => {
                            finish_capture(&mut device, &mut state, &input_tx);
                            return;
                        }
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(_) => {
                if !finish_capture(&mut device, &mut state, &input_tx) {
                    return;
                }
            }
        }
        if capture_interrupted || !control.is_enabled() {
            if !finish_capture(&mut device, &mut state, &input_tx) {
                return;
            }
            continue;
        }
        control.wait_for_change(
            true,
            Some(if device.is_some() {
                super::ACTIVE_READ_WAIT
            } else {
                super::DISCOVERY_WAIT
            }),
        );
    }
}

enum EventResult {
    Continue,
    Disabled,
    ReceiverDisconnected,
}

fn handle_event(
    control: &KeyboardCaptureControl,
    input_tx: &Sender<HostMessage>,
    state: &mut KeyboardState,
    key: KeyboardKey,
    value: i32,
) -> EventResult {
    let Some(prepared) = state.prepare(key, value) else {
        return EventResult::Continue;
    };
    if let Some(input) = prepared.input {
        if !control.is_enabled() {
            return EventResult::Disabled;
        }
        if !emit(input_tx, input) {
            return EventResult::ReceiverDisconnected;
        }
    }
    state.commit(prepared);
    EventResult::Continue
}

fn finish_capture(
    device: &mut Option<impl KeyboardDevice>,
    state: &mut KeyboardState,
    input_tx: &Sender<HostMessage>,
) -> bool {
    if !release_held_inputs(state, input_tx) {
        drop_device(device);
        return false;
    }
    drop_device(device);
    true
}

fn drop_device(device: &mut Option<impl KeyboardDevice>) {
    if let Some(mut device) = device.take() {
        device.ungrab();
    }
}
