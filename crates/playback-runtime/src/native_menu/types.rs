use std::collections::HashMap;

pub use super::types_config::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuAction {
    BehaviorAction(String),
    PlatformEffect(String),
    SetParamBinding {
        target: String,
        binding: NativeParamBindingSpec,
    },
    ClearParamBinding {
        target: String,
    },
    SetAuxClick {
        index: usize,
        action: Option<Box<NativeMenuAction>>,
    },
    CloneInstrument {
        index: usize,
    },
    ResetInstrument {
        index: usize,
    },
    ResetBehavior,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeParamBindingSpec {
    pub key: String,
    pub label: Option<String>,
    pub kind: String,
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub step: Option<i32>,
    pub options: Vec<String>,
    pub invert: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NativeMenuValue {
    Group,
    Enum {
        options: Vec<String>,
        selected: usize,
    },
    Number {
        value: i32,
        min: i32,
        max: i32,
        step: i32,
    },
    Bool {
        value: bool,
    },
    Text {
        value: String,
        max_len: usize,
        cursor: usize,
    },
    Action(NativeMenuAction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuItem {
    pub label: String,
    pub key: Option<String>,
    pub value: NativeMenuValue,
    pub children: Vec<NativeMenuItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct NativeMenuState {
    pub stack: Vec<usize>,
    pub cursor: usize,
    pub editing: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuSnapshot {
    pub path: String,
    pub lines: Vec<String>,
    pub colors: Vec<u16>,
    pub bar_values: Vec<Option<NativeMenuBarValue>>,
    pub scroll: Option<NativeMenuScrollMetadata>,
    pub line_keys: Vec<Option<String>>,
    pub line_actions: Vec<Option<NativeMenuAction>>,
    pub selected_row: Option<usize>,
    pub selected_action: Option<NativeMenuAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuScrollMetadata {
    pub scroll_offset: usize,
    pub total_rows: usize,
    pub visible_rows: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuBarValue {
    pub frac_pct: u8,
    pub num_chars: usize,
    pub style: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuHelpTarget {
    pub path: String,
    pub key: String,
    pub kind: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeMenuModel {
    pub root: NativeMenuItem,
    pub state: NativeMenuState,
    pub numeric_display_mode: String,
    pub navigation_memory: HashMap<String, usize>,
}
