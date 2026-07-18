use super::{native_impl, pattern_music, NativeBehavior, NativeBehaviorState};
use serde_json::Value;

impl NativeBehavior {
    pub(super) fn init_native(self, config: Value) -> Result<NativeBehaviorState, String> {
        if self.is_pattern() {
            return Ok(NativeBehaviorState::Pattern(pattern_music::pattern_init(
                self.id(),
                config,
            )?));
        }
        match self {
            NativeBehavior::Keys => Ok(NativeBehaviorState::Keys(native_impl::keys_init(config)?)),
            NativeBehavior::Looper => Ok(NativeBehaviorState::Looper(native_impl::looper_init(
                config,
            )?)),
            NativeBehavior::Brain => {
                Ok(NativeBehaviorState::Brain(native_impl::brain_init(config)?))
            }
            NativeBehavior::Cyclic => Ok(NativeBehaviorState::Cyclic(native_impl::cyclic_init(
                config,
            )?)),
            NativeBehavior::ForestFire => Ok(NativeBehaviorState::ForestFire(
                native_impl::forest_fire_init(config)?,
            )),
            NativeBehavior::PredatorPrey => Ok(NativeBehaviorState::PredatorPrey(
                native_impl::predator_prey_init(config)?,
            )),
            NativeBehavior::Ant => Ok(NativeBehaviorState::Ant(native_impl::ant_init(config)?)),
            NativeBehavior::Boids => {
                Ok(NativeBehaviorState::Boids(native_impl::boids_init(config)?))
            }
            NativeBehavior::Bounce => Ok(NativeBehaviorState::Bounce(native_impl::bounce_init(
                config,
            )?)),
            NativeBehavior::Bubbles => Ok(NativeBehaviorState::Bubbles(native_impl::bubbles_init(
                config,
            )?)),
            NativeBehavior::Gravity => Ok(NativeBehaviorState::Gravity(native_impl::gravity_init(
                config,
            )?)),
            NativeBehavior::LavaLamp => Ok(NativeBehaviorState::LavaLamp(
                native_impl::lava_lamp_init(config)?,
            )),
            NativeBehavior::Orbit => {
                Ok(NativeBehaviorState::Orbit(native_impl::orbit_init(config)?))
            }
            NativeBehavior::SandRipples => Ok(NativeBehaviorState::SandRipples(
                native_impl::sand_ripples_init(config)?,
            )),
            NativeBehavior::FractalExplorer => Ok(NativeBehaviorState::FractalExplorer(
                native_impl::fractal_explorer_init(config)?,
            )),
            NativeBehavior::MazeGrowth => Ok(NativeBehaviorState::MazeGrowth(
                native_impl::maze_growth_init(config)?,
            )),
            NativeBehavior::Shapes => Ok(NativeBehaviorState::Shapes(native_impl::shapes_init(
                config,
            )?)),
            NativeBehavior::Ink => Ok(NativeBehaviorState::Ink(native_impl::ink_init(config)?)),
            NativeBehavior::Ising => {
                Ok(NativeBehaviorState::Ising(native_impl::ising_init(config)?))
            }
            NativeBehavior::Kuramoto => Ok(NativeBehaviorState::Kuramoto(
                native_impl::kuramoto_init(config)?,
            )),
            NativeBehavior::Lightning => Ok(NativeBehaviorState::Lightning(
                native_impl::lightning_init(config)?,
            )),
            NativeBehavior::Wave => Ok(NativeBehaviorState::Wave(native_impl::wave_init(config)?)),
            NativeBehavior::Raindrops => Ok(NativeBehaviorState::Raindrops(
                native_impl::raindrops_init(config)?,
            )),
            NativeBehavior::ReactionDiffusion => Ok(NativeBehaviorState::ReactionDiffusion(
                native_impl::reaction_diffusion_init(config)?,
            )),
            NativeBehavior::Rivers => Ok(NativeBehaviorState::Rivers(native_impl::rivers_init(
                config,
            )?)),
            NativeBehavior::Cracks => Ok(NativeBehaviorState::Cracks(native_impl::cracks_init(
                config,
            )?)),
            NativeBehavior::Coral => {
                Ok(NativeBehaviorState::Coral(native_impl::coral_init(config)?))
            }
            NativeBehavior::CrystalGrowth => Ok(NativeBehaviorState::CrystalGrowth(
                native_impl::crystal_growth_init(config)?,
            )),
            NativeBehavior::Dla => Ok(NativeBehaviorState::Dla(native_impl::dla_init(config)?)),
            NativeBehavior::Physarum => Ok(NativeBehaviorState::Physarum(
                native_impl::physarum_init(config)?,
            )),
            NativeBehavior::Vines => {
                Ok(NativeBehaviorState::Vines(native_impl::vines_init(config)?))
            }
            _ => Err(format!(
                "unsupported native init for behavior {}",
                self.id()
            )),
        }
    }

