#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyboardKey {
    Left,
    Up,
    Right,
    Down,
    Q,
    W,
    E,
    A,
    S,
    D,
    Z,
    X,
    C,
    Enter,
    Backspace,
    Escape,
    Space,
    LeftShift,
    RightShift,
    LeftControl,
    RightControl,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum KeyboardInput {
    EncoderTurn { id: &'static str, delta: i8 },
    EncoderPress { id: &'static str },
    ButtonA(bool),
    ButtonS(bool),
    ButtonShift(bool),
    ButtonFn(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct PreparedKeyInput {
    pub(crate) input: Option<KeyboardInput>,
    held_keys: u16,
}

#[derive(Default)]
pub(crate) struct KeyboardState {
    held_keys: u16,
}

impl KeyboardState {
    pub(crate) fn prepare(&self, key: KeyboardKey, value: i32) -> Option<PreparedKeyInput> {
        let action = map_key_event(key, value);
        let bit = key_bit(key);
        let held_keys = match (bit, value) {
            (0, _) => self.held_keys,
            (_, 1) => self.held_keys | bit,
            (_, 0) => self.held_keys & !bit,
            _ => self.held_keys,
        };
        let input = match action {
            Some(KeyboardInput::EncoderPress { id }) => {
                (self.held_keys & bit == 0).then_some(KeyboardInput::EncoderPress { id })
            }
            Some(action @ KeyboardInput::EncoderTurn { .. }) => Some(action),
            Some(action @ KeyboardInput::ButtonA(pressed))
            | Some(action @ KeyboardInput::ButtonS(pressed))
            | Some(action @ KeyboardInput::ButtonShift(pressed))
            | Some(action @ KeyboardInput::ButtonFn(pressed)) => {
                let kind = button_kind(key).expect("button action has a button key");
                let was_held = button_held(self.held_keys, kind);
                let remains_held = button_held(held_keys, kind);
                (pressed && !was_held || !pressed && was_held && !remains_held).then_some(action)
            }
            None => None,
        };
        (input.is_some() || held_keys != self.held_keys)
            .then_some(PreparedKeyInput { input, held_keys })
    }

    pub(crate) fn commit(&mut self, prepared: PreparedKeyInput) {
        self.held_keys = prepared.held_keys;
    }

    pub(crate) fn held_releases(&self) -> [Option<KeyboardInput>; 4] {
        [
            button_held(self.held_keys, ButtonKind::A).then_some(KeyboardInput::ButtonA(false)),
            button_held(self.held_keys, ButtonKind::S).then_some(KeyboardInput::ButtonS(false)),
            button_held(self.held_keys, ButtonKind::Shift)
                .then_some(KeyboardInput::ButtonShift(false)),
            button_held(self.held_keys, ButtonKind::Fn).then_some(KeyboardInput::ButtonFn(false)),
        ]
    }

    pub(crate) fn clear(&mut self) {
        self.held_keys = 0;
    }
}

pub(crate) fn map_key_event(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    encoder_turn(key, value)
        .or_else(|| encoder_click(key, value))
        .or_else(|| button_edge(key, value))
}

fn encoder_turn(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    if !matches!(value, 1 | 2) {
        return None;
    }
    let (id, delta) = match key {
        KeyboardKey::Left | KeyboardKey::Up => ("encoder_main", -1),
        KeyboardKey::Right | KeyboardKey::Down => ("encoder_main", 1),
        KeyboardKey::Q => ("encoder_aux_1", -1),
        KeyboardKey::E => ("encoder_aux_1", 1),
        KeyboardKey::A => ("encoder_aux_2", -1),
        KeyboardKey::D => ("encoder_aux_2", 1),
        KeyboardKey::Z => ("encoder_aux_3", -1),
        KeyboardKey::C => ("encoder_aux_3", 1),
        _ => return None,
    };
    Some(KeyboardInput::EncoderTurn { id, delta })
}

fn encoder_click(key: KeyboardKey, value: i32) -> Option<KeyboardInput> {
    if value != 1 {
        return None;
    }
    let id = match key {
        KeyboardKey::Enter => "encoder_main",
        KeyboardKey::W => "encoder_aux_1",
        KeyboardKey::S => "encoder_aux_2",
        KeyboardKey::X => "encoder_aux_3",
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
        KeyboardKey::W => 1 << 7,
        KeyboardKey::S => 1 << 8,
        KeyboardKey::X => 1 << 9,
        _ => 0,
    }
}
