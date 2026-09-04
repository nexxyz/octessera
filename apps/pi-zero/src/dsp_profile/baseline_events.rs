use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, SampleBankConfig, VoiceStealingMode,
    DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

const BASELINE_NOTE_DURATION_MS: u32 = 240_000;
const BASELINE_FX_KINDS: [&str; 12] = [
    "delay",
    "reverb",
    "glitch",
    "flanger",
    "chorus",
    "filter_lfo",
    "wah",
    "vibrato",
    "vinyl",
    "auto_pan",
    "compressor",
    "eq",
];

pub(super) fn synth_events(
    voices: usize,
    mode: VoiceStealingMode,
    sample_rate: u32,
    slot_count: usize,
) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        instruments(
            ["synth"; INSTRUMENT_SLOT_COUNT],
            [0; INSTRUMENT_SLOT_COUNT],
            None,
        ),
        None,
        mode,
        sample_rate,
    )];
    push_distributed_notes(&mut events, voices, 60, false, slot_count);
    events
}

pub(super) fn sample_events(
    voices: usize,
    sample_rate: u32,
    sample_banks: &[SampleBankConfig],
    slot_count: usize,
) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        instruments(
            ["sampler"; INSTRUMENT_SLOT_COUNT],
            [0; INSTRUMENT_SLOT_COUNT],
            None,
        ),
        Some(sample_banks.to_vec()),
        VoiceStealingMode::AutoBalanced,
        sample_rate,
    )];
    push_distributed_notes(&mut events, voices, 36, true, slot_count);
    events
}

pub(super) fn mixed_events(
    synth_voices: usize,
    sample_voices: usize,
    sample_rate: u32,
    sample_banks: &[SampleBankConfig],
) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        instruments(
            [
                "synth", "synth", "synth", "synth", "sampler", "sampler", "sampler", "sampler",
            ],
            [0; INSTRUMENT_SLOT_COUNT],
            None,
        ),
        Some(sample_banks.to_vec()),
        VoiceStealingMode::AutoBalanced,
        sample_rate,
    )];
    push_distributed_notes_in_slots(&mut events, synth_voices, 60, false, 0, 4);
    push_distributed_notes_in_slots(&mut events, sample_voices, 36, true, 4, 4);
    events
}

pub(super) fn mixed_ramp_16_48_events(
    sample_rate: u32,
    sample_banks: &[SampleBankConfig],
) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        instruments(
            [
                "synth", "synth", "sampler", "sampler", "sampler", "sampler", "sampler", "sampler",
            ],
            [0; INSTRUMENT_SLOT_COUNT],
            None,
        ),
        Some(sample_banks.to_vec()),
        VoiceStealingMode::Fixed16,
        sample_rate,
    )];
    for slot in 0..2 {
        push_distributed_notes_in_slots(&mut events, 8, 60, false, slot, 1);
    }
    for slot in 2..INSTRUMENT_SLOT_COUNT {
        push_distributed_notes_in_slots(&mut events, 8, 36, true, slot, 1);
    }
    events
}

pub(super) fn default_capacity_events(
    synth_slots: &[usize],
    sample_slots: &[usize],
    sample_rate: u32,
    sample_banks: &[SampleBankConfig],
) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        default_capacity_instruments(synth_slots, sample_slots),
        Some(sample_banks.to_vec()),
        VoiceStealingMode::None,
        sample_rate,
    )];
    for &slot in synth_slots {
        push_distributed_notes_in_slots(&mut events, 8, 60, false, slot, 1);
    }
    for &slot in sample_slots {
        push_distributed_notes_in_slots(&mut events, 8, 36, true, slot, 1);
    }
    events.extend(default_capacity_momentary_events(sample_rate));
    events
}

pub(super) fn fx_events(
    bus_slots: usize,
    global_slots: usize,
    momentary: usize,
    sample_rate: u32,
    sample_banks: &[SampleBankConfig],
) -> Vec<EngineEvent> {
    let bus_count = bus_slots.div_ceil(3);
    let routes = std::array::from_fn(|slot| {
        if bus_count == 0 {
            0
        } else {
            (slot % bus_count) + 1
        }
    });
    let mut events = vec![prepared_config(
        instruments(
            [
                "synth", "synth", "synth", "synth", "sampler", "sampler", "sampler", "sampler",
            ],
            routes,
            Some(mixer(bus_slots, global_slots)),
        ),
        Some(sample_banks.to_vec()),
        VoiceStealingMode::AutoBalanced,
        sample_rate,
    )];
    push_distributed_notes_in_slots(&mut events, 8, 60, false, 0, 4);
    push_distributed_notes_in_slots(&mut events, 8, 36, true, 4, 4);
    events.extend(momentary_events(momentary, sample_rate));
    events
}

