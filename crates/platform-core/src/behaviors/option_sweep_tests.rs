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

/// What the sweep saw: the option row (empty for the behavior as a whole) and a description.
struct Finding {
    key: String,
    text: String,
}

fn sweep_findings(id: &str) -> Vec<Finding> {
    let behavior = get_native_behavior(id).unwrap();
    let menu = behavior
        .config_menu(&behavior.init(Value::Null).unwrap())
        .unwrap()
        .unwrap_or_default();
    let defaults = menu_defaults(behavior, &menu);
    let play = PLAY_FAMILY.contains(&id);
    let mut findings = Vec::new();
    let mut find = |key: &str, text: String| {
        findings.push(Finding {
            key: key.into(),
            text: format!("{id}: {text}"),
        })
    };
    let base = Value::Object(defaults.clone());
    if !play && run(behavior, base.clone(), false) == run(behavior, base, true) {
        find("", "a press changes nothing".into());
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
                    find(key, format!("{key}={value} {label} stays empty"));
                } else if tail.iter().all(|frame| lit(frame) == TILES) {
                    find(key, format!("{key}={value} {label} saturates"));
                } else if !play && tail.iter().all(|frame| lit(frame) == 0) {
                    find(key, format!("{key}={value} {label} dies out"));
                }
            }
            traces.push((untouched, pressed));
        }
        for (index, value) in values.iter().enumerate().skip(1) {
            if traces[index] == traces[index - 1] {
                let previous = &values[index - 1];
                find(key, format!("{key}={previous} and {value} play the same"));
            }
        }
    }
    findings
}

/// Options the sweep flags on purpose, each with the reason. Every entry must still be flagged, so
/// a fix shows up here as a stale entry to delete.
const EXPECTED: &[(&str, &str, &str)] = &[
    (
        "sequencer",
        "pagesX",
        "pages only differ once they are drawn on",
    ),
    (
        "sequencer",
        "pagesY",
        "pages only differ once they are drawn on",
    ),
    (
        "sequencer",
        "pageX",
        "pages only differ once they are drawn on",
    ),
    (
        "sequencer",
        "pageY",
        "pages only differ once they are drawn on",
    ),
    (
        "sequencer",
        "advanceSteps",
        "pages only differ once they are drawn on",
    ),
    ("brain", "seedInterval", "0 is off: an unseeded brain rests"),
    (
        "brain",
        "randomSeedCells",
        "0 is off: an unseeded brain rests",
    ),
    ("twinkle", "birthChancePct", "0 is off: no star is born"),
    ("ink", "autoDropInterval", "0 is off: no drop falls"),
    (
        "reaction_diffusion",
        "killPct",
        "no kill lets chemical B take the grid",
    ),
    (
        "reaction_diffusion",
        "diffusionPct",
        "full diffusion spreads B everywhere",
    ),
    (
        "reaction_diffusion",
        "reactionPct",
        "a doubled reaction floods the grid",
    ),
    ("rivers", "rainPct", "0 is off: no rain, no river"),
    (
        "rivers",
        "evaporationPct",
        "full evaporation dries every stream",
    ),
    (
        "wave",
        "autoImpulseInterval",
        "0 is off: no impulse strikes",
    ),
    (
        "wave",
        "dampingPct",
        "no damping lets the waves fill the grid",
    ),
    ("ink", "fadePct", "no fade lets the pigment fill the grid"),
    (
        "crystal_growth",
        "growthChancePct",
        "certain growth fills the grid",
    ),
    ("bubbles", "spawnInterval", "0 is off: no bubble rises"),
    ("gravity", "spawnRatePct", "0 is off: no sand falls"),
    (
        "cracks",
        "stressPct",
        "0 is off: an unstressed pane stays clear",
    ),
    (
        "shapes",
        "lifespan",
        "the longest trails keep the grid lit, as they should",
    ),
    (
        "ant",
        "maxAnts",
        "a cap: only reached after many presses or minutes of spawning",
    ),
    (
        "bounce",
        "maxBalls",
        "a cap: only reached after many presses",
    ),
    (
        "bubbles",
        "maxBubbles",
        "a cap: only reached after many presses",
    ),
    (
        "life",
        "randomTickInterval",
        "only with Spawn Count above 0",
    ),
    (
        "life",
        "spawnStep",
        "an offset within the spawn interval: values a whole interval apart match",
    ),
    (
        "raindrops",
        "spawnStep",
        "an offset within the spawn interval: values a whole interval apart match",
    ),
    (
        "wave",
        "spawnStep",
        "an offset within the spawn interval: values a whole interval apart match",
    ),
    (
        "bubbles",
        "spawnStep",
        "an offset within the spawn interval: values a whole interval apart match",
    ),
    (
        "sand_ripples",
        "depositionPct",
        "open: crests rarely rise above the sand they ride on",
    ),
    (
        "crystal_growth",
        "cellLife",
        "open: crowding thins crystals before they age out",
    ),
    (
        "shapes",
        "maxRadius",
        "open: a radius past about 10 covers the whole grid",
    ),
    (
        "shapes",
        "pulseShape",
        "open: a growing filled disk lights the same new ring as a ring",
    ),
];

#[test]
fn every_build_option_changes_the_world_and_keeps_it_alive() {
    let mut unexpected = Vec::new();
    let mut seen = vec![false; EXPECTED.len()];
    for entry in behavior_catalog() {
        for finding in sweep_findings(entry.id) {
            match EXPECTED
                .iter()
                .position(|(id, key, _)| *id == entry.id && *key == finding.key)
            {
                Some(index) => seen[index] = true,
                None => unexpected.push(finding.text),
            }
        }
    }
    let stale = EXPECTED
        .iter()
        .zip(seen)
        .filter(|(_, seen)| !seen)
        .map(|((id, key, _), _)| format!("{id}.{key}"))
        .collect::<Vec<_>>();
    assert!(
        unexpected.is_empty(),
        "{}",
        unexpected.join(
            "
"
        )
    );
    assert!(stale.is_empty(), "no longer flagged: {}", stale.join(", "));
}
