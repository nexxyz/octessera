use super::*;
use realtime_engine::synth::{FxBusSlotConfig, SynthProfileSnapshot};
use rodio_engine_source::{EngineEvent, EngineSource};
use std::sync::mpsc::sync_channel;
use std::time::Instant;

const CONTROL_QUEUE_CAPACITY: usize = 512;

#[test]
fn parser_accepts_canonical_positive_units_only_within_capacity() {
    let max = max_units();
    assert_eq!(parse(&format!("capacity_analogue_{max}")), Some(max));
    assert_eq!(parse("capacity_analogue_1"), Some(1));
    for name in [
        "capacity_analogue_0",
        "capacity_analogue_01",
        "capacity_analogue_",
        "capacity_analogue_1x",
        "capacity_analogue_18446744073709551616",
    ] {
        assert_eq!(parse(name), None, "accepted invalid scenario {name}");
    }
    assert_eq!(parse(&format!("capacity_analogue_{}", max + 1)), None);
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[test]
fn topology_and_duck_sources_are_exact_at_representative_points() {
    for (units, expected_bus_kinds, expected_ducks) in [
        (
            8,
            vec![vec!["delay", "duck"], vec!["duck", "saturator"]],
            vec![
                (0, 1, "I2", "sampler", "direct"),
                (1, 0, "I1", "synth", "fx_bus_1"),
            ],
        ),
        (
            9,
            vec![
                vec!["delay", "duck"],
                vec!["duck", "saturator"],
                vec!["delay"],
                vec![],
            ],
            vec![
                (0, 1, "I2", "sampler", "direct"),
                (1, 0, "I1", "synth", "fx_bus_1"),
            ],
        ),
        (
            11,
            vec![
                vec!["delay", "duck"],
                vec!["duck", "saturator"],
                vec!["delay", "duck"],
                vec![],
            ],
            vec![
                (0, 1, "I2", "sampler", "direct"),
                (1, 0, "I1", "synth", "fx_bus_1"),
                (2, 1, "I6", "sampler", "direct"),
            ],
        ),
        (
            13,
            vec![
                vec!["delay", "duck"],
                vec!["duck", "saturator"],
                vec!["delay", "duck"],
                vec!["duck"],
            ],
            vec![
                (0, 1, "I2", "sampler", "direct"),
                (1, 0, "I1", "synth", "fx_bus_1"),
                (2, 1, "I6", "sampler", "direct"),
                (3, 0, "I5", "synth", "fx_bus_3"),
            ],
        ),
        (
            16,
            vec![
                vec!["delay", "duck"],
                vec!["duck", "saturator"],
                vec!["delay", "duck"],
                vec!["duck", "saturator"],
            ],
            vec![
                (0, 1, "I2", "sampler", "direct"),
                (1, 0, "I1", "synth", "fx_bus_1"),
                (2, 1, "I6", "sampler", "direct"),
                (3, 0, "I5", "synth", "fx_bus_3"),
            ],
        ),
        (
            24,
            vec![
                vec!["delay", "duck", "reverb"],
                vec!["duck", "saturator", "chorus"],
                vec!["delay", "duck", "filter_lfo"],
                vec!["duck", "saturator", "eq"],
            ],
            vec![
                (0, 1, "I2", "sampler", "direct"),
                (1, 0, "I1", "synth", "fx_bus_1"),
                (2, 1, "I6", "sampler", "direct"),
                (3, 0, "I5", "synth", "fx_bus_3"),
            ],
        ),
    ] {
        let config = instruments(units);
        assert_eq!(
            config
                .instruments
                .iter()
                .map(|slot| slot.kind.as_str())
                .collect::<Vec<_>>(),
            if units <= 8 {
                vec![
                    "synth", "sampler", "synth", "synth", "none", "none", "none", "none",
                ]
            } else {
                vec![
                    "synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth",
                ]
            }
        );
        assert_eq!(
            config
                .instruments
                .iter()
                .map(|slot| slot.mixer.as_ref().unwrap().route.as_str())
                .collect::<Vec<_>>(),
            if units <= 8 {
                vec![
                    "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "direct", "direct", "direct",
                    "direct",
                ]
            } else {
                vec![
                    "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "direct", "fx_bus_3",
                    "fx_bus_4",
                ]
            }
        );
        let mixer = config.mixer.as_ref().unwrap();
        assert_eq!(
            mixer
                .buses
                .iter()
                .map(|bus| bus.slots.iter().map(slot_kind).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            expected_bus_kinds
        );
        assert_eq!(duck_sources(&config), expected_ducks);
        assert_group_component(&config, 0, 0);
        if units > 8 && bus_fx_count(units) >= 7 {
            assert_group_component(&config, 4, 2);
        }
    }
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256"
))]
#[test]
fn representative_capacity_points_apply_through_256_frame_drains() {
    for (units, expected) in [
        (9, expected_state(27, 9, 5, 2)),
        (11, expected_state(33, 11, 6, 2)),
        (13, expected_state(39, 13, 7, 2)),
    ] {
        assert_applied(units, expected);
    }
}

