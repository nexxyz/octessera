use super::{cellular, native_impl, NativeBehavior, NativeBehaviorState};
use crate::behavior::{BehaviorConfigItem, GridInteraction};

impl NativeBehavior {
    pub fn config_menu(
        self,
        state: &NativeBehaviorState,
    ) -> Result<Option<Vec<BehaviorConfigItem>>, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(_)) => Ok(None),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
                Ok(Some(cellular::life::config_menu(state)))
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(_)) => Ok(None),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(_)) => {
                Ok(Some(native_impl::keys_config_menu()))
            }
            (NativeBehavior::Looper, NativeBehaviorState::Looper(_)) => {
                Ok(Some(native_impl::looper_config_menu()))
            }
            (NativeBehavior::Brain, NativeBehaviorState::Brain(_)) => {
                Ok(Some(native_impl::brain_config_menu()))
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(_)) => {
                Ok(Some(native_impl::ant_config_menu()))
            }
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(_)) => {
                Ok(Some(native_impl::bounce_config_menu()))
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(_)) => {
                Ok(Some(native_impl::shapes_config_menu()))
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(_)) => {
                Ok(Some(native_impl::raindrops_config_menu()))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(_)) => {
                Ok(Some(native_impl::dla_config_menu()))
            }
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn interpret_input_transitions(self) -> bool {
        !matches!(self, NativeBehavior::None | NativeBehavior::Sequencer)
    }

    pub fn grid_interaction(self) -> Option<GridInteraction> {
        match self {
            NativeBehavior::Keys => native_impl::grid_interaction_for_keys(),
            NativeBehavior::Looper => native_impl::grid_interaction_for_looper(),
            _ => None,
        }
    }
}
