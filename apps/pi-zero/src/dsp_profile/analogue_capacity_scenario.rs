use crate::dsp_scenarios::{ExpectedLiveState, LiveScenarioSpec, LIVE_SAMPLE_LIFETIME_SECONDS};
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, VoiceStealingMode, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY, SYNTH_VOICE_LANE_CAPACITY,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

const SHIPPED_SYNTH_SLOTS: [usize; 3] = [0, 2, 3];
const SHIPPED_SAMPLE_SLOTS: [usize; 1] = [1];
const EXPANDED_SYNTH_SLOTS: [usize; 6] = [0, 2, 3, 4, 6, 7];
const EXPANDED_SAMPLE_SLOTS: [usize; 2] = [1, 5];
const BUS_FX_KINDS: [[&str; 3]; 4] = [
    ["delay", "duck", "reverb"],
    ["duck", "saturator", "chorus"],
    ["delay", "duck", "filter_lfo"],
    ["duck", "saturator", "eq"],
];
const DUCK_SOURCES: [&str; 4] = ["I2", "I1", "I6", "I5"];

pub(crate) fn parse(name: &str) -> Option<usize> {
    let value = name.strip_prefix("capacity_analogue_")?;
    parse_units(value)
}

pub(crate) fn build(
    name: &str,
    sample_rate: u32,
    note_duration_ms: u32,
) -> Option<LiveScenarioSpec> {
    let units = parse(name)?;
    let synth_slots = synth_slots(units);
    let sample_slots = sample_slots(units);
    let instruments = instruments(units);
    let mut events = vec![EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
        instruments,
        Some(crate::dsp_profile::samples::long_sample_banks(
            sample_rate,
            LIVE_SAMPLE_LIFETIME_SECONDS,
        )),
        Some(VoiceStealingMode::None),
        sample_rate,
    ))];
    push_synth_notes(
        &mut events,
        &distribute(3 * units, synth_slots),
        note_duration_ms,
    );
    push_sample_notes(
        &mut events,
        &distribute(units, sample_slots),
        note_duration_ms,
    );
    events.extend(momentary_events(units, sample_rate));
    Some(LiveScenarioSpec {
        events,
        expected: expected_for_units(units),
    })
}

pub(crate) fn expected(name: &str) -> Option<ExpectedLiveState> {
    parse(name).map(expected_for_units)
}

fn parse_units(value: &str) -> Option<usize> {
    if value.is_empty()
        || value.len() > 1 && value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let units = value.parse::<usize>().ok()?;
    (1..=max_units()).contains(&units).then_some(units)
}

fn max_units() -> usize {
    SAMPLE_VOICE_LANE_CAPACITY.min(SYNTH_VOICE_LANE_CAPACITY / 3)
}

fn expected_for_units(units: usize) -> ExpectedLiveState {
    ExpectedLiveState {
        active_synth_voices: 3 * units,
        active_sample_voices: units,
        active_momentary_fx: momentary_count(units),
        active_bus_fx_slots: bus_fx_count(units),
        active_global_fx_slots: global_fx_count(units),
        expected_voice_steals: 0,
        expected_voice_admission_drops_start: 0,
        expected_voice_admission_drops_end: 0,
    }
}

fn synth_slots(units: usize) -> &'static [usize] {
    if units <= 8 {
        &SHIPPED_SYNTH_SLOTS
    } else {
        &EXPANDED_SYNTH_SLOTS
    }
}

fn sample_slots(units: usize) -> &'static [usize] {
    if units <= 8 {
        &SHIPPED_SAMPLE_SLOTS
    } else {
        &EXPANDED_SAMPLE_SLOTS
    }
}

fn distribute(total: usize, slots: &[usize]) -> [usize; INSTRUMENT_SLOT_COUNT] {
    let mut counts = [0; INSTRUMENT_SLOT_COUNT];
    let base = total / slots.len();
    let remainder = total % slots.len();
    for (index, slot) in slots.iter().enumerate() {
        counts[*slot] = base + usize::from(index < remainder);
    }
    counts
}

fn push_synth_notes(
    events: &mut Vec<EngineEvent>,
    counts: &[usize; INSTRUMENT_SLOT_COUNT],
    duration_ms: u32,
) {
    for (slot, count) in counts.iter().enumerate() {
        for index in 0..*count {
            events.push(note_event(slot, 60 + index as u8, duration_ms));
        }
    }
}

