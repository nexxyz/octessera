//! Every Build option at its minimum, default and maximum and halfway between: the tile frames of
//! a few hundred ticks, untouched and after one press, must change from value to value and, for
//! each of three random streams, must neither stay empty, nearly fill the grid nor freeze.
use super::{behavior_catalog, get_native_behavior, NativeBehavior};
use crate::behavior::{
    BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext, DeviceInput, TileTriggerType,
};
use crate::behavior_random::LayerRandom;
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use serde_json::{json, Map, Value};

const TICKS: usize = 300;
const PRESS_TICK: usize = 12;
const SEEDS: [u64; 3] = [7, 1, 2];
const TILES: usize = GRID_WIDTH * GRID_HEIGHT;
const SATURATED: usize = TILES - 5;
const FROZEN_BELOW: usize = 4;
const FAIR_SHARE: usize = 10;
const PLAY_FAMILY: &[&str] = &["none", "keys", "looper", "sequencer"];

#[derive(PartialEq)]
struct Frame {
    tiles: Vec<bool>,
    colors: Option<Vec<[u8; 3]>>,
    triggers: Option<Vec<TileTriggerType>>,
}

type Frames = Vec<Frame>;

fn frame(behavior: NativeBehavior, state: &super::NativeBehaviorState) -> Frame {
    let model = behavior.render_model(state).unwrap();
    Frame {
        tiles: model.tiles,
        colors: model.tile_colors,
        triggers: model.trigger_types,
    }
}

/// The frames of `TICKS` ticks, plus one at the press tick; with `press`, that frame shows a tap
/// on one tile while it is held.
fn run(behavior: NativeBehavior, config: Value, press: bool, seed: u64) -> Frames {
    let mut random = LayerRandom::new(Some(seed));
    random.scope(|| {
        let mut context = BehaviorContext::new(120.0);
        let mut state = behavior.init(config).unwrap();
        let mut frames = Vec::with_capacity(TICKS + 1);
        for tick in 0..TICKS {
            if tick == PRESS_TICK {
                if press {
                    let input = DeviceInput::GridPress { x: 3, y: 4 };
                    state = behavior.on_input(state, input, &mut context).unwrap();
                }
                frames.push(frame(behavior, &state));
                if press {
                    let input = DeviceInput::GridRelease { x: 3, y: 4 };
                    state = behavior.on_input(state, input, &mut context).unwrap();
                }
            }
            state = behavior.on_tick(state, &mut context).unwrap();
            frames.push(frame(behavior, &state));
        }
        frames
    })
}

fn lit(frame: &Frame) -> usize {
    frame.tiles.iter().filter(|tile| **tile).count()
}

fn distinct_frames(frames: &[Frame]) -> usize {
    let mut distinct: Vec<&Frame> = Vec::new();
    for frame in frames {
        if !distinct.contains(&frame) {
            distinct.push(frame);
        }
    }
    distinct.len()
}

/// Two runs differ when their lit counts or a fair share of their frames differ.
fn differ(a: &Frames, b: &Frames) -> bool {
    let total = |frames: &Frames| frames.iter().map(lit).sum::<usize>();
    let changed = a.iter().zip(b).filter(|(a, b)| a != b).count();
    total(a).abs_diff(total(b)) >= a.len() || changed * FAIR_SHARE >= a.len()
}

/// The values each option is run at: minimum, default and maximum, and the points halfway between.
fn sweep_values(item: &BehaviorConfigItem, default: Option<&Value>) -> Vec<Value> {
    let mut values = match item.item_type {
        BehaviorConfigItemType::Number => {
            let (min, max) = (item.min.unwrap() as i64, item.max.unwrap() as i64);
            let middle = default.and_then(Value::as_i64).unwrap_or((min + max) / 2);
            let step = item.step.unwrap_or(1).max(1) as i64;
            let between = |a: i64, b: i64| min + ((a + b) / 2 - min) / step * step;
            [min, between(min, middle), middle, between(middle, max), max]
                .map(|value| json!(value))
                .to_vec()
        }
        BehaviorConfigItemType::Bool => vec![json!(false), json!(true)],
        BehaviorConfigItemType::Enum => item
            .options
            .iter()
            .flatten()
            .map(|option| json!(option))
            .collect(),
        BehaviorConfigItemType::Action => Vec::new(),
    };
    values.dedup();
    values
}

fn config_with(defaults: &Map<String, Value>, key: &str, value: &Value) -> Value {
    let mut config = defaults.clone();
    config.insert(key.into(), value.clone());
    Value::Object(config)
}

