use crate::events::MusicalEvent;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TileTriggerType {
    Activate,
    Stable,
    Deactivate,
    Scanned,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GridInteraction {
    Paint,
    Momentary,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeviceInput {
    EncoderTurn {
        delta: i8,
        #[serde(default)]
        id: Option<String>,
    },
    EncoderPress {
        #[serde(default)]
        id: Option<String>,
    },
    ButtonA {
        #[serde(default)]
        pressed: Option<bool>,
    },
    ButtonS {
        #[serde(default)]
        pressed: Option<bool>,
    },
    ButtonShift {
        #[serde(default)]
        pressed: Option<bool>,
    },
    ButtonFn {
        #[serde(default)]
        pressed: Option<bool>,
    },
    ButtonCombinedModifier {
        #[serde(default)]
        pressed: Option<bool>,
    },
    GridPress {
        x: usize,
        y: usize,
    },
    GridRelease {
        x: usize,
        y: usize,
    },
    BehaviorAction(BehaviorActionInput),
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorActionInput {
    #[serde(rename = "actionType")]
    pub action_type: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BehaviorContext {
    pub bpm: f32,
    #[serde(skip)]
    pub emitted_events: Vec<MusicalEvent>,
}

impl BehaviorContext {
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm,
            emitted_events: Vec::new(),
        }
    }

    pub fn emit(&mut self, event: MusicalEvent) {
        self.emitted_events.push(event);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorRenderModel {
    pub name: String,
    pub status_line: String,
    pub tiles: Vec<bool>,
    #[serde(default)]
    pub palette: BehaviorRenderPalette,
    #[serde(default)]
    pub trigger_types: Option<Vec<TileTriggerType>>,
    /// Per-tile colors for lit tiles, in place of the palette's trigger colors, for behaviors
    /// whose things each keep their own color (pong's ball, blockout's block rows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile_colors: Option<Vec<[u8; 3]>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorRenderPalette {
    pub active: [u8; 3],
    pub inactive: [u8; 3],
    pub stable: [u8; 3],
}

impl Default for BehaviorRenderPalette {
    fn default() -> Self {
        Self {
            active: crate::palette::DEFAULT_BEHAVIOR_ACTIVE,
            inactive: crate::palette::DEFAULT_BEHAVIOR_INACTIVE,
            stable: crate::palette::DEFAULT_BEHAVIOR_STABLE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BehaviorConfigItem {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub item_type: BehaviorConfigItemType,
    #[serde(default)]
    pub min: Option<i32>,
    #[serde(default)]
    pub max: Option<i32>,
    #[serde(default)]
    pub step: Option<i32>,
    #[serde(default)]
    pub options: Option<Vec<String>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BehaviorConfigItemType {
    Number,
    Bool,
    Enum,
    Action,
}
