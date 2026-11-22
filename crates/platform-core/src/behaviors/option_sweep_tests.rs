//! Every Build option at its minimum, default and maximum: the tile frames of a few hundred ticks,
//! untouched and after one press, must change from value to value and must neither stay empty nor
//! fill the grid.
use super::{behavior_catalog, get_native_behavior, NativeBehavior};
use crate::behavior::{
    BehaviorConfigItem, BehaviorConfigItemType, BehaviorContext, DeviceInput, TileTriggerType,
};
use crate::behavior_random::LayerRandom;
use crate::grid::{GRID_HEIGHT, GRID_WIDTH};
use serde_json::{json, Map, Value};

const TICKS: usize = 300;
const PRESS_TICK: usize = 12;
const TILES: usize = GRID_WIDTH * GRID_HEIGHT;
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

/// The frames of `TICKS` ticks; with `press`, a tap on one tile early on, drawn while held.
fn run(behavior: NativeBehavior, config: Value, press: bool) -> Frames {
    let mut random = LayerRandom::new(Some(7));
    random.scope(|| {
        let mut context = BehaviorContext::new(120.0);
        let mut state = behavior.init(config).unwrap();
        let mut frames = Vec::with_capacity(TICKS + 1);
        for tick in 0..TICKS {
            if press && tick == PRESS_TICK {
                let input = DeviceInput::GridPress { x: 3, y: 4 };
                state = behavior.on_input(state, input, &mut context).unwrap();
                frames.push(frame(behavior, &state));
                let input = DeviceInput::GridRelease { x: 3, y: 4 };
                state = behavior.on_input(state, input, &mut context).unwrap();
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

fn sweep_values(item: &BehaviorConfigItem, default: Option<&Value>) -> Vec<Value> {
    let mut values = match item.item_type {
        BehaviorConfigItemType::Number => {
            let mut values = vec![json!(item.min.unwrap())];
            values.extend(default.cloned());
            values.push(json!(item.max.unwrap()));
            values
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
    let mut find = |text: String| findings.push(format!("{id}: {text}"));
    let base = Value::Object(defaults.clone());
    if !play && run(behavior, base.clone(), false) == run(behavior, base, true) {
        find("a press changes nothing".into());
    }
    for item in &menu {
        let key = item.key.as_str();
        let values = sweep_values(item, defaults.get(key));
        let mut traces = Vec::new();
        for value in &values {
            let config = config_with(&defaults, key, value);
            let untouched = run(behavior, config.clone(), false);
            let pressed = run(behavior, config, true);
            for (label, frames) in [("untouched", &untouched), ("pressed", &pressed)] {
                if play && label == "untouched" {
                    continue;
                }
                let tail = &frames[frames.len() / 2..];
                if frames.iter().all(|frame| lit(frame) == 0) {
                    find(format!("{key}={value} {label} stays empty"));
                } else if tail.iter().all(|frame| lit(frame) == TILES) {
                    find(format!("{key}={value} {label} saturates"));
                } else if !play && tail.iter().all(|frame| lit(frame) == 0) {
                    find(format!("{key}={value} {label} dies out"));
                }
            }
            traces.push((untouched, pressed));
        }
        for (index, value) in values.iter().enumerate().skip(1) {
            if traces[index] == traces[index - 1] {
                let previous = &values[index - 1];
                find(format!("{key}={previous} and {value} play the same"));
            }
        }
    }
    findings
}

const OFF: &str = "0 is off: the world was told to rest";
const EXTREME: &str =
    "a physical extreme: no fade, damping or kill fills the grid, full evaporation dries it";
const CAP: &str = "a cap: only reached after many presses or minutes of spawning";
const OFFSET: &str = "an offset within the spawn interval: values a whole interval apart match";
const UNDRAWN: &str = "sequencer pages only differ once they are drawn on";
const LIFE_AT_REST: &str =
    "the press breaks the blinker; with no spawning, a world that dies out rests until pressed";

/// Every finding the sweep makes on purpose, word for word, with its reason. A new finding fails,
/// even for an option listed here, and so does an entry that is no longer found, so a fix shows
/// up as a stale entry to delete.
const EXPECTED: &[(&str, &str)] = &[
    ("sequencer: pagesX=1 and 4 play the same", UNDRAWN),
    ("sequencer: pagesY=1 and 4 play the same", UNDRAWN),
    ("sequencer: pageX=1 and 4 play the same", UNDRAWN),
    ("sequencer: pageY=1 and 4 play the same", UNDRAWN),
    ("sequencer: advanceSteps=0 and 64 play the same", UNDRAWN),
    ("ant: maxAnts=50 and 100 play the same", CAP),
    ("brain: seedInterval=0 untouched stays empty", OFF),
    ("brain: seedInterval=0 pressed dies out", OFF),
    ("brain: randomSeedCells=0 untouched stays empty", OFF),
    ("brain: randomSeedCells=0 pressed dies out", OFF),
    (
        "life: randomTickInterval=1 and 20 play the same",
        "only with Spawn Count above 0",
    ),
    ("life: spawnStep=0 and 63 play the same", OFFSET),
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
    ("raindrops: spawnStep=0 and 63 play the same", OFFSET),
    ("reaction_diffusion: killPct=0 untouched saturates", EXTREME),
    ("reaction_diffusion: killPct=0 pressed saturates", EXTREME),
    (
        "reaction_diffusion: diffusionPct=100 untouched saturates",
        EXTREME,
    ),
    (
        "reaction_diffusion: diffusionPct=100 pressed saturates",
        EXTREME,
    ),
    (
        "reaction_diffusion: reactionPct=100 untouched saturates",
        EXTREME,
    ),
    (
        "reaction_diffusion: reactionPct=100 pressed saturates",
        EXTREME,
    ),
    ("rivers: rainPct=0 untouched dies out", OFF),
    ("rivers: rainPct=0 pressed dies out", OFF),
    ("rivers: evaporationPct=100 untouched stays empty", EXTREME),
    ("rivers: evaporationPct=100 pressed dies out", EXTREME),
    ("wave: dampingPct=0 untouched saturates", EXTREME),
    ("wave: dampingPct=0 pressed saturates", EXTREME),
    ("wave: autoImpulseInterval=0 untouched dies out", OFF),
    ("wave: autoImpulseInterval=0 pressed dies out", OFF),
    ("wave: spawnStep=3 and 63 play the same", OFFSET),
    // Open. Now: a filled disk lights only the ring it newly reaches, the same tiles as `ring`.
    // After (proposal): light the whole disk while it grows, with a smaller default Max Radius so
    // it does not flood the grid.
    (
        "shapes: pulseShape=\"ring\" and \"filled\" play the same",
        "open: filled plays as ring",
    ),
    (
        "shapes: lifespan=12 untouched saturates",
        "the longest trails keep the grid lit",
    ),
    (
        "shapes: lifespan=12 pressed saturates",
        "the longest trails keep the grid lit",
    ),
    // Open. Now: any radius past about 10 already covers the grid, so 12 to 32 play alike.
    // After (proposal): narrow Max Radius to 4..12, normalizing the board stores first.
    (
        "shapes: maxRadius=12 and 32 play the same",
        "open: radii past the grid",
    ),
    ("cracks: stressPct=0 untouched stays empty", OFF),
    (
        "crystal_growth: growthChancePct=100 untouched saturates",
        EXTREME,
    ),
    (
        "crystal_growth: growthChancePct=100 pressed saturates",
        EXTREME,
    ),
    // Open. Now: crowding thins crystals before any cell ages out, so Cell Life never acts.
    // After (proposal): thin only once the grid is nearly full, so old crystals age out first.
    (
        "crystal_growth: cellLife=0 and 128 play the same",
        "open: crowding thins first",
    ),
    (
        "crystal_growth: cellLife=128 and 256 play the same",
        "open: crowding thins first",
    ),
    ("bounce: maxBalls=60 and 100 play the same", CAP),
    ("bubbles: spawnInterval=0 untouched stays empty", OFF),
    ("bubbles: spawnInterval=0 pressed dies out", OFF),
    ("bubbles: spawnStep=0 and 63 play the same", OFFSET),
    ("bubbles: maxBubbles=24 and 64 play the same", CAP),
    ("gravity: spawnRatePct=0 untouched stays empty", OFF),
    ("gravity: spawnRatePct=0 pressed dies out", OFF),
    // Open. Now: a deposit adds half a moved grain to a crest that rarely rises above the sand it
    // rides on, so Deposition never shows. After (proposal): a deposit raises the crest to the
    // sand height it carried, so high Deposition leaves standing ridges.
    (
        "sand_ripples: depositionPct=0 and 35 play the same",
        "open: crests stay under the sand",
    ),
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