fn push_sample_notes(
    events: &mut Vec<EngineEvent>,
    counts: &[usize; INSTRUMENT_SLOT_COUNT],
    duration_ms: u32,
) {
    for (slot, count) in counts.iter().enumerate() {
        for _ in 0..*count {
            events.push(note_event(slot, 36, duration_ms));
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

fn instruments(units: usize) -> InstrumentsConfig {
    let expanded = units > 8;
    let kinds = if expanded {
        [
            "synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth",
        ]
    } else {
        [
            "synth", "sampler", "synth", "synth", "none", "none", "none", "none",
        ]
    };
    let routes = if expanded {
        [1, 0, 1, 2, 3, 0, 3, 4]
    } else {
        [1, 0, 1, 2, 0, 0, 0, 0]
    };
    InstrumentsConfig {
        instruments: kinds
            .into_iter()
            .enumerate()
            .map(|(slot, kind)| InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: route_name(routes[slot]),
                    pan_pos: slot.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: bus_configs(units),
            master: Some(MasterFxConfig {
                slots: global_fx_slots(units),
            }),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn bus_configs(units: usize) -> Vec<FxBusConfig> {
    let bus_count = if units <= 8 { 2 } else { 4 };
    let mut remaining = bus_fx_count(units);
    let mut buses: Vec<Vec<FxBusSlotConfig>> = (0..bus_count).map(|_| Vec::new()).collect();
    for (bus_index, slots) in buses.iter_mut().enumerate() {
        let count = remaining.min(2);
        remaining -= count;
        slots.extend(
            BUS_FX_KINDS[bus_index][..count]
                .iter()
                .map(|kind| bus_fx_slot(kind, bus_index)),
        );
    }
    for (bus_index, slots) in buses.iter_mut().enumerate() {
        if remaining == 0 {
            break;
        }
        slots.push(bus_fx_slot(BUS_FX_KINDS[bus_index][2], bus_index));
        remaining -= 1;
    }
    buses
        .into_iter()
        .map(|slots| FxBusConfig {
            slots,
            pan_pos: 16,
            volume_pct: 100.0,
        })
        .collect()
}

fn bus_fx_slot(kind: &str, bus_index: usize) -> FxBusSlotConfig {
    if kind != "duck" {
        return FxBusSlotConfig::Kind(kind.into());
    }
    FxBusSlotConfig::Config {
        kind: kind.into(),
        params: BTreeMap::from([("source".into(), serde_json::json!(DUCK_SOURCES[bus_index]))]),
    }
}

fn global_fx_slots(units: usize) -> Vec<FxBusSlotConfig> {
    ["compressor", "reverb"][..global_fx_count(units)]
        .iter()
        .map(|kind| FxBusSlotConfig::Kind((*kind).into()))
        .collect()
}

fn momentary_events(units: usize, sample_rate: u32) -> Vec<EngineEvent> {
    [
        (
            "capacity-analogue-stutter",
            "stutter",
            BTreeMap::from([
                ("depthPct".into(), serde_json::json!(100)),
                ("rateHz".into(), serde_json::json!(8)),
            ]),
        ),
        (
            "capacity-analogue-freeze",
            "freeze",
            BTreeMap::from([
                ("mixPct".into(), serde_json::json!(100)),
                ("releaseMs".into(), serde_json::json!(500)),
            ]),
        ),
    ]
    .into_iter()
    .take(momentary_count(units))
    .map(|(id, kind, params)| {
        EngineEvent::PreparedMomentaryFxStart(
            prepare_momentary_fx_start(
                id.into(),
                kind.into(),
                params,
                MomentaryFxTarget::Global,
                sample_rate,
            )
            .expect("analogue capacity momentary FX is valid"),
        )
    })
    .collect()
}

fn bus_fx_count(units: usize) -> usize {
    units.div_ceil(2).min(12)
}

fn global_fx_count(units: usize) -> usize {
    units.div_ceil(8).min(2)
}

fn momentary_count(units: usize) -> usize {
    units.div_ceil(4).min(2)
}

fn route_name(route: usize) -> String {
    if route == 0 {
        "direct".into()
    } else {
        format!("fx_bus_{route}")
    }
}

#[cfg(test)]
#[path = "analogue_capacity_scenario_tests.rs"]
mod tests;
