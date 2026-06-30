use super::{glider, life, ported, NativeBehavior, NativeBehaviorState};
use crate::behavior::{BehaviorConfigItem, GridInteraction};

impl NativeBehavior {
    pub fn config_menu(
        self,
        state: &NativeBehaviorState,
    ) -> Result<Option<Vec<BehaviorConfigItem>>, String> {
        match (self, state) {
            (NativeBehavior::None, NativeBehaviorState::None(_)) => Ok(None),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
                Ok(Some(life::config_menu(state)))
            }
            (NativeBehavior::Glider, NativeBehaviorState::Glider(state)) => {
                Ok(Some(glider::config_menu(state)))
            }
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(_)) => Ok(None),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(_)) => {
                Ok(Some(ported::keys_config_menu()))
            }
            (NativeBehavior::Looper, NativeBehaviorState::Looper(_)) => {
                Ok(Some(ported::looper_config_menu()))
            }
            (NativeBehavior::Brain, NativeBehaviorState::Brain(_)) => {
                Ok(Some(ported::brain_config_menu()))
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(_)) => {
                Ok(Some(ported::ant_config_menu()))
            }
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(_)) => {
                Ok(Some(ported::bounce_config_menu()))
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(_)) => {
                Ok(Some(ported::shapes_config_menu()))
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(_)) => {
                Ok(Some(ported::raindrops_config_menu()))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(_)) => {
                Ok(Some(ported::dla_config_menu()))
            }
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn interpret_input_transitions(self) -> bool {
        !matches!(self, NativeBehavior::None | NativeBehavior::Sequencer)
    }

    pub fn grid_interaction(self) -> Option<GridInteraction> {
        match self {
            NativeBehavior::Keys => ported::grid_interaction_for_keys(),
            NativeBehavior::Looper => ported::grid_interaction_for_looper(),
            _ => None,
        }
    }
}
