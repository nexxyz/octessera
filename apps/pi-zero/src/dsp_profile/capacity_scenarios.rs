use crate::dsp_scenarios::{ExpectedLiveState, LiveScenarioSpec, LIVE_SAMPLE_LIFETIME_SECONDS};
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, VoiceStealingMode, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

const ALL_SLOTS: [usize; INSTRUMENT_SLOT_COUNT] = [0, 1, 2, 3, 4, 5, 6, 7];
const MIXED_SYNTH_SLOTS: [usize; 6] = [0, 2, 3, 4, 6, 7];
const MIXED_SAMPLE_SLOTS: [usize; 2] = [1, 5];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CapacityScenario {
    Synth(usize),
    Sample(usize),
    Mixed { synth: usize, sample: usize },
}

pub(crate) fn parse(name: &str) -> Option<CapacityScenario> {
    let capacity = realtime_engine::synth::SYNTH_VOICE_LANE_CAPACITY;
    match name.split('_').collect::<Vec<_>>().as_slice() {
        ["capacity", "synth", count] => parse_count(count, capacity).map(CapacityScenario::Synth),
        ["capacity", "sample", count] => {
            parse_count(count, realtime_engine::synth::SAMPLE_VOICE_LANE_CAPACITY)
                .map(CapacityScenario::Sample)
        }
        ["capacity", "mixed", synth, sample] => Some(CapacityScenario::Mixed {
            synth: parse_count(synth, capacity)?,
            sample: parse_count(sample, realtime_engine::synth::SAMPLE_VOICE_LANE_CAPACITY)?,
        }),
        _ => None,
    }
}

pub(crate) fn build(
    name: &str,
    sample_rate: u32,
    note_duration_ms: u32,
) -> Option<LiveScenarioSpec> {
    let scenario = parse(name)?;
    let sample_banks = match scenario {
        CapacityScenario::Synth(_) => None,
        CapacityScenario::Sample(_) | CapacityScenario::Mixed { .. } => {
            Some(crate::dsp_profile::samples::long_sample_banks(
                sample_rate,
                LIVE_SAMPLE_LIFETIME_SECONDS,
            ))
        }
    };
    let (synth_counts, sample_counts, instruments) = match scenario {
        CapacityScenario::Synth(count) => (
            distribute(count, &ALL_SLOTS),
            [0; INSTRUMENT_SLOT_COUNT],
            all_synth_instruments(),
        ),
        CapacityScenario::Sample(count) => (
            [0; INSTRUMENT_SLOT_COUNT],
            distribute(count, &ALL_SLOTS),
            all_sample_instruments(),
        ),
        CapacityScenario::Mixed { synth, sample } => (
            distribute(synth, &MIXED_SYNTH_SLOTS),
            distribute(sample, &MIXED_SAMPLE_SLOTS),
            mixed_instruments(),
        ),
    };
    let mut events = vec![EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
        instruments,
        sample_banks,
        Some(VoiceStealingMode::None),
        sample_rate,
    ))];
    push_synth_notes(&mut events, &synth_counts, note_duration_ms);
    push_sample_notes(&mut events, &sample_counts, note_duration_ms);
    if matches!(scenario, CapacityScenario::Mixed { .. }) {
        events.extend(momentary_events(sample_rate));
    }
    Some(LiveScenarioSpec {
        events,
        expected: expected_for_scenario(scenario),
    })
}

pub(crate) fn expected(name: &str) -> Option<ExpectedLiveState> {
    parse(name).map(expected_for_scenario)
}

fn parse_count(value: &str, capacity: usize) -> Option<usize> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let count = value.parse::<usize>().ok()?;
    (1..=capacity).contains(&count).then_some(count)
}

