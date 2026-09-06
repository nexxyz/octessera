use super::*;
use crate::behavior::BehaviorContext;
use serde_json::Value;

const CADENCE_BEHAVIORS: &[NativeBehavior] = &[
    NativeBehavior::Life,
    NativeBehavior::Brain,
    NativeBehavior::Ant,
    NativeBehavior::Bounce,
    NativeBehavior::Bubbles,
    NativeBehavior::Boids,
    NativeBehavior::Gravity,
    NativeBehavior::LavaLamp,
    NativeBehavior::Orbit,
    NativeBehavior::SandRipples,
    NativeBehavior::FractalExplorer,
    NativeBehavior::MazeGrowth,
    NativeBehavior::Shapes,
    NativeBehavior::Ink,
    NativeBehavior::Ising,
    NativeBehavior::Wave,
    NativeBehavior::Raindrops,
    NativeBehavior::ReactionDiffusion,
    NativeBehavior::Rivers,
    NativeBehavior::Cracks,
    NativeBehavior::Coral,
    NativeBehavior::CrystalGrowth,
    NativeBehavior::Dla,
    NativeBehavior::Physarum,
    NativeBehavior::Vines,
];

#[test]
fn reset_transport_phase_clears_every_behavior_cadence() {
    for behavior in CADENCE_BEHAVIORS {
        let mut context = BehaviorContext::new(120.0);
        let state = behavior.init(Value::Null).unwrap();
        let mut state = behavior.on_tick(state, &mut context).unwrap();
        assert!(
            cadence(&state).is_some_and(|value| value > 0),
            "{behavior:?}"
        );

        state.reset_transport_phase();

        assert_eq!(cadence(&state), Some(0), "{behavior:?}");
    }
}

fn cadence(state: &NativeBehaviorState) -> Option<u64> {
    match state {
        NativeBehaviorState::None(_) => None,
        NativeBehaviorState::Life(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Sequencer(_) => None,
        NativeBehaviorState::Keys(_) => None,
        NativeBehaviorState::Looper(_) => None,
        NativeBehaviorState::Brain(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Cyclic(_) => None,
        NativeBehaviorState::ForestFire(_) => None,
        NativeBehaviorState::PredatorPrey(_) => None,
        NativeBehaviorState::Twinkle(_) => None,
        NativeBehaviorState::Ant(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Boids(state) => Some(state.tick_counter),
        NativeBehaviorState::Bounce(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Bubbles(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Gravity(state) => Some(state.tick_counter),
        NativeBehaviorState::LavaLamp(state) => Some(state.tick_counter),
        NativeBehaviorState::Orbit(state) => Some(state.tick_counter),
        NativeBehaviorState::SandRipples(state) => Some(state.tick_counter),
        NativeBehaviorState::FractalExplorer(state) => Some(state.tick_counter),
        NativeBehaviorState::MazeGrowth(state) => Some(state.tick_counter),
        NativeBehaviorState::Shapes(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Ink(state) => Some(state.tick_counter),
        NativeBehaviorState::Ising(state) => Some(state.tick_counter),
        NativeBehaviorState::Kuramoto(_) => None,
        NativeBehaviorState::Lightning(_) => None,
        NativeBehaviorState::Wave(state) => Some(state.tick_counter),
        NativeBehaviorState::Raindrops(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::ReactionDiffusion(state) => Some(state.tick_counter),
        NativeBehaviorState::Rivers(state) => Some(state.tick_counter),
        NativeBehaviorState::Cracks(state) => Some(state.tick_counter),
        NativeBehaviorState::Coral(state) => Some(state.tick_counter),
        NativeBehaviorState::CrystalGrowth(state) => Some(state.tick_counter),
        NativeBehaviorState::Dla(state) => Some(state.tick_counter as u64),
        NativeBehaviorState::Physarum(state) => Some(state.tick_counter),
        NativeBehaviorState::Vines(state) => Some(state.tick_counter),
        NativeBehaviorState::Pattern(_) => None,
    }
}
