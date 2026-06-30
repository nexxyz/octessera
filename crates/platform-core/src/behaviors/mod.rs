mod behavior_config;
mod behavior_ported_lifecycle;
mod glider;
mod life;
mod none;
mod ported;
mod sequencer;

use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use serde_json::Value;

#[cfg(test)]
mod tests;

pub use glider::GliderState;
pub use life::LifeState;
pub use none::NoneState;
pub use ported::{
    AntState, BounceState, BrainState, DlaState, KeysState, LooperState, RaindropsState,
    ShapesState,
};
pub use sequencer::SequencerState;

#[derive(Clone, Debug, PartialEq)]
pub enum NativeBehaviorState {
    None(NoneState),
    Life(LifeState),
    Glider(GliderState),
    Sequencer(SequencerState),
    Keys(KeysState),
    Looper(LooperState),
    Brain(BrainState),
    Ant(AntState),
    Bounce(BounceState),
    Shapes(ShapesState),
    Raindrops(RaindropsState),
    Dla(DlaState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBehavior {
    None,
    Life,
    Glider,
    Sequencer,
    Keys,
    Looper,
    Brain,
    Ant,
    Bounce,
    Shapes,
    Raindrops,
    Dla,
}

pub fn get_native_behavior(id: &str) -> Option<NativeBehavior> {
    match id {
        "none" => Some(NativeBehavior::None),
        "life" => Some(NativeBehavior::Life),
        "glider" => Some(NativeBehavior::Glider),
        "sequencer" => Some(NativeBehavior::Sequencer),
        "keys" => Some(NativeBehavior::Keys),
        "looper" => Some(NativeBehavior::Looper),
        "brain" => Some(NativeBehavior::Brain),
        "ant" => Some(NativeBehavior::Ant),
        "bounce" => Some(NativeBehavior::Bounce),
        "shapes" => Some(NativeBehavior::Shapes),
        "raindrops" => Some(NativeBehavior::Raindrops),
        "dla" => Some(NativeBehavior::Dla),
        _ => None,
    }
}

pub fn list_native_behavior_ids() -> &'static [&'static str] {
    &[
        "none",
        "life",
        "sequencer",
        "keys",
        "looper",
        "brain",
        "ant",
        "bounce",
        "shapes",
        "raindrops",
        "dla",
        "glider",
    ]
}

impl NativeBehavior {
    pub fn id(self) -> &'static str {
        match self {
            NativeBehavior::None => "none",
            NativeBehavior::Life => "life",
            NativeBehavior::Glider => "glider",
            NativeBehavior::Sequencer => "sequencer",
            NativeBehavior::Keys => "keys",
            NativeBehavior::Looper => "looper",
            NativeBehavior::Brain => "brain",
            NativeBehavior::Ant => "ant",
            NativeBehavior::Bounce => "bounce",
            NativeBehavior::Shapes => "shapes",
            NativeBehavior::Raindrops => "raindrops",
            NativeBehavior::Dla => "dla",
        }
    }