fn expected_for_scenario(scenario: CapacityScenario) -> ExpectedLiveState {
    let (active_synth_voices, active_sample_voices, has_fx) = match scenario {
        CapacityScenario::Synth(count) => (count, 0, false),
        CapacityScenario::Sample(count) => (0, count, false),
        CapacityScenario::Mixed { synth, sample } => (synth, sample, true),
    };
    ExpectedLiveState {
        active_synth_voices,
        active_sample_voices,
        active_momentary_fx: if has_fx { 2 } else { 0 },
        active_bus_fx_slots: if has_fx { 4 } else { 0 },
        active_global_fx_slots: usize::from(has_fx),
        expected_voice_steals: 0,
        expected_voice_admission_drops_start: 0,
        expected_voice_admission_drops_end: 0,
    }
}

fn distribute(total: usize, slots: &[usize]) -> [usize; INSTRUMENT_SLOT_COUNT] {
    let mut counts = [0; INSTRUMENT_SLOT_COUNT];
    let base = total / slots.len();
    let extra = total % slots.len();
    for (index, slot) in slots.iter().enumerate() {
        counts[*slot] = base + usize::from(index < extra);
    }
    counts
}

fn push_synth_notes(
    events: &mut Vec<EngineEvent>,
    counts: &[usize; INSTRUMENT_SLOT_COUNT],
    note_duration_ms: u32,
) {
    for (slot, count) in counts.iter().enumerate() {
        for index in 0..*count {
            events.push(note_event(slot, 60 + index as u8, note_duration_ms));
        }
    }
}

fn push_sample_notes(
    events: &mut Vec<EngineEvent>,
    counts: &[usize; INSTRUMENT_SLOT_COUNT],
    note_duration_ms: u32,
) {
    for (slot, count) in counts.iter().enumerate() {
        for _ in 0..*count {
            events.push(note_event(slot, 36, note_duration_ms));
        }
    }
}

fn note_event(slot: usize, note: u8, duration_ms: u32) -> EngineEvent {
    EngineEvent::NoteOn {
        instrument_slot: slot as u8,
        note,
        velocity: 100,
        duration_ms,
    }
}

fn all_synth_instruments() -> InstrumentsConfig {
    instruments_config(
        ["synth"; INSTRUMENT_SLOT_COUNT],
        [0; INSTRUMENT_SLOT_COUNT],
        None,
    )
}

fn all_sample_instruments() -> InstrumentsConfig {
    instruments_config(
        ["sampler"; INSTRUMENT_SLOT_COUNT],
        [0; INSTRUMENT_SLOT_COUNT],
        None,
    )
}

fn mixed_instruments() -> InstrumentsConfig {
    instruments_config(
        [
            "synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth",
        ],
        [1, 0, 1, 2, 1, 0, 1, 2],
        Some(MixerConfig {
            buses: vec![bus(["delay", "duck"]), bus(["duck", "saturator"])],
            master: Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind("compressor".into())],
            }),
        }),
    )
}

