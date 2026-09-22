use super::{sorted_event_nodes, KeyboardKey};
use evdev::{BusType, Device, EventType, KeyCode};
use std::fs;
use std::io;
use std::path::PathBuf;

pub(super) struct EvdevKeyboard {
    device: Device,
}

pub(super) struct EvdevAcquirer;

impl super::worker::KeyboardAcquirer for EvdevAcquirer {
    type Device = EvdevKeyboard;

    fn discover_first(&mut self) -> super::worker::Acquisition<Self::Device> {
        discover_keyboard()
    }
}

impl EvdevKeyboard {
    fn read_events(&mut self) -> io::Result<Vec<(KeyboardKey, i32)>> {
        self.device.fetch_events().map(|events| {
            events
                .filter_map(
                    |event| match (event.event_type(), keyboard_key(event.code())) {
                        (EventType::KEY, Some(key)) => Some((key, event.value())),
                        _ => None,
                    },
                )
                .collect()
        })
    }

    fn ungrab(&mut self) {
        let _ = self.device.ungrab();
    }
}

impl super::worker::KeyboardDevice for EvdevKeyboard {
    fn read_events(&mut self) -> io::Result<Vec<(KeyboardKey, i32)>> {
        self.read_events()
    }

    fn ungrab(&mut self) {
        self.ungrab();
    }
}

pub(super) fn discover_keyboard() -> super::worker::Acquisition<EvdevKeyboard> {
    use super::worker::Acquisition::{GrabFailed, None, Ready};

    let paths = fs::read_dir("/dev/input")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<PathBuf>>();
    for path in sorted_event_nodes(paths) {
        let Ok(mut device) = Device::open(&path) else {
            continue;
        };
        if !is_supported_keyboard(&device) {
            continue;
        }
        if device.set_nonblocking(true).is_err() {
            continue;
        }
        if device.grab().is_err() {
            return GrabFailed;
        }
        return Ready(EvdevKeyboard { device });
    }
    None
}

fn is_supported_keyboard(device: &Device) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    super::worker::accepts_keyboard(device.input_id().bus_type() == BusType::BUS_USB, |key| {
        keys.contains(evdev_key(key))
    })
}

pub(super) fn evdev_key(key: KeyboardKey) -> KeyCode {
    match key {
        KeyboardKey::Left => KeyCode::KEY_LEFT,
        KeyboardKey::Up => KeyCode::KEY_UP,
        KeyboardKey::Right => KeyCode::KEY_RIGHT,
        KeyboardKey::Down => KeyCode::KEY_DOWN,
        KeyboardKey::Q => KeyCode::KEY_Q,
        KeyboardKey::W => KeyCode::KEY_W,
        KeyboardKey::E => KeyCode::KEY_E,
        KeyboardKey::A => KeyCode::KEY_A,
        KeyboardKey::S => KeyCode::KEY_S,
        KeyboardKey::D => KeyCode::KEY_D,
        KeyboardKey::Z => KeyCode::KEY_Z,
        KeyboardKey::X => KeyCode::KEY_X,
        KeyboardKey::C => KeyCode::KEY_C,
        KeyboardKey::Enter => KeyCode::KEY_ENTER,
        KeyboardKey::Backspace => KeyCode::KEY_BACKSPACE,
        KeyboardKey::Escape => KeyCode::KEY_ESC,
        KeyboardKey::Space => KeyCode::KEY_SPACE,
        KeyboardKey::LeftShift => KeyCode::KEY_LEFTSHIFT,
        KeyboardKey::RightShift => KeyCode::KEY_RIGHTSHIFT,
        KeyboardKey::LeftControl => KeyCode::KEY_LEFTCTRL,
        KeyboardKey::RightControl => KeyCode::KEY_RIGHTCTRL,
    }
}

pub(super) fn keyboard_key(code: u16) -> Option<KeyboardKey> {
    Some(match KeyCode::new(code) {
        KeyCode::KEY_LEFT => KeyboardKey::Left,
        KeyCode::KEY_UP => KeyboardKey::Up,
        KeyCode::KEY_RIGHT => KeyboardKey::Right,
        KeyCode::KEY_DOWN => KeyboardKey::Down,
        KeyCode::KEY_Q => KeyboardKey::Q,
        KeyCode::KEY_W => KeyboardKey::W,
        KeyCode::KEY_E => KeyboardKey::E,
        KeyCode::KEY_A => KeyboardKey::A,
        KeyCode::KEY_S => KeyboardKey::S,
        KeyCode::KEY_D => KeyboardKey::D,
        KeyCode::KEY_Z => KeyboardKey::Z,
        KeyCode::KEY_X => KeyboardKey::X,
        KeyCode::KEY_C => KeyboardKey::C,
        KeyCode::KEY_ENTER => KeyboardKey::Enter,
        KeyCode::KEY_BACKSPACE => KeyboardKey::Backspace,
        KeyCode::KEY_ESC => KeyboardKey::Escape,
        KeyCode::KEY_SPACE => KeyboardKey::Space,
        KeyCode::KEY_LEFTSHIFT => KeyboardKey::LeftShift,
        KeyCode::KEY_RIGHTSHIFT => KeyboardKey::RightShift,
        KeyCode::KEY_LEFTCTRL => KeyboardKey::LeftControl,
        KeyCode::KEY_RIGHTCTRL => KeyboardKey::RightControl,
        _ => return None,
    })
}
