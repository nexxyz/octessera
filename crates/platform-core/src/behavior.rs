use crate::events::MusicalEvent;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellTriggerType {
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
    pub cells: Vec<bool>,
    #[serde(default)]
    pub trigger_types: Option<Vec<CellTriggerType>>,
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

pub trait BehaviorEngine<State, Config> {
    fn id(&self) -> &'static str;
    fn init(&self, config: Config) -> State;
    fn on_input(&self, state: State, input: DeviceInput, context: &mut BehaviorContext) -> State;
    fn on_tick(&self, state: State, context: &mut BehaviorContext) -> State;
    fn render_model(&self, state: &State) -> BehaviorRenderModel;
    fn serialize(&self, state: &State) -> serde_json::Value;
    fn deserialize(&self, data: serde_json::Value) -> Result<State, String>;
    fn config_menu(&self, _state: &State) -> Option<Vec<BehaviorConfigItem>> {
        None
    }
    fn interpret_input_transitions(&self) -> bool {
        false
    }
    fn grid_interaction(&self) -> Option<GridInteraction> {
        None
    }
}
