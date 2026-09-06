use super::native_behavior::NativeBehaviorState;
use super::{pattern_music, play};

impl NativeBehaviorState {
    pub(crate) fn reset_transport_phase(&mut self) {
        match self {
            NativeBehaviorState::None(_) => {}
            NativeBehaviorState::Life(state) => state.tick_counter = 0,
            NativeBehaviorState::Sequencer(_) => {}
            NativeBehaviorState::Keys(_) => {}
            NativeBehaviorState::Looper(state) => play::looper::reset_transport_phase(state),
            NativeBehaviorState::Brain(state) => state.tick_counter = 0,
            NativeBehaviorState::Cyclic(_) => {}
            NativeBehaviorState::ForestFire(_) => {}
            NativeBehaviorState::PredatorPrey(_) => {}
            NativeBehaviorState::Twinkle(_) => {}
            NativeBehaviorState::Ant(state) => state.tick_counter = 0,
            NativeBehaviorState::Boids(state) => state.tick_counter = 0,
            NativeBehaviorState::Bounce(state) => state.tick_counter = 0,
            NativeBehaviorState::Bubbles(state) => state.tick_counter = 0,
            NativeBehaviorState::Gravity(state) => state.tick_counter = 0,
            NativeBehaviorState::LavaLamp(state) => state.tick_counter = 0,
            NativeBehaviorState::Orbit(state) => state.tick_counter = 0,
            NativeBehaviorState::SandRipples(state) => state.tick_counter = 0,
            NativeBehaviorState::FractalExplorer(state) => state.tick_counter = 0,
            NativeBehaviorState::MazeGrowth(state) => state.tick_counter = 0,
            NativeBehaviorState::Shapes(state) => state.tick_counter = 0,
            NativeBehaviorState::Ink(state) => state.tick_counter = 0,
            NativeBehaviorState::Ising(state) => state.tick_counter = 0,
            NativeBehaviorState::Kuramoto(_) => {}
            NativeBehaviorState::Lightning(_) => {}
            NativeBehaviorState::Wave(state) => state.tick_counter = 0,
            NativeBehaviorState::Raindrops(state) => state.tick_counter = 0,
            NativeBehaviorState::ReactionDiffusion(state) => state.tick_counter = 0,
            NativeBehaviorState::Rivers(state) => state.tick_counter = 0,
            NativeBehaviorState::Cracks(state) => state.tick_counter = 0,
            NativeBehaviorState::Coral(state) => state.tick_counter = 0,
            NativeBehaviorState::CrystalGrowth(state) => state.tick_counter = 0,
            NativeBehaviorState::Dla(state) => state.tick_counter = 0,
            NativeBehaviorState::Physarum(state) => state.tick_counter = 0,
            NativeBehaviorState::Vines(state) => state.tick_counter = 0,
            NativeBehaviorState::Pattern(state) => pattern_music::reset_transport_phase(state),
        }
    }
}
