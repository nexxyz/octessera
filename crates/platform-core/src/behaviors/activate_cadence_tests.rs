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

#[derive(Clone, Copy, Debug)]
enum Expect {
    /// Activates within every window of this many ticks.
    Within(usize),
    /// Never activates without input.
    Silent,
}
use Expect::{Silent, Within};

struct Row {
    id: &'static str,
    expect: Expect,
    why: &'static str,
}

const fn row(id: &'static str, expect: Expect, why: &'static str) -> Row {
    Row { id, expect, why }
}

/// How each behavior sounds from its default init with no input. `Within` windows are about twice
/// the longest Activate-free gap of a 200-salt, 3000-tick sweep, rounded to the behavior's cadence.
const CADENCE: &[Row] = &[
    row("none", Silent, "nothing to run"),
    row(
        "sequencer",
        Silent,
        "plays only what is drawn; a default canvas is empty",
    ),
    row("keys", Silent, "plays only what is pressed"),
    row("looper", Silent, "plays only what is recorded"),
    row("life", Within(4), "a generation always flips some cell on"),
    row(
        "brain",
        Within(4),
        "firing cells hand on to their neighbors every tick",
    ),
    row(
        "cyclic",
        Within(4),
        "every state keeps eating its successor",
    ),
    row(
        "forest_fire",
        Within(300),
        "a fire burns out and the forest regrows until lightning or a reseed",
    ),
    row(
        "predator_prey",
        Within(4),
        "animals move, breed and reseed every tick",
    ),
    row("twinkle", Within(64), "sparse random twinkles"),
    row("ant", Within(8), "the ant flips a tile every step"),
    row("bounce", Within(8), "balls keep moving"),
    row("bubbles", Within(16), "bubbles rise and new ones spawn"),
    row("gravity", Within(16), "grains fall and new ones spawn"),
    row("boids", Within(8), "the flock keeps moving"),
    row("lava_lamp", Within(24), "blobs drift slowly"),
    row("orbit", Within(12), "bodies keep orbiting"),
    row("sand_ripples", Within(8), "dunes keep shifting"),
    row(
        "fractal_explorer",
        Within(64),
        "the view drifts and zooms between class changes",
    ),
    row(
        "maze_growth",
        Within(4),
        "carving and walkers move every tick",
    ),
    row("shapes", Within(4), "geometric pulses every tick"),
    row("ink", Within(32), "drops land on a 16-tick cadence"),
    row(
        "ising",
        Within(8),
        "spins keep flipping at the default temperature",
    ),
    row(
        "kuramoto",
        Within(12),
        "oscillators keep crossing their phase",
    ),
    row("lightning", Within(8), "bolts strike every few ticks"),
    row("raindrops", Within(4), "rain falls every tick"),
    row(
        "reaction_diffusion",
        Within(8),
        "scheduled splashes keep the pattern moving",
    ),
    row("rivers", Within(4), "rain falls every tick"),
    row("wave", Within(8), "impulses keep travelling"),
    row(
        "cracks",
        Within(8),
        "a tip moves or retires every tick; only a shatter's few-tick dissolve is quiet",
    ),
    row("coral", Within(48), "colonies grow, break off and regrow"),
    row("crystal_growth", Within(8), "clusters grow and reseed"),
    row("dla", Within(8), "walkers keep sticking"),
    row("physarum", Within(8), "agents keep moving"),
    row("vines", Within(32), "tendrils grow, branch and prune back"),
    row(
        "weave",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "polyrhythm",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "breaks",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "fills",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "clave",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "groove",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "euclid",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "ostinato",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "motif",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "canon",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "chords",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "contour",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "cadence",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "phrase",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
];

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
                Within(window) => {
                    let gap = longest_gap(&activates, ticks);
                    assert!(
                        gap <= window,
                        "{} salt {salt}: {gap} ticks without an Activate, expected at most {window} ({})",
                        row.id,
                        row.why
                    );
                }
                Silent => assert!(
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