    pub fn init(self, config: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::None => Ok(NativeBehaviorState::None(none::init(config)?)),
            NativeBehavior::Life => Ok(NativeBehaviorState::Life(life::init(config)?)),
            NativeBehavior::Glider => Ok(NativeBehaviorState::Glider(glider::init(config)?)),
            NativeBehavior::Sequencer => {
                Ok(NativeBehaviorState::Sequencer(sequencer::init(config)?))
            }
            _ => self.init_ported(config),
        }
    }

    pub fn on_input(
        self,
        state: NativeBehaviorState,
        input: DeviceInput,
        context: &mut BehaviorContext,
    ) -> Result<NativeBehaviorState, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(state)) => Ok(
                NativeBehaviorState::None(none::on_input(state, input, context)),
            ),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => Ok(
                NativeBehaviorState::Life(life::on_input(state, input, context)),
            ),
            (NativeBehavior::Glider, NativeBehaviorState::Glider(state)) => Ok(
                NativeBehaviorState::Glider(glider::on_input(state, input, context)),
            ),
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => Ok(
                NativeBehaviorState::Sequencer(sequencer::on_input(state, input, context)),
            ),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => Ok(
                NativeBehaviorState::Keys(ported::keys_on_input(state, input, context)),
            ),
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => Ok(
                NativeBehaviorState::Looper(ported::looper_on_input(state, input, context)),
            ),
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => Ok(
                NativeBehaviorState::Brain(ported::brain_on_input(state, input, context)),
            ),
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => Ok(NativeBehaviorState::Ant(
                ported::ant_on_input(state, input, context),
            )),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => Ok(
                NativeBehaviorState::Bounce(ported::bounce_on_input(state, input, context)),
            ),
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => Ok(
                NativeBehaviorState::Shapes(ported::shapes_on_input(state, input, context)),
            ),
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => Ok(
                NativeBehaviorState::Raindrops(ported::raindrops_on_input(state, input, context)),
            ),
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => Ok(NativeBehaviorState::Dla(
                ported::dla_on_input(state, input, context),
            )),
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn on_tick(
        self,
        state: NativeBehaviorState,
        context: &mut BehaviorContext,
    ) -> Result<NativeBehaviorState, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(state)) => {
                Ok(NativeBehaviorState::None(none::on_tick(state, context)))
            }
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
                Ok(NativeBehaviorState::Life(life::on_tick(state, context)))
            }
            (NativeBehavior::Glider, NativeBehaviorState::Glider(state)) => {
                Ok(NativeBehaviorState::Glider(glider::on_tick(state, context)))
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => Ok(
                NativeBehaviorState::Sequencer(sequencer::on_tick(state, context)),
            ),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => Ok(
                NativeBehaviorState::Keys(ported::keys_on_tick(state, context)),
            ),
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => Ok(
                NativeBehaviorState::Looper(ported::looper_on_tick(state, context)),
            ),
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => Ok(
                NativeBehaviorState::Brain(ported::brain_on_tick(state, context)),
            ),
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => Ok(NativeBehaviorState::Ant(
                ported::ant_on_tick(state, context),
            )),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => Ok(
                NativeBehaviorState::Bounce(ported::bounce_on_tick(state, context)),
            ),
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => Ok(
                NativeBehaviorState::Shapes(ported::shapes_on_tick(state, context)),
            ),
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => Ok(
                NativeBehaviorState::Raindrops(ported::raindrops_on_tick(state, context)),
            ),
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => Ok(NativeBehaviorState::Dla(
                ported::dla_on_tick(state, context),
            )),
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn render_model(self, state: &NativeBehaviorState) -> Result<BehaviorRenderModel, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(state)) => {
                Ok(none::render_model(state))
            }
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
                Ok(life::render_model(state))
            }
            (NativeBehavior::Glider, NativeBehaviorState::Glider(state)) => {
                Ok(glider::render_model(state))
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => {
                Ok(sequencer::render_model(state))
            }
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => {
                Ok(ported::keys_render_model(state))
            }
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => {
                Ok(ported::looper_render_model(state))
            }
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => {
                Ok(ported::brain_render_model(state))
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => {
                Ok(ported::ant_render_model(state))
            }
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => {
                Ok(ported::bounce_render_model(state))
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => {
                Ok(ported::shapes_render_model(state))
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
                Ok(ported::raindrops_render_model(state))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => {
                Ok(ported::dla_render_model(state))
            }
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn serialize(self, state: &NativeBehaviorState) -> Result<Value, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(state)) => none::serialize(state),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => life::serialize(state),
            (NativeBehavior::Glider, NativeBehaviorState::Glider(state)) => {
                glider::serialize(state)
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => {
                sequencer::serialize(state)
            }
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => ported::serialize(state),
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => {
                ported::looper_serialize(state)
            }
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => ported::serialize(state),
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => ported::serialize(state),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => {
                ported::serialize(state)
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => {
                ported::serialize(state)
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
                ported::serialize(state)
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => ported::serialize(state),
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn deserialize(self, data: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::None => Ok(NativeBehaviorState::None(none::deserialize(data)?)),
            NativeBehavior::Life => Ok(NativeBehaviorState::Life(life::deserialize(data)?)),
            NativeBehavior::Glider => Ok(NativeBehaviorState::Glider(glider::deserialize(data)?)),
            NativeBehavior::Sequencer => Ok(NativeBehaviorState::Sequencer(
                sequencer::deserialize(data)?,
            )),
            _ => self.deserialize_ported(data),
        }
    }
}