fn instruments_config(
    kinds: [&str; INSTRUMENT_SLOT_COUNT],
    routes: [usize; INSTRUMENT_SLOT_COUNT],
    mixer: Option<MixerConfig>,
) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: kinds
            .iter()
            .enumerate()
            .map(|(slot, kind)| InstrumentSlotConfig {
                kind: (*kind).into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: route_name(routes[slot]),
                    pan_pos: slot.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn bus(kinds: [&str; 2]) -> FxBusConfig {
    FxBusConfig {
        slots: kinds
            .into_iter()
            .map(|kind| FxBusSlotConfig::Kind(kind.into()))
            .collect(),
        pan_pos: 16,
        volume_pct: 100.0,
    }
}

fn route_name(route: usize) -> String {
    if route == 0 {
        "direct".into()
    } else {
        format!("fx_bus_{route}")
    }
}

fn momentary_events(sample_rate: u32) -> Vec<EngineEvent> {
    [
        (
            "capacity-stutter",
            "stutter",
            BTreeMap::from([
                ("depthPct".into(), serde_json::json!(100)),
                ("rateHz".into(), serde_json::json!(8)),
            ]),
        ),
        (
            "capacity-freeze",
            "freeze",
            BTreeMap::from([
                ("mixPct".into(), serde_json::json!(100)),
                ("releaseMs".into(), serde_json::json!(500)),
            ]),
        ),
    ]
    .into_iter()
    .map(|(id, kind, params)| {
        EngineEvent::PreparedMomentaryFxStart(
            prepare_momentary_fx_start(
                id.into(),
                kind.into(),
                params,
                MomentaryFxTarget::Global,
                sample_rate,
            )
            .expect("capacity scenario momentary FX is valid"),
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp_profile::telemetry::apply_events;
    use realtime_engine::synth::{
        FxBusSlotConfig, SynthEngine, SynthProfileSnapshot, SYNTH_VOICE_LANE_CAPACITY,
    };

    #[test]
    fn benchmark_capacity_features_override_both_lane_capacities() {
        #[cfg(feature = "benchmark-voice-pools-128")]
        assert_eq!(SYNTH_VOICE_LANE_CAPACITY, 128);
        #[cfg(feature = "benchmark-voice-pools-256")]
        assert_eq!(SYNTH_VOICE_LANE_CAPACITY, 256);
        assert_eq!(
            realtime_engine::synth::SAMPLE_VOICE_LANE_CAPACITY,
            SYNTH_VOICE_LANE_CAPACITY
        );
    }

    #[test]
    fn parser_requires_canonical_positive_bounded_counts() {
        let capacity = SYNTH_VOICE_LANE_CAPACITY;
        assert_eq!(
            parse(&format!("capacity_synth_{capacity}")),
            Some(CapacityScenario::Synth(capacity))
        );
        assert_eq!(
            parse(&format!("capacity_sample_{capacity}")),
            Some(CapacityScenario::Sample(capacity))
        );
        assert_eq!(
            parse(&format!("capacity_mixed_{capacity}_{capacity}")),
            Some(CapacityScenario::Mixed {
                synth: capacity,
                sample: capacity
            })
        );
        for name in [
            "capacity_synth_0",
            "capacity_synth_01",
            "capacity_sample_00",
            "capacity_mixed_1_1_1",
            "capacity_mixed_1_",
            "capacity_synth_18446744073709551616",
            "capacity_synth_257",
            "capacity_sample_257",
            "capacity_mixed_1_257",
        ] {
            assert_eq!(parse(name), None, "accepted invalid scenario {name}");
        }
    }

    #[test]
    fn prepared_sample_banks_are_absent_for_synth_and_shared_for_sample_and_mixed() {
        let capacity = SYNTH_VOICE_LANE_CAPACITY;
        let synth = build(&format!("capacity_synth_{capacity}"), 44_100, 600_000).unwrap();
        assert!(prepared_config(&synth).sample_banks().is_none());

        for name in [
            format!("capacity_sample_{capacity}"),
            format!("capacity_mixed_{capacity}_{capacity}"),
        ] {
            let scenario = build(&name, 44_100, 600_000).unwrap();
            let banks = prepared_config(&scenario).sample_banks().unwrap();
            assert_eq!(banks.len(), INSTRUMENT_SLOT_COUNT);
            let first = &banks[0].slots[0].buffer.as_ref().unwrap().samples;
            for bank in banks.iter().skip(1) {
                assert!(std::sync::Arc::ptr_eq(
                    first,
                    &bank.slots[0].buffer.as_ref().unwrap().samples
                ));
            }
        }
    }

    #[test]
    fn exact_limit_scenarios_apply_with_expected_slots_fx_and_counters() {
        let capacity = SYNTH_VOICE_LANE_CAPACITY;
        for (name, expected_slots) in [
            (
                format!("capacity_synth_{capacity}"),
                [capacity / 8; INSTRUMENT_SLOT_COUNT],
            ),
            (
                format!("capacity_sample_{capacity}"),
                [capacity / 8; INSTRUMENT_SLOT_COUNT],
            ),
        ] {
            let scenario = build(&name, 44_100, 600_000).unwrap();
            let observed_slots = note_on_slots(&scenario.events);
            assert_eq!(observed_slots, expected_slots);
            let mut engine = SynthEngine::new(44_100);
            let retired = apply_events(&mut engine, &scenario.events);
            assert_snapshot_matches(scenario.expected, engine.profile_snapshot());
            assert_eq!(
                engine.profile_snapshot().cumulative_voice_admission_drops,
                0
            );
            drop(retired);
        }

        assert!(build(&format!("capacity_synth_{}", capacity + 1), 44_100, 600_000).is_none());
        assert!(build(
            &format!("capacity_sample_{}", capacity + 1),
            44_100,
            600_000
        )
        .is_none());
    }

    #[test]
    fn mixed_limit_scenario_uses_default_like_topology_and_exact_distribution() {
        let capacity = SYNTH_VOICE_LANE_CAPACITY;
        let name = format!("capacity_mixed_{capacity}_{capacity}");
        let scenario = build(&name, 44_100, 600_000).unwrap();
        let quotient = capacity / 6;
        let remainder = capacity % 6;
        let mut expected_slots = [0; INSTRUMENT_SLOT_COUNT];
        for (index, slot) in MIXED_SYNTH_SLOTS.iter().enumerate() {
            expected_slots[*slot] = quotient + usize::from(index < remainder);
        }
        expected_slots[1] = capacity / 2;
        expected_slots[5] = capacity - expected_slots[1];
        assert_eq!(note_on_slots(&scenario.events), expected_slots);

        let instruments = mixed_instruments();
        assert_eq!(
            instruments
                .instruments
                .iter()
                .map(|slot| slot.kind.as_str())
                .collect::<Vec<_>>(),
            ["synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth"]
        );
        assert_eq!(
            instruments
                .instruments
                .iter()
                .map(|slot| slot.mixer.as_ref().unwrap().route.as_str())
                .collect::<Vec<_>>(),
            [
                "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "fx_bus_1", "direct", "fx_bus_1",
                "fx_bus_2"
            ]
        );
        let mixer = instruments.mixer.unwrap();
        let bus_kinds: Vec<_> = mixer
            .buses
            .iter()
            .flat_map(|bus| bus.slots.iter())
            .map(|slot| match slot {
                FxBusSlotConfig::Kind(kind) => kind.as_str(),
                FxBusSlotConfig::Config { .. } => "config",
            })
            .collect();
        assert_eq!(bus_kinds, ["delay", "duck", "duck", "saturator"]);
        assert_eq!(
            mixer
                .master
                .unwrap()
                .slots
                .iter()
                .map(|slot| match slot {
                    FxBusSlotConfig::Kind(kind) => kind.as_str(),
                    FxBusSlotConfig::Config { .. } => "config",
                })
                .collect::<Vec<_>>(),
            ["compressor"]
        );

        let mut engine = SynthEngine::new(44_100);
        let retired = apply_events(&mut engine, &scenario.events);
        assert_snapshot_matches(scenario.expected, engine.profile_snapshot());
        drop(retired);
    }

    fn note_on_slots(events: &[EngineEvent]) -> [usize; INSTRUMENT_SLOT_COUNT] {
        let mut counts = [0; INSTRUMENT_SLOT_COUNT];
        for event in events {
            if let EngineEvent::NoteOn {
                instrument_slot, ..
            } = event
            {
                counts[*instrument_slot as usize] += 1;
            }
        }
        counts
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
        assert_eq!(
            actual.cumulative_voice_steals,
            expected.expected_voice_steals
        );
        assert_eq!(
            actual.cumulative_voice_admission_drops,
            expected.expected_voice_admission_drops_start
        );
    }

    fn prepared_config(
        scenario: &LiveScenarioSpec,
    ) -> &realtime_engine::synth::PreparedAudioConfig {
        scenario
            .events
            .iter()
            .find_map(|event| match event {
                EngineEvent::SetPreparedAudioConfig(config) => Some(config),
                _ => None,
            })
            .unwrap()
    }
}