fn menu_defaults(behavior: NativeBehavior, menu: &[BehaviorConfigItem]) -> Map<String, Value> {
    let state = behavior.init(Value::Null).unwrap();
    let saved = behavior.serialize(&state).unwrap();
    menu.iter()
        .filter_map(|item| Some((item.key.clone(), saved.get(&item.key)?.clone())))
        .collect()
}

/// What the sweep saw for one behavior, one line per finding.
fn sweep_findings(id: &str) -> Vec<String> {
    let behavior = get_native_behavior(id).unwrap();
    let menu = behavior
        .config_menu(&behavior.init(Value::Null).unwrap())
        .unwrap()
        .unwrap_or_default();
    let defaults = menu_defaults(behavior, &menu);
    let play = PLAY_FAMILY.contains(&id);
    let mut findings = Vec::new();
    let mut find = |text: String| {
        let text = format!("{id}: {text}");
        if !findings.contains(&text) {
            findings.push(text);
        }
    };
    let base = Value::Object(defaults.clone());
    if !play && run(behavior, base.clone(), false, SEEDS[0]) == run(behavior, base, true, SEEDS[0])
    {
        find("a press changes nothing".into());
    }
    for item in &menu {
        let key = item.key.as_str();
        let values = sweep_values(item, defaults.get(key));
        let mut traces = Vec::new();
        for value in &values {
            let config = config_with(&defaults, key, value);
            for seed in SEEDS {
                let untouched = run(behavior, config.clone(), false, seed);
                let pressed = run(behavior, config.clone(), true, seed);
                for (label, frames) in [("untouched", &untouched), ("pressed", &pressed)] {
                    if play && label == "untouched" {
                        continue;
                    }
                    let tail = &frames[frames.len() / 2..];
                    if frames.iter().all(|frame| lit(frame) == 0) {
                        find(format!("{key}={value} {label} stays empty"));
                    } else if tail.iter().all(|frame| lit(frame) >= SATURATED) {
                        find(format!("{key}={value} {label} saturates"));
                    } else if !play && tail.iter().all(|frame| lit(frame) == 0) {
                        find(format!("{key}={value} {label} dies out"));
                    } else if !play && distinct_frames(tail) < FROZEN_BELOW {
                        find(format!("{key}={value} {label} freezes"));
                    }
                }
                if seed == SEEDS[0] {
                    traces.push((value, untouched, pressed));
                }
            }
        }
        for pair in traces.windows(2) {
            let [(previous, previous_untouched, previous_pressed), (value, untouched, pressed)] =
                pair
            else {
                unreachable!()
            };
            let same = |a: &Frames, b: &Frames| match play {
                true => a == b,
                false => !differ(a, b),
            };
            if same(untouched, previous_untouched) && same(pressed, previous_pressed) {
                find(format!("{key}={previous} and {value} play the same"));
            }
        }
    }
    findings
}

const OFF: &str = "0 is off: the world was told to rest";
const EXTREME: &str = "a physical extreme: with no fade or damping the grid fills";
const CAP: &str = "a cap: only reached after many presses or minutes of spawning";
const OFFSET: &str = "an offset within the spawn interval: values a whole interval apart match";
const NO_FADE: &str = "0 is off: nothing fades, so the lit tiles stand still";
const CALM: &str = "0 is off: with no wind or no erosion the dunes rest";
const FOREVER: &str = "0 is forever: crystals never dissolve, so they fill the grid";
const HELD_ZOOM: &str = "0 holds the zoom: Drift alone sways between two views";
const ROW_ACCENT: &str =
    "changes only the trigger a cleared row sends; few rows clear in 300 ticks";
const SPAWN_COUNT: &str = "only with Spawn Count above 0";
const PRESS_NOOP: &str = "the tapped tile already holds a block, paddle or crystal at that moment";
const DEAD_ZONE: &str = "a known dead zone: neighboring values play alike, left for a later pass";
const UNDRAWN: &str = "sequencer pages only differ once they are drawn on";
const LIFE_AT_REST: &str =
    "with no spawning, Life settles into still lifes and blinkers or dies out, and rests until pressed";

