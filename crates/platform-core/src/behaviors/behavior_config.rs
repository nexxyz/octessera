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
            (NativeBehavior::Cyclic, NativeBehaviorState::Cyclic(_)) => {
                Ok(Some(native_impl::cyclic_config_menu()))
            }
            (NativeBehavior::ForestFire, NativeBehaviorState::ForestFire(_)) => {
                Ok(Some(native_impl::forest_fire_config_menu()))
            }
            (NativeBehavior::PredatorPrey, NativeBehaviorState::PredatorPrey(_)) => {
                Ok(Some(native_impl::predator_prey_config_menu()))
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(_)) => {
                Ok(Some(native_impl::ant_config_menu()))
            }
            (NativeBehavior::Boids, NativeBehaviorState::Boids(_)) => {
                Ok(Some(native_impl::boids_config_menu()))
            }
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(_)) => {
                Ok(Some(native_impl::bounce_config_menu()))
            }
            (NativeBehavior::Bubbles, NativeBehaviorState::Bubbles(_)) => {
                Ok(Some(native_impl::bubbles_config_menu()))
            }
            (NativeBehavior::Gravity, NativeBehaviorState::Gravity(_)) => {
                Ok(Some(native_impl::gravity_config_menu()))
            }
            (NativeBehavior::LavaLamp, NativeBehaviorState::LavaLamp(_)) => {
                Ok(Some(native_impl::lava_lamp_config_menu()))
            }
            (NativeBehavior::Orbit, NativeBehaviorState::Orbit(_)) => {
                Ok(Some(native_impl::orbit_config_menu()))
            }
            (NativeBehavior::SandRipples, NativeBehaviorState::SandRipples(_)) => {
                Ok(Some(native_impl::sand_ripples_config_menu()))
            }
            (NativeBehavior::FractalExplorer, NativeBehaviorState::FractalExplorer(_)) => {
                Ok(Some(native_impl::fractal_explorer_config_menu()))
            }
            (NativeBehavior::MazeGrowth, NativeBehaviorState::MazeGrowth(_)) => {
                Ok(Some(native_impl::maze_growth_config_menu()))
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(_)) => {
                Ok(Some(native_impl::shapes_config_menu()))
            }
            (NativeBehavior::Ink, NativeBehaviorState::Ink(_)) => {
                Ok(Some(native_impl::ink_config_menu()))
            }
            (NativeBehavior::Ising, NativeBehaviorState::Ising(_)) => {
                Ok(Some(native_impl::ising_config_menu()))
            }
            (NativeBehavior::Kuramoto, NativeBehaviorState::Kuramoto(_)) => {
                Ok(Some(native_impl::kuramoto_config_menu()))
            }
            (NativeBehavior::Lightning, NativeBehaviorState::Lightning(_)) => {
                Ok(Some(native_impl::lightning_config_menu()))
            }
            (NativeBehavior::Wave, NativeBehaviorState::Wave(_)) => {
                Ok(Some(native_impl::wave_config_menu()))
            }
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(_)) => {
                Ok(Some(native_impl::raindrops_config_menu()))
            }
            (NativeBehavior::ReactionDiffusion, NativeBehaviorState::ReactionDiffusion(_)) => {
                Ok(Some(native_impl::reaction_diffusion_config_menu()))
            }
            (NativeBehavior::Rivers, NativeBehaviorState::Rivers(_)) => {
                Ok(Some(native_impl::rivers_config_menu()))
            }
            (NativeBehavior::Cracks, NativeBehaviorState::Cracks(_)) => {
                Ok(Some(native_impl::cracks_config_menu()))
            }
            (NativeBehavior::Coral, NativeBehaviorState::Coral(_)) => {
                Ok(Some(native_impl::coral_config_menu()))
            }
            (NativeBehavior::CrystalGrowth, NativeBehaviorState::CrystalGrowth(_)) => {
                Ok(Some(native_impl::crystal_growth_config_menu()))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(_)) => {
                Ok(Some(native_impl::dla_config_menu()))
            }
            (NativeBehavior::Physarum, NativeBehaviorState::Physarum(_)) => {
                Ok(Some(native_impl::physarum_config_menu()))
            }
            (NativeBehavior::Vines, NativeBehaviorState::Vines(_)) => {
                Ok(Some(native_impl::vines_config_menu()))
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
