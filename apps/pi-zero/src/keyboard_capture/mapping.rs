/// Keys by physical position. `Aux { encoder, slot }` slots are turn `-1`, click, turn `+1`;
/// `Grid { column, row }` rows count keyboard rows from the bottom (row 0 = `C`..`/`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyboardKey {
    Left,
    Up,
    Right,
    Down,
    Enter,
    KeypadEnter,
    Backspace,
    Escape,
    Space,
    LeftShift,
    RightShift,
    LeftControl,
    RightControl,
    LeftAlt,
    Aux { encoder: u8, slot: u8 },
    Grid { column: u8, row: u8 },
}

pub(crate) const AUX_ENCODERS: u8 = 3;
pub(crate) const GRID_KEY_COLUMNS: u8 = 8;
pub(crate) const GRID_KEY_ROWS: u8 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyboardInput {
    EncoderTurn { id: &'static str, delta: i8 },
    EncoderPress { id: &'static str },
    ButtonA(bool),
    ButtonS(bool),
    ButtonShift(bool),
    ButtonFn(bool),
    Grid { x: u8, y: u8, pressed: bool },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Held {
    keys: u16,
    grid_plain: u32,
    grid_alt: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedKeyInput {
    pub(crate) input: Option<KeyboardInput>,
    held: Held,
}

#[derive(Default)]
pub(crate) struct KeyboardState {
    held: Held,
}

impl KeyboardState {
    pub(crate) fn prepare(&self, key: KeyboardKey, value: i32) -> Option<PreparedKeyInput> {
        let (input, held) = match key {
            KeyboardKey::Grid { column, row } => self.prepare_grid(column, row, value),
            _ => self.prepare_control(key, value),
        };
        (input.is_some() || held != self.held).then_some(PreparedKeyInput { input, held })
    }

    fn prepare_control(&self, key: KeyboardKey, value: i32) -> (Option<KeyboardInput>, Held) {
        let bit = key_bit(key);
        let keys = match (bit, value) {
            (0, _) => self.held.keys,
            (_, 1) => self.held.keys | bit,
            (_, 0) => self.held.keys & !bit,
            _ => self.held.keys,
        };
        let input = match map_key_event(key, value) {
            Some(KeyboardInput::EncoderPress { id }) => {
                (self.held.keys & bit == 0).then_some(KeyboardInput::EncoderPress { id })
            }
            Some(action @ KeyboardInput::EncoderTurn { .. }) => Some(action),
            Some(action @ KeyboardInput::ButtonA(pressed))
            | Some(action @ KeyboardInput::ButtonS(pressed))
            | Some(action @ KeyboardInput::ButtonShift(pressed))
            | Some(action @ KeyboardInput::ButtonFn(pressed)) => {
                let kind = button_kind(key).expect("button action has a button key");
                let was_held = button_held(self.held.keys, kind);
                let remains_held = button_held(keys, kind);
                (pressed && !was_held || !pressed && was_held && !remains_held).then_some(action)
            }
            Some(KeyboardInput::Grid { .. }) | None => None,
        };
        (input, Held { keys, ..self.held })
    }

    fn prepare_grid(&self, column: u8, row: u8, value: i32) -> (Option<KeyboardInput>, Held) {
        let bit = 1_u32 << (row * GRID_KEY_COLUMNS + column);
        let mut held = self.held;
        let input = match value {
            1 if (held.grid_plain | held.grid_alt) & bit == 0 => {
                let alt = held.keys & key_bit(KeyboardKey::LeftAlt) != 0;
                if alt {
                    held.grid_alt |= bit;
                } else {
                    held.grid_plain |= bit;
                }
                Some(grid_input(column, row, alt, true))
            }
            0 if held.grid_plain & bit != 0 => {
                held.grid_plain &= !bit;
                Some(grid_input(column, row, false, false))
            }
            0 if held.grid_alt & bit != 0 => {
                held.grid_alt &= !bit;
                Some(grid_input(column, row, true, false))
            }
            _ => None,
        };
        (input, held)
    }

    pub(crate) fn commit(&mut self, prepared: PreparedKeyInput) {
        self.held = prepared.held;
    }

    pub(crate) fn held_releases(&self) -> Vec<KeyboardInput> {
        let mut releases = Vec::new();
        for (mask, alt) in [(self.held.grid_plain, false), (self.held.grid_alt, true)] {
            for index in 0..GRID_KEY_COLUMNS * GRID_KEY_ROWS {
                if mask & (1 << index) != 0 {
                    let (column, row) = (index % GRID_KEY_COLUMNS, index / GRID_KEY_COLUMNS);
                    releases.push(grid_input(column, row, alt, false));
                }
            }
        }
        let keys = self.held.keys;
        releases.extend(
            [
                button_held(keys, ButtonKind::A).then_some(KeyboardInput::ButtonA(false)),
                button_held(keys, ButtonKind::S).then_some(KeyboardInput::ButtonS(false)),
                button_held(keys, ButtonKind::Shift).then_some(KeyboardInput::ButtonShift(false)),
                button_held(keys, ButtonKind::Fn).then_some(KeyboardInput::ButtonFn(false)),
            ]
            .into_iter()
            .flatten(),
        );
        releases
    }

    pub(crate) fn clear(&mut self) {
        self.held = Held::default();
    }
}

fn grid_input(column: u8, row: u8, alt: bool, pressed: bool) -> KeyboardInput {
    KeyboardInput::Grid {
        x: column,
        y: if alt { row + GRID_KEY_ROWS } else { row },
        pressed,
    }
}

/// Stateless mapping of every key except the grid, whose tile depends on the held Alt key.
pub(crate) fn map_key_event(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    encoder_turn(key, value)
        .or_else(|| encoder_click(key, value))
        .or_else(|| button_edge(key, value))
}

fn aux_id(encoder: u8) -> &'static str {
    match encoder {
        1 => "encoder_aux_1",
        2 => "encoder_aux_2",
        _ => "encoder_aux_3",
    }
}

fn encoder_turn(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    if !matches!(value, 1 | 2) {
        return None;
    }
    let (id, delta) = match key {
        KeyboardKey::Left | KeyboardKey::Up => ("encoder_main", -1),
        KeyboardKey::Right | KeyboardKey::Down => ("encoder_main", 1),
        KeyboardKey::Aux { encoder, slot: 0 } => (aux_id(encoder), -1),
        KeyboardKey::Aux { encoder, slot: 2 } => (aux_id(encoder), 1),
        _ => return None,
    };
    Some(KeyboardInput::EncoderTurn { id, delta })
}

fn encoder_click(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    if value != 1 {
        return None;
    }
    let id = match key {
        KeyboardKey::Enter | KeyboardKey::KeypadEnter => "encoder_main",
        KeyboardKey::Aux { encoder, slot: 1 } => aux_id(encoder),
        _ => return None,
    };
    Some(KeyboardInput::EncoderPress { id })
}

fn button_edge(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    let pressed = match value {
        1 => true,
        0 => false,
        _ => return None,
    };
    match key {
        KeyboardKey::Backspace | KeyboardKey::Escape => Some(KeyboardInput::ButtonA(pressed)),
        KeyboardKey::Space => Some(KeyboardInput::ButtonS(pressed)),
        KeyboardKey::LeftShift | KeyboardKey::RightShift => {
            Some(KeyboardInput::ButtonShift(pressed))
        }
        KeyboardKey::LeftControl | KeyboardKey::RightControl => {
            Some(KeyboardInput::ButtonFn(pressed))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ButtonKind {
    A,
    S,
    Shift,
    Fn,
}

fn button_kind(key: KeyboardKey) -> Option<ButtonKind> {
    match key {
        KeyboardKey::Backspace | KeyboardKey::Escape => Some(ButtonKind::A),
        KeyboardKey::Space => Some(ButtonKind::S),
        KeyboardKey::LeftShift | KeyboardKey::RightShift => Some(ButtonKind::Shift),
        KeyboardKey::LeftControl | KeyboardKey::RightControl => Some(ButtonKind::Fn),
        _ => None,
    }
}

fn button_held(held_keys: u16, kind: ButtonKind) -> bool {
    [
        KeyboardKey::Backspace,
        KeyboardKey::Escape,
        KeyboardKey::Space,
        KeyboardKey::LeftShift,
        KeyboardKey::RightShift,
        KeyboardKey::LeftControl,
        KeyboardKey::RightControl,
    ]
    .into_iter()
    .filter(|key| button_kind(*key) == Some(kind))
    .any(|key| held_keys & key_bit(key) != 0)
}

fn key_bit(key: KeyboardKey) -> u16 {
    match key {
        KeyboardKey::Backspace => 1 << 0,
        KeyboardKey::Escape => 1 << 1,
        KeyboardKey::Space => 1 << 2,
        KeyboardKey::LeftShift => 1 << 3,
        KeyboardKey::RightShift => 1 << 4,
        KeyboardKey::LeftControl => 1 << 5,
        KeyboardKey::RightControl => 1 << 6,
        KeyboardKey::Aux { encoder, slot: 1 } => 1 << (6 + encoder),
        KeyboardKey::LeftAlt => 1 << 10,
        _ => 0,
    }
}
