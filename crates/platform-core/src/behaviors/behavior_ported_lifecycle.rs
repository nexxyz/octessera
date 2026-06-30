use super::{ported, NativeBehavior, NativeBehaviorState};
use serde_json::Value;

impl NativeBehavior {
    pub(super) fn init_ported(self, config: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::Keys => Ok(NativeBehaviorState::Keys(ported::keys_init(config)?)),
            NativeBehavior::Looper => Ok(NativeBehaviorState::Looper(ported::looper_init(config)?)),
            NativeBehavior::Brain => Ok(NativeBehaviorState::Brain(ported::brain_init(config)?)),
            NativeBehavior::Ant => Ok(NativeBehaviorState::Ant(ported::ant_init(config)?)),
            NativeBehavior::Bounce => Ok(NativeBehaviorState::Bounce(ported::bounce_init(config)?)),
            NativeBehavior::Shapes => Ok(NativeBehaviorState::Shapes(ported::shapes_init(config)?)),
            NativeBehavior::Raindrops => Ok(NativeBehaviorState::Raindrops(
                ported::raindrops_init(config)?,
            )),
            NativeBehavior::Dla => Ok(NativeBehaviorState::Dla(ported::dla_init(config)?)),
            _ => Err(format!(
                "unsupported ported init for behavior {}",
                self.id()
            )),
        }
    }

    pub(super) fn deserialize_ported(self, data: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::Keys => Ok(NativeBehaviorState::Keys(ported::deserialize(data)?)),
            NativeBehavior::Looper => Ok(NativeBehaviorState::Looper(ported::looper_deserialize(
                data,
            )?)),
            NativeBehavior::Brain => Ok(NativeBehaviorState::Brain(ported::deserialize(data)?)),
            NativeBehavior::Ant => Ok(NativeBehaviorState::Ant(ported::deserialize(data)?)),
            NativeBehavior::Bounce => Ok(NativeBehaviorState::Bounce(ported::deserialize(data)?)),
            NativeBehavior::Shapes => Ok(NativeBehaviorState::Shapes(ported::deserialize(data)?)),
            NativeBehavior::Raindrops => {
                Ok(NativeBehaviorState::Raindrops(ported::deserialize(data)?))
            }
            NativeBehavior::Dla => Ok(NativeBehaviorState::Dla(ported::deserialize(data)?)),
            _ => Err(format!(
                "unsupported ported deserialize for behavior {}",
                self.id()
            )),
        }
    }
}
