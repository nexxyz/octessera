use super::{cellular, native_impl, pattern_music, play, NativeBehavior, NativeBehaviorState};
use crate::behavior::BehaviorContext;

pub fn on_tick(
    behavior: NativeBehavior,
    state: NativeBehaviorState,
    context: &mut BehaviorContext,
) -> Result<NativeBehaviorState, String> {
    match (behavior, state) {
        (behavior, NativeBehaviorState::Pattern(state)) if behavior.is_pattern() => Ok(
            NativeBehaviorState::Pattern(pattern_music::pattern_on_tick(state, context)),
        ),
        (NativeBehavior::None, NativeBehaviorState::None(state)) => Ok(NativeBehaviorState::None(
            play::none::on_tick(state, context),
        )),
        (NativeBehavior::Life, NativeBehaviorState::Life(state)) => Ok(NativeBehaviorState::Life(
            cellular::life::on_tick(state, context),
        )),
        (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => Ok(
            NativeBehaviorState::Sequencer(play::sequencer::on_tick(state, context)),
        ),
        (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => Ok(NativeBehaviorState::Keys(
            native_impl::keys_on_tick(state, context),
        )),
        (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => Ok(
            NativeBehaviorState::Looper(native_impl::looper_on_tick(state, context)),
        ),
        (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => Ok(
            NativeBehaviorState::Brain(native_impl::brain_on_tick(state, context)),
        ),
        (NativeBehavior::Cyclic, NativeBehaviorState::Cyclic(state)) => Ok(
            NativeBehaviorState::Cyclic(native_impl::cyclic_on_tick(state, context)),
        ),
        (NativeBehavior::ForestFire, NativeBehaviorState::ForestFire(state)) => Ok(
            NativeBehaviorState::ForestFire(native_impl::forest_fire_on_tick(state, context)),
        ),
        (NativeBehavior::PredatorPrey, NativeBehaviorState::PredatorPrey(state)) => Ok(
            NativeBehaviorState::PredatorPrey(native_impl::predator_prey_on_tick(state, context)),
        ),
        (NativeBehavior::Twinkle, NativeBehaviorState::Twinkle(state)) => Ok(
            NativeBehaviorState::Twinkle(native_impl::twinkle_on_tick(state, context)),
        ),
        (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => Ok(NativeBehaviorState::Ant(
            native_impl::ant_on_tick(state, context),
        )),
        (NativeBehavior::Boids, NativeBehaviorState::Boids(state)) => Ok(
            NativeBehaviorState::Boids(native_impl::boids_on_tick(state, context)),
        ),
        (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => Ok(
            NativeBehaviorState::Bounce(native_impl::bounce_on_tick(state, context)),
        ),
        (NativeBehavior::Bubbles, NativeBehaviorState::Bubbles(state)) => Ok(
            NativeBehaviorState::Bubbles(native_impl::bubbles_on_tick(state, context)),
        ),
        (NativeBehavior::Gravity, NativeBehaviorState::Gravity(state)) => Ok(
            NativeBehaviorState::Gravity(native_impl::gravity_on_tick(state, context)),
        ),
        (NativeBehavior::LavaLamp, NativeBehaviorState::LavaLamp(state)) => Ok(
            NativeBehaviorState::LavaLamp(native_impl::lava_lamp_on_tick(state, context)),
        ),
        (NativeBehavior::Orbit, NativeBehaviorState::Orbit(state)) => Ok(
            NativeBehaviorState::Orbit(native_impl::orbit_on_tick(state, context)),
        ),
        (NativeBehavior::SandRipples, NativeBehaviorState::SandRipples(state)) => Ok(
            NativeBehaviorState::SandRipples(native_impl::sand_ripples_on_tick(state, context)),
        ),
        (NativeBehavior::FractalExplorer, NativeBehaviorState::FractalExplorer(state)) => {
            Ok(NativeBehaviorState::FractalExplorer(
                native_impl::fractal_explorer_on_tick(state, context),
            ))
        }
        (NativeBehavior::MazeGrowth, NativeBehaviorState::MazeGrowth(state)) => Ok(
            NativeBehaviorState::MazeGrowth(native_impl::maze_growth_on_tick(state, context)),
        ),
        (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => Ok(
            NativeBehaviorState::Shapes(native_impl::shapes_on_tick(state, context)),
        ),
        (NativeBehavior::Ink, NativeBehaviorState::Ink(state)) => Ok(NativeBehaviorState::Ink(
            native_impl::ink_on_tick(state, context),
        )),
        (NativeBehavior::Ising, NativeBehaviorState::Ising(state)) => Ok(
            NativeBehaviorState::Ising(native_impl::ising_on_tick(state, context)),
        ),
        (NativeBehavior::Kuramoto, NativeBehaviorState::Kuramoto(state)) => Ok(
            NativeBehaviorState::Kuramoto(native_impl::kuramoto_on_tick(state, context)),
        ),
        (NativeBehavior::Lightning, NativeBehaviorState::Lightning(state)) => Ok(
            NativeBehaviorState::Lightning(native_impl::lightning_on_tick(state, context)),
        ),
        (NativeBehavior::Wave, NativeBehaviorState::Wave(state)) => Ok(NativeBehaviorState::Wave(
            native_impl::wave_on_tick(state, context),
        )),
        (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => Ok(
            NativeBehaviorState::Raindrops(native_impl::raindrops_on_tick(state, context)),
        ),
        (NativeBehavior::ReactionDiffusion, NativeBehaviorState::ReactionDiffusion(state)) => {
            Ok(NativeBehaviorState::ReactionDiffusion(
                native_impl::reaction_diffusion_on_tick(state, context),
            ))
        }
        (NativeBehavior::Rivers, NativeBehaviorState::Rivers(state)) => Ok(
            NativeBehaviorState::Rivers(native_impl::rivers_on_tick(state, context)),
        ),
        (NativeBehavior::Cracks, NativeBehaviorState::Cracks(state)) => Ok(
            NativeBehaviorState::Cracks(native_impl::cracks_on_tick(state, context)),
        ),
        (NativeBehavior::Coral, NativeBehaviorState::Coral(state)) => Ok(
            NativeBehaviorState::Coral(native_impl::coral_on_tick(state, context)),
        ),
        (NativeBehavior::CrystalGrowth, NativeBehaviorState::CrystalGrowth(state)) => Ok(
            NativeBehaviorState::CrystalGrowth(native_impl::crystal_growth_on_tick(state, context)),
        ),
        (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => Ok(NativeBehaviorState::Dla(
            native_impl::dla_on_tick(state, context),
        )),
        (NativeBehavior::Physarum, NativeBehaviorState::Physarum(state)) => Ok(
            NativeBehaviorState::Physarum(native_impl::physarum_on_tick(state, context)),
        ),
        (NativeBehavior::Vines, NativeBehaviorState::Vines(state)) => Ok(
            NativeBehaviorState::Vines(native_impl::vines_on_tick(state, context)),
        ),
        _ => Err(format!("state mismatch for behavior {}", behavior.id())),
    }
}