#[cfg(feature = "benchmark-voice-pools-128")]
#[test]
fn maximum_analogue_capacity_applies_with_128_voice_pools() {
    assert_eq!(max_units(), 42);
    assert_applied(42, expected_state(126, 42, 12, 2));
}

#[cfg(feature = "benchmark-voice-pools-256")]
#[test]
fn maximum_analogue_capacity_applies_with_256_voice_pools() {
    assert_eq!(max_units(), 85);
    assert_applied(85, expected_state(255, 85, 12, 2));
}

fn expected_state(
    active_synth_voices: usize,
    active_sample_voices: usize,
    active_bus_fx_slots: usize,
    active_global_fx_slots: usize,
) -> ExpectedLiveState {
    ExpectedLiveState {
        active_synth_voices,
        active_sample_voices,
        active_momentary_fx: 2,
        active_bus_fx_slots,
        active_global_fx_slots,
        expected_voice_steals: 0,
        expected_voice_admission_drops_start: 0,
        expected_voice_admission_drops_end: 0,
    }
}

fn assert_applied(units: usize, expected: ExpectedLiveState) {
    let name = format!("capacity_analogue_{units}");
    let scenario = build(&name, 44_100, 600_000).expect("capacity scenario");
    assert_eq!(scenario.expected, expected);
    let (actual, output) = apply_through_source(&scenario);
    assert_snapshot_matches(expected, actual);
    assert_eq!(actual.active_preview_sample_voices, 0);
    assert_eq!(actual.cumulative_voice_steals, 0);
    assert_eq!(actual.cumulative_voice_admission_drops, 0);
    assert!(
        output.iter().any(|sample| sample.abs() > f32::EPSILON),
        "capacity scenario rendered silence: {name}"
    );
}

fn apply_through_source(scenario: &LiveScenarioSpec) -> (SynthProfileSnapshot, Vec<f32>) {
    let (sender, receiver) = rodio_engine_source::event_queue();
    assert!(scenario.events.len() < CONTROL_QUEUE_CAPACITY);
    let first_momentary = scenario
        .events
        .iter()
        .position(|event| matches!(event, EngineEvent::PreparedMomentaryFxStart(_)))
        .expect("capacity momentary events");
    for event in &scenario.events[..first_momentary] {
        sender.send(event.clone()).expect("capacity event queue");
    }

    let mut source = EngineSource::with_block_frames(receiver, 44_100, 256);
    let mut output = (0..512)
        .map(|_| source.next().expect("capacity pre-mute sample"))
        .collect::<Vec<_>>();
    assert!(
        output.iter().any(|sample| sample.abs() > f32::EPSILON),
        "capacity scenario rendered silence before momentary FX"
    );

    for event in &scenario.events[first_momentary..] {
        sender.send(event.clone()).expect("capacity event queue");
    }
    let (probe_tx, probe_rx) = sync_channel(1);
    sender
        .send(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx: probe_tx,
        })
        .expect("capacity barrier event");

    for _ in 0..(CONTROL_QUEUE_CAPACITY * 2) {
        output.push(source.next().expect("capacity source sample"));
        if probe_rx.try_recv().is_ok() {
            return (source.profile_snapshot(), output);
        }
    }
    panic!("capacity event barrier was not drained");
}

