use super::*;

fn engine(behavior: NativeBehavior, random_seed: Option<u64>) -> NativeLayerEngine {
    NativeLayerEngine::new(NativeLayerEngineConfig {
        behavior,
        random_seed,
        ..base_config()
    })
    .unwrap()
}

fn run(engine: &mut NativeLayerEngine, ticks: usize) -> Vec<Vec<bool>> {
    (0..ticks)
        .map(|_| engine.tick(120.0).unwrap().model.cells)
        .collect()
}

#[test]
fn seeded_engines_replay_thread_random_behaviors() {
    for behavior in [NativeBehavior::ForestFire, NativeBehavior::Raindrops] {
        let mut first = engine(behavior, Some(7));
        let mut second = engine(behavior, Some(7));
        assert_eq!(run(&mut first, 24), run(&mut second, 24), "{behavior:?}");
    }
}

#[test]
fn restarting_the_stream_replays_from_the_same_state() {
    let mut engine = engine(NativeBehavior::ForestFire, Some(7));
    let start = engine.serialized_state().unwrap();
    engine.restart_random();
    let first = run(&mut engine, 24);
    let mut replay = NativeLayerEngine::from_serialized_state(
        NativeLayerEngineConfig {
            behavior: NativeBehavior::ForestFire,
            random_seed: Some(7),
            ..base_config()
        },
        start,
    )
    .unwrap();
    replay.restart_random();
    assert_eq!(run(&mut replay, 24), first);
}

#[test]
fn global_seed_reshapes_hash_random_behaviors() {
    let mut first = engine(NativeBehavior::Coral, Some(7));
    let mut second = engine(NativeBehavior::Coral, Some(8));
    let mut unseeded = engine(NativeBehavior::Coral, None);
    let mut unseeded_again = engine(NativeBehavior::Coral, None);
    assert_ne!(run(&mut first, 48), run(&mut second, 48));
    assert_eq!(run(&mut unseeded, 48), run(&mut unseeded_again, 48));
}
