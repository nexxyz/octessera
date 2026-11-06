use super::activate_cadence_table::{Expect, CADENCE};
use super::{get_native_behavior, list_native_behavior_ids, NativeBehavior};
use crate::behavior::BehaviorContext;
use crate::behavior_random::LayerRandom;
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use crate::interpretation::{
    interpret_grid_with_marker_authority, AxisStrategy, GridSnapshot, InterpretationEventProfile,
    InterpretationProfile, InterpretationStateProfile, TickStrategy, TileTriggerKind,
    TriggerMarkerAuthority,
};
use serde_json::Value;

fn event_profile() -> InterpretationProfile {
    InterpretationProfile {
        id: "cadence".into(),
        event: InterpretationEventProfile { enabled: true },
        state: InterpretationStateProfile {
            enabled: false,
            tick: TickStrategy::WholeGridTransitions,
        },
        x: AxisStrategy::Ignore,
        y: AxisStrategy::Ignore,
    }
}

fn snapshot(behavior: NativeBehavior, state: &super::NativeBehaviorState) -> GridSnapshot {
    let model = behavior.render_model(state).unwrap();
    GridSnapshot {
        width: GRID_WIDTH,
        height: GRID_HEIGHT,
        tiles: model.tiles,
        trigger_types: model.trigger_types,
    }
}

/// Activate ticks of one default-init run, as the runtime's event interpretation sees them.
fn activate_ticks(behavior: NativeBehavior, salt: u64, ticks: usize) -> Vec<usize> {
    LayerRandom::new(Some(salt)).scope(|| {
        let profile = event_profile();
        let mut context = BehaviorContext::new(120.0);
        let mut state = behavior.init(Value::Null).unwrap();
        let mut previous = snapshot(behavior, &state);
        let mut activates = Vec::new();
        for tick in 1..=ticks {
            state = behavior.on_tick(state, &mut context).unwrap();
            let next = snapshot(behavior, &state);
            if interpret_grid_with_marker_authority(
                &previous,
                &next,
                tick,
                &profile,
                TriggerMarkerAuthority::FreshTick,
            )
            .iter()
            .any(|intent| intent.kind == TileTriggerKind::Activate)
            {
                activates.push(tick);
            }
            previous = next;
        }
        activates
    })
}

fn longest_gap(activates: &[usize], ticks: usize) -> usize {
    let mut last = 0;
    let mut longest = 0;
    for tick in activates.iter().copied().chain([ticks + 1]) {
        longest = longest.max(tick - last);
        last = tick;
    }
    longest
}

fn check_cadence(salts: u64, ticks: usize) {
    let ids = list_native_behavior_ids();
    assert_eq!(CADENCE.len(), ids.len());
    for id in ids {
        assert!(
            CADENCE.iter().any(|row| row.id == *id),
            "{id} has no cadence row"
        );
    }
    for row in CADENCE {
        let behavior = get_native_behavior(row.id).unwrap();
        for salt in 0..salts {
            let activates = activate_ticks(behavior, salt, ticks);
            match row.expect {
                Expect::Within(window) => {
                    let gap = longest_gap(&activates, ticks);
                    assert!(
                        gap <= window,
                        "{} salt {salt}: {gap} ticks without an Activate, expected at most {window} ({})",
                        row.id,
                        row.why
                    );
                }
                Expect::Silent => assert!(
                    activates.is_empty(),
                    "{} salt {salt}: activated at ticks {activates:?} ({})",
                    row.id,
                    row.why
                ),
            }
        }
    }
}

#[test]
fn default_behaviors_keep_their_activate_cadence() {
    check_cadence(16, 1000);
}

/// The sweep the windows come from; run it with `--ignored --release` after behavior changes.
#[test]
#[ignore = "heavy sweep, run with --release"]
fn default_behaviors_keep_their_activate_cadence_heavy_sweep() {
    check_cadence(200, 3000);
}
