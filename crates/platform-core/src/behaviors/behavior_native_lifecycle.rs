use super::{native_impl, NativeBehavior, NativeBehaviorState};
use serde_json::Value;

impl NativeBehavior {
    pub(super) fn init_native(self, config: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::Keys => Ok(NativeBehaviorState::Keys(native_impl::keys_init(config)?)),
            NativeBehavior::Looper => Ok(NativeBehaviorState::Looper(native_impl::looper_init(
                config,
            )?)),
            NativeBehavior::Brain => {
                Ok(NativeBehaviorState::Brain(native_impl::brain_init(config)?))
            }
            NativeBehavior::Ant => Ok(NativeBehaviorState::Ant(native_impl::ant_init(config)?)),
            NativeBehavior::Bounce => Ok(NativeBehaviorState::Bounce(native_impl::bounce_init(
                config,
            )?)),
            NativeBehavior::Shapes => Ok(NativeBehaviorState::Shapes(native_impl::shapes_init(
                config,
            )?)),
            NativeBehavior::Raindrops => Ok(NativeBehaviorState::Raindrops(
                native_impl::raindrops_init(config)?,
            )),
            NativeBehavior::Dla => Ok(NativeBehaviorState::Dla(native_impl::dla_init(config)?)),
            _ => Err(format!(
                "unsupported native init for behavior {}",
                self.id()
            )),
        }
    }

    pub(super) fn deserialize_native(self, data: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::Keys => Ok(NativeBehaviorState::Keys(native_impl::deserialize(data)?)),
            NativeBehavior::Looper => Ok(NativeBehaviorState::Looper(
                native_impl::looper_deserialize(data)?,
            )),
            NativeBehavior::Brain => {
                Ok(NativeBehaviorState::Brain(native_impl::deserialize(data)?))
            }
            NativeBehavior::Ant => Ok(NativeBehaviorState::Ant(native_impl::deserialize(data)?)),
            NativeBehavior::Bounce => {
                Ok(NativeBehaviorState::Bounce(native_impl::deserialize(data)?))
            }
            NativeBehavior::Shapes => {
                Ok(NativeBehaviorState::Shapes(native_impl::deserialize(data)?))
            }
            NativeBehavior::Raindrops => Ok(NativeBehaviorState::Raindrops(
                native_impl::deserialize(data)?,
            )),
            NativeBehavior::Dla => Ok(NativeBehaviorState::Dla(native_impl::deserialize(data)?)),
            _ => Err(format!(
                "unsupported native deserialize for behavior {}",
                self.id()
            )),
        }
    }
}
