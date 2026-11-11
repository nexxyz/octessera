use super::{InputNodeListing, KeyboardKey};
use evdev::{BusType, Device, EventType, KeyCode};
use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::DirEntryExt;
use std::path::PathBuf;

/// `EVIOCGRAB` from linux/input.h. The grab is set here rather than through
/// evdev, whose drop would retry the ungrab on an unplugged keyboard and log
/// ENODEV; the kernel drops the grab when the device node closes anyway.
const EVIOCGRAB: libc::c_ulong = 0x4004_4590;

pub(super) struct EvdevKeyboard {
    device: Device,
}

#[derive(Default)]
pub(super) struct EvdevAcquirer {
    rejected: Option<InputNodeListing>,
}

impl super::worker::KeyboardAcquirer for EvdevAcquirer {
    type Device = EvdevKeyboard;

    fn discover_first(
        &mut self,
        buses: super::KeyboardBuses,
    ) -> super::worker::Acquisition<Self::Device> {
        let entries = fs::read_dir("/dev/input")
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| (entry.path(), entry.ino()))
            .collect();
        let listing = InputNodeListing::new(buses, entries);
        if self.rejected.as_ref() == Some(&listing) {
            return super::worker::Acquisition::None;
        }
        let (acquisition, every_node_rejected) = discover_keyboard(buses, listing.paths());
        self.rejected = every_node_rejected.then_some(listing);
        acquisition
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
}

fn grab(device: &Device) -> io::Result<()> {
    // SAFETY: EVIOCGRAB takes an int by value on an open evdev descriptor.
    let result = unsafe { libc::ioctl(device.as_raw_fd(), EVIOCGRAB as _, 1) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

impl super::worker::KeyboardDevice for EvdevKeyboard {
    fn read_events(&mut self) -> io::Result<Vec<(KeyboardKey, i32)>> {
        self.read_events()
    }
}

/// Also reports whether every node opened and was rejected as no supported keyboard; a node that
/// failed to open, such as one udev has not yet given to the input group, must be probed again.
fn discover_keyboard(
    buses: super::KeyboardBuses,
    event_nodes: Vec<PathBuf>,
) -> (super::worker::Acquisition<EvdevKeyboard>, bool) {
    use super::worker::Acquisition::{GrabFailed, None, Ready};

    let mut every_node_rejected = true;
    for path in event_nodes {
        let Ok(device) = Device::open(&path) else {
            every_node_rejected = false;
            continue;
        };
        if !is_supported_keyboard(&device, buses) {
            continue;
        }
        if device.set_nonblocking(true).is_err() {
            every_node_rejected = false;
            continue;
        }
        if grab(&device).is_err() {
            return (GrabFailed, false);
        }
        return (Ready(EvdevKeyboard { device }), false);
    }
    (None, every_node_rejected)
}

fn is_supported_keyboard(device: &Device, buses: super::KeyboardBuses) -> bool {
    let Some(keys) = device.supported_keys() else {
        return false;
    };
    let bus_allowed = match device.input_id().bus_type() {
        BusType::BUS_USB => buses.usb,
        BusType::BUS_BLUETOOTH => buses.bluetooth,
        _ => false,
    };
    super::worker::accepts_keyboard(bus_allowed, |key| keys.contains(evdev_key(key)))
}

const AUX_KEYS: [[KeyCode; 3]; 3] = [
    [KeyCode::KEY_1, KeyCode::KEY_2, KeyCode::KEY_3],
    [KeyCode::KEY_Q, KeyCode::KEY_W, KeyCode::KEY_E],
    [KeyCode::KEY_A, KeyCode::KEY_S, KeyCode::KEY_D],
];

/// Keyboard rows from the bottom: `C`..`/`, `F`..`'`, `R`..`[`, `4`..`-`.
const GRID_KEYS: [[KeyCode; 8]; 4] = [
    [
        KeyCode::KEY_C,
        KeyCode::KEY_V,
        KeyCode::KEY_B,
        KeyCode::KEY_N,
        KeyCode::KEY_M,
        KeyCode::KEY_COMMA,
        KeyCode::KEY_DOT,
        KeyCode::KEY_SLASH,
    ],
    [
        KeyCode::KEY_F,
        KeyCode::KEY_G,
        KeyCode::KEY_H,
        KeyCode::KEY_J,
        KeyCode::KEY_K,
        KeyCode::KEY_L,
        KeyCode::KEY_SEMICOLON,
        KeyCode::KEY_APOSTROPHE,
    ],
    [
        KeyCode::KEY_R,
        KeyCode::KEY_T,
        KeyCode::KEY_Y,
        KeyCode::KEY_U,
        KeyCode::KEY_I,
        KeyCode::KEY_O,
        KeyCode::KEY_P,
        KeyCode::KEY_LEFTBRACE,
    ],
    [
        KeyCode::KEY_4,
        KeyCode::KEY_5,
        KeyCode::KEY_6,
        KeyCode::KEY_7,
        KeyCode::KEY_8,
        KeyCode::KEY_9,
        KeyCode::KEY_0,
        KeyCode::KEY_MINUS,
    ],
];

const NAMED_KEYS: [(KeyCode, KeyboardKey); 14] = [
    (KeyCode::KEY_LEFT, KeyboardKey::Left),
    (KeyCode::KEY_UP, KeyboardKey::Up),
    (KeyCode::KEY_RIGHT, KeyboardKey::Right),
    (KeyCode::KEY_DOWN, KeyboardKey::Down),
    (KeyCode::KEY_ENTER, KeyboardKey::Enter),
    (KeyCode::KEY_KPENTER, KeyboardKey::KeypadEnter),
    (KeyCode::KEY_BACKSPACE, KeyboardKey::Backspace),
    (KeyCode::KEY_ESC, KeyboardKey::Escape),
    (KeyCode::KEY_SPACE, KeyboardKey::Space),
    (KeyCode::KEY_LEFTSHIFT, KeyboardKey::LeftShift),
    (KeyCode::KEY_RIGHTSHIFT, KeyboardKey::RightShift),
    (KeyCode::KEY_LEFTCTRL, KeyboardKey::LeftControl),
    (KeyCode::KEY_RIGHTCTRL, KeyboardKey::RightControl),
    (KeyCode::KEY_LEFTALT, KeyboardKey::LeftAlt),
];

pub(super) fn evdev_key(key: KeyboardKey) -> KeyCode {
    match key {
        KeyboardKey::Aux { encoder, slot } => AUX_KEYS[usize::from(encoder - 1)][usize::from(slot)],
        KeyboardKey::Grid { column, row } => GRID_KEYS[usize::from(row)][usize::from(column)],
        _ => {
            NAMED_KEYS
                .iter()
                .find(|(_, named)| *named == key)
                .expect("every named keyboard key has an evdev code")
                .0
        }
    }
}

pub(super) fn keyboard_key(code: u16) -> Option<KeyboardKey> {
    let code = KeyCode::new(code);
    for (encoder, keys) in (1..).zip(AUX_KEYS) {
        if let Some(slot) = keys.iter().position(|key| *key == code) {
            let slot = slot as u8;
            return Some(KeyboardKey::Aux { encoder, slot });
        }
    }
    for (row, keys) in (0..).zip(GRID_KEYS) {
        if let Some(column) = keys.iter().position(|key| *key == code) {
            let column = column as u8;
            return Some(KeyboardKey::Grid { column, row });
        }
    }
    NAMED_KEYS
        .iter()
        .find(|(named, _)| *named == code)
        .map(|(_, key)| *key)
}