fn duck_sources(config: &InstrumentsConfig) -> Vec<(usize, usize, &str, &str, &str)> {
    config
        .mixer
        .as_ref()
        .unwrap()
        .buses
        .iter()
        .enumerate()
        .flat_map(|(bus_index, bus)| {
            bus.slots
                .iter()
                .enumerate()
                .filter_map(move |(slot_index, slot)| {
                    let FxBusSlotConfig::Config { kind, params } = slot else {
                        return None;
                    };
                    if kind != "duck" {
                        return None;
                    }
                    let source = params.get("source")?.as_str()?;
                    let source_index = source
                        .strip_prefix('I')?
                        .parse::<usize>()
                        .ok()?
                        .checked_sub(1)?;
                    let source_instrument = config.instruments.get(source_index)?;
                    let source_route = source_instrument.mixer.as_ref()?.route.as_str();
                    Some((
                        bus_index,
                        slot_index,
                        source,
                        source_instrument.kind.as_str(),
                        source_route,
                    ))
                })
        })
        .collect()
}

fn slot_kind(slot: &FxBusSlotConfig) -> &str {
    match slot {
        FxBusSlotConfig::Kind(kind) => kind.as_str(),
        FxBusSlotConfig::Config { kind, .. } => kind.as_str(),
    }
}

fn assert_group_component(config: &InstrumentsConfig, instrument_start: usize, bus_start: usize) {
    let mut edges = [[false; 6]; 6];
    for instrument_index in instrument_start..instrument_start + 4 {
        let route = config.instruments[instrument_index]
            .mixer
            .as_ref()
            .unwrap()
            .route
            .as_str();
        if let Some(route) = route.strip_prefix("fx_bus_") {
            let route = route.parse::<usize>().unwrap();
            assert!((bus_start + 1..=bus_start + 2).contains(&route));
            connect(
                &mut edges,
                instrument_index - instrument_start,
                4 + route - bus_start - 1,
            );
        } else {
            assert_eq!(route, "direct");
        }
    }
    for (bus_offset, bus) in config.mixer.as_ref().unwrap().buses[bus_start..bus_start + 2]
        .iter()
        .enumerate()
    {
        for slot in &bus.slots {
            let FxBusSlotConfig::Config { kind, params } = slot else {
                continue;
            };
            if kind != "duck" {
                continue;
            }
            let source = params
                .get("source")
                .unwrap()
                .as_str()
                .unwrap()
                .strip_prefix('I')
                .unwrap()
                .parse::<usize>()
                .unwrap()
                - 1;
            assert!((instrument_start..instrument_start + 4).contains(&source));
            connect(&mut edges, 4 + bus_offset, source - instrument_start);
        }
    }
    let mut visited = [false; 6];
    let mut pending = vec![0];
    while let Some(node) = pending.pop() {
        if visited[node] {
            continue;
        }
        visited[node] = true;
        for (neighbor, connected) in edges[node].iter().enumerate() {
            if *connected && !visited[neighbor] {
                pending.push(neighbor);
            }
        }
    }
    assert!(visited.into_iter().all(|connected| connected));
}

fn connect(edges: &mut [[bool; 6]; 6], left: usize, right: usize) {
    edges[left][right] = true;
    edges[right][left] = true;
}

fn assert_snapshot_matches(expected: ExpectedLiveState, actual: SynthProfileSnapshot) {
    assert_eq!(actual.active_synth_voices, expected.active_synth_voices);
    assert_eq!(actual.active_sample_voices, expected.active_sample_voices);
    assert_eq!(actual.active_momentary_fx, expected.active_momentary_fx);
    assert_eq!(actual.active_bus_fx_slots, expected.active_bus_fx_slots);
    assert_eq!(
        actual.active_global_fx_slots,
        expected.active_global_fx_slots
    );
}
