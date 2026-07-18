use super::{cellular, native_impl, play, NativeBehavior, NativeBehaviorState};
use serde_json::Value;

pub fn serialize(behavior: NativeBehavior, state: &NativeBehaviorState) -> Result<Value, String> {
    match (behavior, state) {
        (behavior, NativeBehaviorState::Pattern(state)) if behavior.is_pattern() => {
            native_impl::serialize(state)
        }
        (NativeBehavior::None, NativeBehaviorState::None(state)) => play::none::serialize(state),
        (NativeBehavior::Life, NativeBehaviorState::Life(state)) => {
            cellular::life::serialize(state)
        }
        (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => {
            play::sequencer::serialize(state)
        }
        (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => native_impl::serialize(state),
        (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => {
            native_impl::looper_serialize(state)
        }
        (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => native_impl::serialize(state),
        (NativeBehavior::Cyclic, NativeBehaviorState::Cyclic(state)) => {
            native_impl::cyclic_serialize(state)
        }
        (NativeBehavior::ForestFire, NativeBehaviorState::ForestFire(state)) => {
            native_impl::serialize(state)
        }
        (NativeBehavior::PredatorPrey, NativeBehaviorState::PredatorPrey(state)) => {
            native_impl::predator_prey_serialize(state)
        }
        (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => native_impl::serialize(state),
        (NativeBehavior::Boids, NativeBehaviorState::Boids(state)) => {
            native_impl::boids_serialize(state)
        }
        (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => {
            native_impl::serialize(state)
        }
        (NativeBehavior::Bubbles, NativeBehaviorState::Bubbles(state)) => {
            native_impl::bubbles_serialize(state)
        }
        (NativeBehavior::Gravity, NativeBehaviorState::Gravity(state)) => {
            native_impl::gravity_serialize(state)
        }
        (NativeBehavior::LavaLamp, NativeBehaviorState::LavaLamp(state)) => {
            native_impl::lava_lamp_serialize(state)
        }
        (NativeBehavior::Orbit, NativeBehaviorState::Orbit(state)) => {
            native_impl::orbit_serialize(state)
        }
        (NativeBehavior::SandRipples, NativeBehaviorState::SandRipples(state)) => {
            native_impl::sand_ripples_serialize(state)
        }
        (NativeBehavior::FractalExplorer, NativeBehaviorState::FractalExplorer(state)) => {
            native_impl::fractal_explorer_serialize(state)
        }
        (NativeBehavior::MazeGrowth, NativeBehaviorState::MazeGrowth(state)) => {
            native_impl::maze_growth_serialize(state)
        }
        (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => {
            native_impl::serialize(state)
        }
        (NativeBehavior::Ink, NativeBehaviorState::Ink(state)) => native_impl::ink_serialize(state),
        (NativeBehavior::Ising, NativeBehaviorState::Ising(state)) => {
            native_impl::ising_serialize(state)
        }
        (NativeBehavior::Kuramoto, NativeBehaviorState::Kuramoto(state)) => {
            native_impl::kuramoto_serialize(state)
        }
        (NativeBehavior::Lightning, NativeBehaviorState::Lightning(state)) => {
            native_impl::lightning_serialize(state)
        }
        (NativeBehavior::Wave, NativeBehaviorState::Wave(state)) => {
            native_impl::wave_serialize(state)
        }
        (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
            native_impl::serialize(state)
        }
        (NativeBehavior::ReactionDiffusion, NativeBehaviorState::ReactionDiffusion(state)) => {
            native_impl::reaction_diffusion_serialize(state)
        }
        (NativeBehavior::Rivers, NativeBehaviorState::Rivers(state)) => {
            native_impl::rivers_serialize(state)
        }
        (NativeBehavior::Cracks, NativeBehaviorState::Cracks(state)) => {
            native_impl::cracks_serialize(state)
        }
        (NativeBehavior::Coral, NativeBehaviorState::Coral(state)) => {
            native_impl::coral_serialize(state)
        }
        (NativeBehavior::CrystalGrowth, NativeBehaviorState::CrystalGrowth(state)) => {
            native_impl::crystal_growth_serialize(state)
        }
        (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => native_impl::serialize(state),
        (NativeBehavior::Physarum, NativeBehaviorState::Physarum(state)) => {
            native_impl::physarum_serialize(state)
        }
        (NativeBehavior::Vines, NativeBehaviorState::Vines(state)) => {
            native_impl::vines_serialize(state)
        }
        _ => Err(format!("state mismatch for behavior {}", behavior.id())),
    }
}
