use super::*;
use crate::native_menu::{NativeMenuItem, NativeMenuValue};

const PULSES: u32 = 480;
const PREFIX: &str = "layers.0.link.";

/// The setup an option needs before it can change anything, so each sweep compares like with
/// like: scan rows sweep on a scanning layer, arp rows with the arp on, lane rows with their lane
/// on, probability rows on a custom map, and every other row on an event layer whose Hold
/// trigger plays.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Context {
    Events,
    Probability,
    Scan,
    RowScan,
    Lanes,
    Arp,
}

const CONTEXTS: [Context; 6] = [
    Context::Events,
    Context::Probability,
    Context::Scan,
    Context::RowScan,
    Context::Lanes,
    Context::Arp,
];

fn context_for(key: &str) -> Context {
    let field = &key[PREFIX.len()..];
    if field.starts_with("arp.") && field != "arp.mode" {
        Context::Arp
    } else if field == "x.pitch.restartEachSection" {
        Context::RowScan
    } else if field.starts_with("triggerProbability") || field == "seeded" {
        Context::Probability
    } else if field.starts_with("scan")
        || field.starts_with("mapping.scanned")
        || field.ends_with("restartEachSection")
    {
        Context::Scan
    } else if [".velocity.", ".filterCutoff.", ".filterResonance."]
        .iter()
        .any(|lane| field.contains(lane))
        && !field.ends_with(".enabled")
    {
        Context::Lanes
    } else {
        Context::Events
    }
}

fn set(runner: &mut NativeRunner, key: &str, value: &SweepValue) {
    let changed = match value {
        SweepValue::Number(value) => runner.menu.set_number_value_for_key(key, *value),
        SweepValue::Enum(value) => runner.menu.set_enum_value_for_key(key, value),
        SweepValue::Bool(value) => runner.menu.set_bool_value_for_key(key, *value),
    };
    if changed {
        runner.apply_or_schedule_menu_key(key).unwrap();
    }
}

fn runner_in(context: Context) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "life".into(),
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    for (x, y) in [
        (1, 2),
        (2, 1),
        (0, 0),
        (1, 0),
        (2, 0),
        (5, 2),
        (5, 1),
        (5, 0),
    ] {
        grid_tap(&mut runner, x, y);
    }
    let link = |field: &str| format!("{PREFIX}{field}");
    set(&mut runner, &link("x.pitch.steps"), &SweepValue::Number(1));
    set(&mut runner, &link("pitch.outOfRange"), &enumerated("wrap"));
    set(
        &mut runner,
        &link("mapping.stable.action"),
        &enumerated("note_on"),
    );
    match context {
        Context::Events => {}
        Context::Probability => {
            set(
                &mut runner,
                &link("triggerProbabilityMode"),
                &enumerated("custom"),
            );
            for (index, tile) in runner.layers[0]
                .trigger_probability_map
                .iter_mut()
                .enumerate()
            {
                *tile = [TileProbability::Low, TileProbability::High][index % 2];
            }
        }
        Context::Scan | Context::RowScan => {
            set(&mut runner, &link("scanMode"), &enumerated("scanning"));
            set(&mut runner, &link("scanSections"), &enumerated("2"));
            set(
                &mut runner,
                &link("mapping.scanned_empty.action"),
                &enumerated("note_on"),
            );
            if context == Context::RowScan {
                set(&mut runner, &link("scanAxis"), &enumerated("rows"));
            }
        }
        Context::Lanes => {
            for lane in ["velocity", "filterCutoff", "filterResonance"] {
                for axis in ["x", "y"] {
                    set(
                        &mut runner,
                        &link(&format!("{axis}.{lane}.enabled")),
                        &SweepValue::Bool(true),
                    );
                }
            }
        }
        Context::Arp => set(&mut runner, &link("arp.mode"), &enumerated("octave_spread")),
    }
    runner
}

fn enumerated(value: &str) -> SweepValue {
    SweepValue::Enum(value.into())
}

#[derive(Clone, Debug, PartialEq)]
enum SweepValue {
    Number(i32),
    Enum(String),
    Bool(bool),
}

/// The values a row is tried at and the value it starts from: numbers at both ends, every enum
/// option, and a flipped bool.
fn sweep_values(item: &NativeMenuItem) -> Option<(Vec<SweepValue>, SweepValue)> {
    match &item.value {
        NativeMenuValue::Number {
            value, min, max, ..
        } => Some((
            vec![SweepValue::Number(*min), SweepValue::Number(*max)],
            SweepValue::Number(*value),
        )),
        NativeMenuValue::Enum { options, selected } if options.len() > 1 => Some((
            options.iter().cloned().map(SweepValue::Enum).collect(),
            SweepValue::Enum(options[*selected].clone()),
        )),
        NativeMenuValue::Bool { value } => {
            Some((vec![SweepValue::Bool(!value)], SweepValue::Bool(*value)))
        }
        _ => None,
    }
}

