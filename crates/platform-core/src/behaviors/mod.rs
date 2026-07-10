mod behavior_config;
mod behavior_native_lifecycle;
mod catalog;
mod cellular;
mod fields;
mod geometry;
mod growth;
mod motion;
mod native_impl;
mod play;

use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use serde_json::Value;

#[cfg(test)]
mod tests;

pub use catalog::{behavior_catalog, behavior_categories, BehaviorCatalogEntry, BehaviorCategory};
pub use cellular::LifeState;
pub use native_impl::{
    AntState, BounceState, BrainState, DlaState, KeysState, LooperState, RaindropsState,
    ShapesState,
};
pub use play::{NoneState, SequencerState};

#[derive(Clone, Debug, PartialEq)]
pub enum NativeBehaviorState {
    None(NoneState),
    Life(LifeState),
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
    ]
}

impl NativeBehavior {
    pub fn id(self) -> &'static str {
        match self {
            NativeBehavior::None => "none",
            NativeBehavior::Life => "life",
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
            NativeBehavior::None => Ok(NativeBehaviorState::None(play::none::init(config)?)),
            NativeBehavior::Life => Ok(NativeBehaviorState::Life(cellular::life::init(config)?)),
            NativeBehavior::Sequencer => Ok(NativeBehaviorState::Sequencer(play::sequencer::init(
                config,
            )?)),
            _ => self.init_native(config),
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
                NativeBehaviorState::None(play::none::on_input(state, input, context)),
            ),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => Ok(
                NativeBehaviorState::Life(cellular::life::on_input(state, input, context)),
            ),
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => Ok(
                NativeBehaviorState::Sequencer(play::sequencer::on_input(state, input, context)),
            ),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => Ok(
                NativeBehaviorState::Keys(native_impl::keys_on_input(state, input, context)),
            ),
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => Ok(
                NativeBehaviorState::Looper(native_impl::looper_on_input(state, input, context)),
            ),
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => Ok(
                NativeBehaviorState::Brain(native_impl::brain_on_input(state, input, context)),
            ),
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => Ok(NativeBehaviorState::Ant(
                native_impl::ant_on_input(state, input, context),
            )),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => Ok(
                NativeBehaviorState::Bounce(native_impl::bounce_on_input(state, input, context)),
            ),
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => Ok(
                NativeBehaviorState::Shapes(native_impl::shapes_on_input(state, input, context)),
            ),
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
                Ok(NativeBehaviorState::Raindrops(
                    native_impl::raindrops_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => Ok(NativeBehaviorState::Dla(
                native_impl::dla_on_input(state, input, context),
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
            (NativeBehavior::None, NativeBehaviorState::None(state)) => Ok(
                NativeBehaviorState::None(play::none::on_tick(state, context)),
            ),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => Ok(
                NativeBehaviorState::Life(cellular::life::on_tick(state, context)),
            ),
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => Ok(
                NativeBehaviorState::Sequencer(play::sequencer::on_tick(state, context)),
            ),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => Ok(
                NativeBehaviorState::Keys(native_impl::keys_on_tick(state, context)),
            ),
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => Ok(
                NativeBehaviorState::Looper(native_impl::looper_on_tick(state, context)),
            ),
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => Ok(
                NativeBehaviorState::Brain(native_impl::brain_on_tick(state, context)),
            ),
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => Ok(NativeBehaviorState::Ant(
                native_impl::ant_on_tick(state, context),
            )),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => Ok(
                NativeBehaviorState::Bounce(native_impl::bounce_on_tick(state, context)),
            ),
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => Ok(
                NativeBehaviorState::Shapes(native_impl::shapes_on_tick(state, context)),
            ),
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => Ok(
                NativeBehaviorState::Raindrops(native_impl::raindrops_on_tick(state, context)),
            ),
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => Ok(NativeBehaviorState::Dla(
                native_impl::dla_on_tick(state, context),
            )),
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn render_model(self, state: &NativeBehaviorState) -> Result<BehaviorRenderModel, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(state)) => {
                Ok(play::none::render_model(state))
            }
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
                Ok(cellular::life::render_model(state))
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => {
                Ok(play::sequencer::render_model(state))
            }
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => {
                Ok(native_impl::keys_render_model(state))
            }
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => {
                Ok(native_impl::looper_render_model(state))
            }
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => {
                Ok(native_impl::brain_render_model(state))
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => {
                Ok(native_impl::ant_render_model(state))
            }
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => {
                Ok(native_impl::bounce_render_model(state))
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => {
                Ok(native_impl::shapes_render_model(state))
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
                Ok(native_impl::raindrops_render_model(state))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => {
                Ok(native_impl::dla_render_model(state))
            }
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn serialize(self, state: &NativeBehaviorState) -> Result<Value, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(state)) => {
                play::none::serialize(state)
            }
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
                cellular::life::serialize(state)
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => {
                play::sequencer::serialize(state)
            }
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => {
                native_impl::serialize(state)
            }
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => {
                native_impl::looper_serialize(state)
            }
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => {
                native_impl::serialize(state)
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => native_impl::serialize(state),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => {
                native_impl::serialize(state)
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => {
                native_impl::serialize(state)
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
                native_impl::serialize(state)
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => native_impl::serialize(state),
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn deserialize(self, data: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::None => Ok(NativeBehaviorState::None(play::none::deserialize(data)?)),
            NativeBehavior::Life => Ok(NativeBehaviorState::Life(cellular::life::deserialize(
                data,
            )?)),
            NativeBehavior::Sequencer => Ok(NativeBehaviorState::Sequencer(
                play::sequencer::deserialize(data)?,
            )),
            _ => self.deserialize_native(data),
        }
    }
}