/// Every finding the sweep makes on purpose, word for word, with its reason. A new finding fails,
/// even for an option listed here, and so does an entry that is no longer found, so a fix shows
/// up as a stale entry to delete.
const EXPECTED: &[(&str, &str)] = &[
    ("brain: seedInterval=0 untouched stays empty", OFF),
    ("brain: seedInterval=0 pressed dies out", OFF),
    ("brain: randomSeedCells=0 untouched stays empty", OFF),
    ("brain: randomSeedCells=0 pressed dies out", OFF),
    ("life: randomCellsPerTick=0 pressed dies out", LIFE_AT_REST),
    ("life: randomTickInterval=1 pressed dies out", LIFE_AT_REST),
    ("life: randomTickInterval=20 pressed dies out", LIFE_AT_REST),
    ("life: gliderSpawnInterval=0 pressed dies out", LIFE_AT_REST),
    ("life: spawnStep=0 pressed dies out", LIFE_AT_REST),
    ("life: spawnStep=63 pressed dies out", LIFE_AT_REST),
    ("twinkle: birthChancePct=0 untouched dies out", OFF),
    ("twinkle: birthChancePct=0 pressed dies out", OFF),
    ("ink: fadePct=0 untouched saturates", EXTREME),
    ("ink: fadePct=0 pressed saturates", EXTREME),
    ("ink: autoDropInterval=0 untouched dies out", OFF),
    ("ink: autoDropInterval=0 pressed dies out", OFF),
    ("rivers: rainPct=0 untouched dies out", OFF),
    ("rivers: rainPct=0 pressed dies out", OFF),
    ("wave: dampingPct=0 untouched saturates", EXTREME),
    ("wave: dampingPct=0 pressed saturates", EXTREME),
    ("wave: autoImpulseInterval=0 untouched dies out", OFF),
    ("wave: autoImpulseInterval=0 pressed dies out", OFF),
    ("cracks: stressPct=0 untouched stays empty", OFF),
    ("bubbles: spawnInterval=0 untouched stays empty", OFF),
    ("bubbles: spawnInterval=0 pressed dies out", OFF),
    ("gravity: spawnRatePct=0 untouched stays empty", OFF),
    ("gravity: spawnRatePct=0 pressed dies out", OFF),
    ("forest_fire: growChancePct=0 untouched freezes", OFF),
    ("forest_fire: growChancePct=0 pressed freezes", OFF),
    ("forest_fire: spreadChancePct=0 untouched saturates", OFF),
    (
        "forest_fire: lightningChancePerThousand=0 untouched saturates",
        OFF,
    ),
    (
        "forest_fire: lightningChancePerThousand=0 pressed saturates",
        OFF,
    ),
    ("life: randomCellsPerTick=0 untouched freezes", LIFE_AT_REST),
    ("life: randomTickInterval=1 untouched freezes", LIFE_AT_REST),
    (
        "life: randomTickInterval=10 untouched freezes",
        LIFE_AT_REST,
    ),
    ("life: randomTickInterval=10 pressed dies out", LIFE_AT_REST),
    (
        "life: randomTickInterval=20 untouched freezes",
        LIFE_AT_REST,
    ),
    (
        "life: gliderSpawnInterval=0 untouched freezes",
        LIFE_AT_REST,
    ),
    ("life: spawnStep=0 untouched freezes", LIFE_AT_REST),
    ("life: spawnStep=31 untouched freezes", LIFE_AT_REST),
    ("life: spawnStep=31 pressed dies out", LIFE_AT_REST),
    ("life: spawnStep=63 untouched freezes", LIFE_AT_REST),
    ("twinkle: fadeChancePct=0 untouched freezes", NO_FADE),
    ("twinkle: fadeChancePct=0 pressed freezes", NO_FADE),
    (
        "fractal_explorer: zoomRatePct=0 untouched freezes",
        HELD_ZOOM,
    ),
    ("fractal_explorer: zoomRatePct=0 pressed freezes", HELD_ZOOM),
    ("maze_growth: carvePct=0 pressed freezes", OFF),
    ("coral: growthPct=0 untouched freezes", OFF),
    ("coral: growthPct=0 pressed freezes", OFF),
    ("crystal_growth: cellLife=0 untouched saturates", FOREVER),
    ("crystal_growth: cellLife=0 pressed saturates", FOREVER),
    ("vines: growthPct=0 untouched freezes", OFF),
    ("vines: growthPct=0 pressed freezes", OFF),
    ("sand_ripples: windStrengthPct=0 untouched freezes", CALM),
    ("sand_ripples: windStrengthPct=0 pressed freezes", CALM),
    ("sand_ripples: erosionPct=0 untouched freezes", CALM),
    ("sand_ripples: erosionPct=0 pressed freezes", CALM),
    (
        "blockout: rowSpawn=\"stable\" and \"new\" play the same",
        ROW_ACCENT,
    ),
    ("sequencer: pagesX=1 and 2 play the same", UNDRAWN),
    ("sequencer: pagesX=2 and 3 play the same", UNDRAWN),
    ("sequencer: pagesX=3 and 4 play the same", UNDRAWN),
    ("sequencer: pagesY=1 and 2 play the same", UNDRAWN),
    ("sequencer: pagesY=2 and 3 play the same", UNDRAWN),
    ("sequencer: pagesY=3 and 4 play the same", UNDRAWN),
    ("sequencer: pageX=1 and 2 play the same", UNDRAWN),
    ("sequencer: pageX=2 and 3 play the same", UNDRAWN),
    ("sequencer: pageX=3 and 4 play the same", UNDRAWN),
    ("sequencer: pageY=1 and 2 play the same", UNDRAWN),
    ("sequencer: pageY=2 and 3 play the same", UNDRAWN),
    ("sequencer: pageY=3 and 4 play the same", UNDRAWN),
    ("sequencer: advanceSteps=0 and 16 play the same", UNDRAWN),
    ("sequencer: advanceSteps=16 and 32 play the same", UNDRAWN),
    ("sequencer: advanceSteps=32 and 48 play the same", UNDRAWN),
    ("sequencer: advanceSteps=48 and 64 play the same", UNDRAWN),
    ("breaks: densityPct=20 and 31 play the same", DEAD_ZONE),
    ("clave: densityPct=10 and 15 play the same", DEAD_ZONE),
    ("contour: densityPct=20 and 32 play the same", DEAD_ZONE),
    ("contour: densityPct=55 and 80 play the same", DEAD_ZONE),
    ("cadence: variationPct=0 and 20 play the same", DEAD_ZONE),
    ("ant: maxAnts=25 and 50 play the same", CAP),
    ("ant: maxAnts=50 and 75 play the same", CAP),
    ("ant: maxAnts=75 and 100 play the same", CAP),
    ("brain: fireThreshold=3 and 4 play the same", DEAD_ZONE),
    ("brain: spawnStep=31 and 63 play the same", OFFSET),
    ("forest_fire: spreadChancePct=0 pressed saturates", OFF),
    (
        "forest_fire: reseedThresholdPct=2 and 5 play the same",
        DEAD_ZONE,
    ),
    (
        "forest_fire: reseedThresholdPct=52 and 100 play the same",
        DEAD_ZONE,
    ),
    (
        "life: randomTickInterval=1 and 10 play the same",
        SPAWN_COUNT,
    ),
    (
        "life: randomTickInterval=10 and 20 play the same",
        SPAWN_COUNT,
    ),
    ("life: spawnStep=0 and 31 play the same", OFFSET),
    ("life: spawnStep=31 and 63 play the same", OFFSET),
    ("reaction_diffusion: seedInterval=0 untouched dies out", OFF),
    ("reaction_diffusion: seedInterval=0 pressed dies out", OFF),
    ("cracks: stressPct=0 pressed freezes", OFF),
    (
        "cracks: shatterThreshold=44 and 64 play the same",
        DEAD_ZONE,
    ),
    ("crystal_growth: a press changes nothing", PRESS_NOOP),
    ("crystal_growth: seedStep=31 and 63 play the same", OFFSET),
    ("dla: spawnStep=31 and 63 play the same", OFFSET),
    ("bounce: maxBalls=30 and 60 play the same", CAP),
    ("bounce: maxBalls=60 and 80 play the same", CAP),
    ("bounce: maxBalls=80 and 100 play the same", CAP),
    ("bubbles: maxBubbles=12 and 24 play the same", CAP),
    ("bubbles: maxBubbles=24 and 44 play the same", CAP),
    ("bubbles: maxBubbles=44 and 64 play the same", CAP),
    ("blockout: a press changes nothing", PRESS_NOOP),
    ("blockout: cpuSkillPct=80 and 90 play the same", DEAD_ZONE),
    ("pong: a press changes nothing", PRESS_NOOP),
];

#[test]
fn every_build_option_changes_the_world_and_keeps_it_alive() {
    let mut unexpected = Vec::new();
    let mut seen = vec![false; EXPECTED.len()];
    for entry in behavior_catalog() {
        for finding in sweep_findings(entry.id) {
            match EXPECTED.iter().position(|(text, _)| *text == finding) {
                Some(index) => seen[index] = true,
                None => unexpected.push(finding),
            }
        }
    }
    let stale = EXPECTED
        .iter()
        .zip(seen)
        .filter(|(_, seen)| !seen)
        .map(|((text, _), _)| *text)
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "{}",
        unexpected.join(
            "
"
        )
    );
    assert!(
        stale.is_empty(),
        "no longer found:
{}",
        stale.join(
            "
"
        )
    );
}