/// The Link rows with a value, once per key: leaves, and parents such as `Set` whose folders pick
/// the parent's value.
fn link_rows(item: &NativeMenuItem, out: &mut Vec<NativeMenuItem>) {
    if item
        .key
        .as_deref()
        .is_some_and(|key| key.starts_with(PREFIX))
        && (item.children.is_empty() || matches!(item.value, NativeMenuValue::Enum { .. }))
        && !out.iter().any(|row| row.key == item.key)
    {
        out.push(item.clone());
    }
    for child in &item.children {
        link_rows(child, out);
    }
}

/// Every note, tile hit and CC the layer plays over `PULSES` pulses, one line per sounding pulse.
fn played(mut runner: NativeRunner) -> Vec<String> {
    runner.transport.transport = RuntimeTransportState::Playing;
    (1..=PULSES)
        .filter_map(|pulse| {
            let messages = runner
                .send(HostMessage::TransportPulseStep {
                    pulses: 1,
                    source: SyncSource::Internal,
                    at_ppqn_pulse: None,
                    request_snapshot: Some(false),
                })
                .unwrap();
            let events = routed_events(&messages, false);
            (!events.is_empty()).then(|| format!("{pulse} {events:?}"))
        })
        .collect()
}

fn note_ons(played: &[String]) -> usize {
    played
        .iter()
        .map(|step| step.matches("NoteOn").count())
        .sum()
}

/// The rows whose minimum, default and maximum all play exactly what the default plays.
fn dead_rows(context: Context) -> Vec<String> {
    let base = runner_in(context);
    let steps = PULSES / base.step_pulses_for_layer(0);
    let mut rows = Vec::new();
    link_rows(&base.menu.root, &mut rows);
    let baseline = played(runner_in(context));
    assert!(note_ons(&baseline) > 0, "{context:?} plays nothing");
    assert!(
        note_ons(&baseline) < steps as usize * GRID_WIDTH * GRID_HEIGHT,
        "{context:?} plays every tile on every step"
    );
    let mut dead = Vec::new();
    for row in rows {
        let key = row.key.clone().unwrap();
        if context_for(&key) != context {
            continue;
        }
        let Some((values, start)) = sweep_values(&row) else {
            continue;
        };
        for value in values.iter().filter(|value| **value != start) {
            let mut runner = runner_in(context);
            set(&mut runner, &key, value);
            if played(runner) == baseline {
                dead.push(format!("{key} = {value:?}"));
            }
        }
    }
    dead
}

fn assert_every_row_changes_what_plays(context: Context) {
    let dead = dead_rows(context);
    assert!(dead.is_empty(), "{context:?}: {dead:#?}");
}

fn saved_keys(prefix: &str, value: &Value, out: &mut std::collections::BTreeSet<String>) {
    match value {
        Value::Object(fields) => {
            for (field, value) in fields {
                saved_keys(&format!("{prefix}{field}."), value, out);
            }
        }
        _ => {
            out.insert(prefix.trim_end_matches('.').to_string());
        }
    }
}

#[test]
pub(crate) fn every_saved_link_key_is_swept_in_some_context() {
    let runner = runner_in(Context::Events);
    let mut unswept = std::collections::BTreeSet::new();
    saved_keys(
        PREFIX,
        &crate::native_runner::link_payload::link_layer_payload(
            &runner.layers[0].link,
            &runner.layers[0].trigger_probability_map,
        ),
        &mut unswept,
    );
    // The painted grid is not a row; the Probability context paints it.
    unswept.remove(&format!("{PREFIX}triggerProbabilityMap"));
    for context in CONTEXTS {
        let mut rows = Vec::new();
        link_rows(&runner_in(context).menu.root, &mut rows);
        for row in rows {
            let key = row.key.clone().unwrap();
            if context_for(&key) == context && sweep_values(&row).is_some() {
                unswept.remove(&key);
            }
        }
    }
    assert!(unswept.is_empty(), "{unswept:#?}");
}

#[test]
pub(crate) fn every_event_row_changes_what_a_fixed_tile_sequence_plays() {
    assert_every_row_changes_what_plays(Context::Events);
}

#[test]
pub(crate) fn every_probability_row_changes_what_a_fixed_tile_sequence_plays() {
    assert_every_row_changes_what_plays(Context::Probability);
}

#[test]
pub(crate) fn every_scan_row_changes_what_a_fixed_tile_sequence_plays() {
    assert_every_row_changes_what_plays(Context::Scan);
}

#[test]
pub(crate) fn every_row_scan_row_changes_what_a_fixed_tile_sequence_plays() {
    assert_every_row_changes_what_plays(Context::RowScan);
}

#[test]
pub(crate) fn every_lane_row_changes_what_a_fixed_tile_sequence_plays() {
    assert_every_row_changes_what_plays(Context::Lanes);
}

#[test]
pub(crate) fn every_arp_row_changes_what_a_fixed_tile_sequence_plays() {
    assert_every_row_changes_what_plays(Context::Arp);
}