    pub(super) fn deserialize_native(self, data: Value) -> Result<NativeBehaviorState, String> {
        if self.is_pattern() {
            return Ok(NativeBehaviorState::Pattern(pattern_music::pattern_init(
                self.id(),
                data,
            )?));
        }
        match self {
            NativeBehavior::Keys => Ok(NativeBehaviorState::Keys(native_impl::deserialize(data)?)),
            NativeBehavior::Looper => Ok(NativeBehaviorState::Looper(
                native_impl::looper_deserialize(data)?,
            )),
            NativeBehavior::Brain => {
                Ok(NativeBehaviorState::Brain(native_impl::deserialize(data)?))
            }
            NativeBehavior::Cyclic => Ok(NativeBehaviorState::Cyclic(
                native_impl::cyclic_deserialize(data)?,
            )),
            NativeBehavior::ForestFire => Ok(NativeBehaviorState::ForestFire(
                native_impl::forest_fire_deserialize(data)?,
            )),
            NativeBehavior::PredatorPrey => Ok(NativeBehaviorState::PredatorPrey(
                native_impl::predator_prey_deserialize(data)?,
            )),
            NativeBehavior::Ant => Ok(NativeBehaviorState::Ant(native_impl::deserialize(data)?)),
            NativeBehavior::Boids => Ok(NativeBehaviorState::Boids(
                native_impl::boids_deserialize(data)?,
            )),
            NativeBehavior::Bounce => {
                Ok(NativeBehaviorState::Bounce(native_impl::deserialize(data)?))
            }
            NativeBehavior::Bubbles => Ok(NativeBehaviorState::Bubbles(
                native_impl::bubbles_deserialize(data)?,
            )),
            NativeBehavior::Gravity => Ok(NativeBehaviorState::Gravity(
                native_impl::gravity_deserialize(data)?,
            )),
            NativeBehavior::LavaLamp => Ok(NativeBehaviorState::LavaLamp(
                native_impl::lava_lamp_deserialize(data)?,
            )),
            NativeBehavior::Orbit => Ok(NativeBehaviorState::Orbit(
                native_impl::orbit_deserialize(data)?,
            )),
            NativeBehavior::SandRipples => Ok(NativeBehaviorState::SandRipples(
                native_impl::sand_ripples_deserialize(data)?,
            )),
            NativeBehavior::FractalExplorer => Ok(NativeBehaviorState::FractalExplorer(
                native_impl::fractal_explorer_deserialize(data)?,
            )),
            NativeBehavior::MazeGrowth => Ok(NativeBehaviorState::MazeGrowth(
                native_impl::maze_growth_deserialize(data)?,
            )),
            NativeBehavior::Shapes => {
                Ok(NativeBehaviorState::Shapes(native_impl::deserialize(data)?))
            }
            NativeBehavior::Ink => Ok(NativeBehaviorState::Ink(native_impl::ink_deserialize(
                data,
            )?)),
            NativeBehavior::Ising => Ok(NativeBehaviorState::Ising(
                native_impl::ising_deserialize(data)?,
            )),
            NativeBehavior::Kuramoto => Ok(NativeBehaviorState::Kuramoto(
                native_impl::kuramoto_deserialize(data)?,
            )),
            NativeBehavior::Lightning => Ok(NativeBehaviorState::Lightning(
                native_impl::lightning_deserialize(data)?,
            )),
            NativeBehavior::Wave => Ok(NativeBehaviorState::Wave(native_impl::wave_deserialize(
                data,
            )?)),
            NativeBehavior::Raindrops => Ok(NativeBehaviorState::Raindrops(
                native_impl::deserialize(data)?,
            )),
            NativeBehavior::ReactionDiffusion => Ok(NativeBehaviorState::ReactionDiffusion(
                native_impl::reaction_diffusion_deserialize(data)?,
            )),
            NativeBehavior::Rivers => Ok(NativeBehaviorState::Rivers(
                native_impl::rivers_deserialize(data)?,
            )),
            NativeBehavior::Cracks => Ok(NativeBehaviorState::Cracks(
                native_impl::cracks_deserialize(data)?,
            )),
            NativeBehavior::Coral => Ok(NativeBehaviorState::Coral(
                native_impl::coral_deserialize(data)?,
            )),
            NativeBehavior::CrystalGrowth => Ok(NativeBehaviorState::CrystalGrowth(
                native_impl::crystal_growth_deserialize(data)?,
            )),
            NativeBehavior::Dla => Ok(NativeBehaviorState::Dla(native_impl::deserialize(data)?)),
            NativeBehavior::Physarum => Ok(NativeBehaviorState::Physarum(
                native_impl::physarum_deserialize(data)?,
            )),
            NativeBehavior::Vines => Ok(NativeBehaviorState::Vines(
                native_impl::vines_deserialize(data)?,
            )),
            _ => Err(format!(
                "unsupported native deserialize for behavior {}",
                self.id()
            )),
        }
    }
}