fn prepared_config(
    instruments: InstrumentsConfig,
    sample_banks: Option<Vec<SampleBankConfig>>,
    voice_stealing_mode: VoiceStealingMode,
    sample_rate: u32,
) -> EngineEvent {
    EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
        instruments,
        sample_banks,
        Some(voice_stealing_mode),
        sample_rate,
    ))
}

fn instruments(
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

fn default_capacity_instruments(
    synth_slots: &[usize],
    sample_slots: &[usize],
) -> InstrumentsConfig {
    let mut kinds = ["none"; INSTRUMENT_SLOT_COUNT];
    let mut routes = [0; INSTRUMENT_SLOT_COUNT];
    for (index, &slot) in synth_slots.iter().enumerate() {
        kinds[slot] = "synth";
        routes[slot] = [1, 1, 2][index % 3];
    }
    for &slot in sample_slots {
        kinds[slot] = "sampler";
    }
    instruments(kinds, routes, Some(default_capacity_mixer()))
}

fn default_capacity_mixer() -> MixerConfig {
    MixerConfig {
        buses: vec![
            bus(vec!["delay", "duck"], 16),
            bus(vec!["duck", "saturator"], 16),
        ],
        master: Some(MasterFxConfig {
            slots: vec![FxBusSlotConfig::Kind("compressor".into())],
        }),
    }
}

fn default_capacity_momentary_events(sample_rate: u32) -> Vec<EngineEvent> {
    [
        (
            "default-stutter",
            "stutter",
            BTreeMap::from([
                ("depthPct".into(), serde_json::json!(100)),
                ("rateHz".into(), serde_json::json!(8)),
            ]),
        ),
        (
            "default-freeze",
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
            .expect("default capacity momentary FX is valid"),
        )
    })
    .collect()
}

fn mixer(bus_slots: usize, global_slots: usize) -> MixerConfig {
    MixerConfig {
        buses: (0..bus_slots.div_ceil(3))
            .map(|bus_index| {
                let slots = bus_slots.saturating_sub(bus_index * 3).min(3);
                bus(
                    (0..slots)
                        .map(|slot| fx_kind(bus_index * 3 + slot))
                        .collect(),
                    bus_index + 1,
                )
            })
            .collect(),
        master: Some(MasterFxConfig {
            slots: (0..global_slots.min(2))
                .map(|slot| FxBusSlotConfig::Kind(global_fx_kind(slot).into()))
                .collect(),
        }),
    }
}

fn bus(kinds: Vec<&str>, pan_pos: usize) -> FxBusConfig {
    FxBusConfig {
        slots: kinds
            .into_iter()
            .map(|kind| FxBusSlotConfig::Kind(kind.into()))
            .collect(),
        pan_pos,
        volume_pct: 100.0,
    }
}

fn momentary_events(count: usize, sample_rate: u32) -> Vec<EngineEvent> {
    [
        ("baseline-filter", "filter_sweep", MomentaryFxTarget::Global),
        (
            "baseline-pitch",
            "pitch_shift",
            MomentaryFxTarget::Instrument { index: 1 },
        ),
    ]
    .into_iter()
    .take(count.min(2))
    .map(|(id, kind, target)| {
        EngineEvent::PreparedMomentaryFxStart(
            prepare_momentary_fx_start(
                id.into(),
                kind.into(),
                BTreeMap::new(),
                target,
                sample_rate,
            )
            .expect("baseline momentary FX is valid"),
        )
    })
    .collect()
}

fn push_distributed_notes(
    events: &mut Vec<EngineEvent>,
    total: usize,
    note: u8,
    sample: bool,
    slot_count: usize,
) {
    push_distributed_notes_in_slots(events, total, note, sample, 0, slot_count);
}

fn push_distributed_notes_in_slots(
    events: &mut Vec<EngineEvent>,
    total: usize,
    note: u8,
    sample: bool,
    start: usize,
    slot_count: usize,
) {
    let base = total / slot_count;
    let extra = total % slot_count;
    for offset in 0..slot_count {
        let count = base + usize::from(offset < extra);
        for index in 0..count {
            events.push(EngineEvent::NoteOn {
                instrument_slot: (start + offset) as u8,
                note: if sample { 36 } else { note + index as u8 },
                velocity: 100,
                duration_ms: BASELINE_NOTE_DURATION_MS,
            });
        }
    }
}

fn route_name(route: usize) -> String {
    if route == 0 {
        "direct".into()
    } else {
        format!("fx_bus_{route}")
    }
}

fn fx_kind(index: usize) -> &'static str {
    BASELINE_FX_KINDS[index]
}

fn global_fx_kind(index: usize) -> &'static str {
    ["compressor", "reverb"][index]
}

#[cfg(test)]
#[path = "baseline_events_tests.rs"]
mod tests;
