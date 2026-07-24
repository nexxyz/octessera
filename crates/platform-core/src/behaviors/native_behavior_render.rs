use super::{cellular, native_impl, pattern_music, play, NativeBehavior, NativeBehaviorState};
use crate::behavior::BehaviorRenderModel;

pub fn render_model(
    behavior: NativeBehavior,
    state: &NativeBehaviorState,
) -> Result<BehaviorRenderModel, String> {
    match (behavior, state) {
        (behavior, NativeBehaviorState::Pattern(state)) if behavior.is_pattern() => {
            Ok(pattern_music::pattern_render_model(state))
        }
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
        (NativeBehavior::Cyclic, NativeBehaviorState::Cyclic(state)) => {
            Ok(native_impl::cyclic_render_model(state))
        }
        (NativeBehavior::ForestFire, NativeBehaviorState::ForestFire(state)) => {
            Ok(native_impl::forest_fire_render_model(state))
        }
        (NativeBehavior::PredatorPrey, NativeBehaviorState::PredatorPrey(state)) => {
            Ok(native_impl::predator_prey_render_model(state))
        }
        (NativeBehavior::Twinkle, NativeBehaviorState::Twinkle(state)) => {
            Ok(native_impl::twinkle_render_model(state))
        }
        (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => {
            Ok(native_impl::ant_render_model(state))
        }
        (NativeBehavior::Boids, NativeBehaviorState::Boids(state)) => {
            Ok(native_impl::boids_render_model(state))
        }
        (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => {
            Ok(native_impl::bounce_render_model(state))
        }
        (NativeBehavior::Bubbles, NativeBehaviorState::Bubbles(state)) => {
            Ok(native_impl::bubbles_render_model(state))
        }
        (NativeBehavior::Gravity, NativeBehaviorState::Gravity(state)) => {
            Ok(native_impl::gravity_render_model(state))
        }
        (NativeBehavior::LavaLamp, NativeBehaviorState::LavaLamp(state)) => {
            Ok(native_impl::lava_lamp_render_model(state))
        }
        (NativeBehavior::Orbit, NativeBehaviorState::Orbit(state)) => {
            Ok(native_impl::orbit_render_model(state))
        }
        (NativeBehavior::SandRipples, NativeBehaviorState::SandRipples(state)) => {
            Ok(native_impl::sand_ripples_render_model(state))
        }
        (NativeBehavior::FractalExplorer, NativeBehaviorState::FractalExplorer(state)) => {
            Ok(native_impl::fractal_explorer_render_model(state))
        }
        (NativeBehavior::MazeGrowth, NativeBehaviorState::MazeGrowth(state)) => {
            Ok(native_impl::maze_growth_render_model(state))
        }
        (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => {
            Ok(native_impl::shapes_render_model(state))
        }
        (NativeBehavior::Ink, NativeBehaviorState::Ink(state)) => {
            Ok(native_impl::ink_render_model(state))
        }
        (NativeBehavior::Ising, NativeBehaviorState::Ising(state)) => {
            Ok(native_impl::ising_render_model(state))
        }
        (NativeBehavior::Kuramoto, NativeBehaviorState::Kuramoto(state)) => {
            Ok(native_impl::kuramoto_render_model(state))
        }
        (NativeBehavior::Lightning, NativeBehaviorState::Lightning(state)) => {
            Ok(native_impl::lightning_render_model(state))
        }
        (NativeBehavior::Wave, NativeBehaviorState::Wave(state)) => {
            Ok(native_impl::wave_render_model(state))
        }
        (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
            Ok(native_impl::raindrops_render_model(state))
        }
        (NativeBehavior::ReactionDiffusion, NativeBehaviorState::ReactionDiffusion(state)) => {
            Ok(native_impl::reaction_diffusion_render_model(state))
        }
        (NativeBehavior::Rivers, NativeBehaviorState::Rivers(state)) => {
            Ok(native_impl::rivers_render_model(state))
        }
        (NativeBehavior::Cracks, NativeBehaviorState::Cracks(state)) => {
            Ok(native_impl::cracks_render_model(state))
        }
        (NativeBehavior::Coral, NativeBehaviorState::Coral(state)) => {
            Ok(native_impl::coral_render_model(state))
        }
        (NativeBehavior::CrystalGrowth, NativeBehaviorState::CrystalGrowth(state)) => {
            Ok(native_impl::crystal_growth_render_model(state))
        }
        (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => {
            Ok(native_impl::dla_render_model(state))
        }
        (NativeBehavior::Physarum, NativeBehaviorState::Physarum(state)) => {
            Ok(native_impl::physarum_render_model(state))
        }
        (NativeBehavior::Vines, NativeBehaviorState::Vines(state)) => {
            Ok(native_impl::vines_render_model(state))
        }
        _ => Err(format!("state mismatch for behavior {}", behavior.id())),
    }
}
