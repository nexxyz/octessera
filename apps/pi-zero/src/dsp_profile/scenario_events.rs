use super::fx_cases::{fx_mixer, fx_routes};
use crate::dsp_profile::samples::all_sample_banks;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MixerConfig, VoiceStealingMode,
    DEFAULT_AUDIO_SAMPLE_RATE, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

const PROFILE_NOTE_DURATION_MS: u32 = 60_000;

pub(super) fn baseline_events() -> Vec<EngineEvent> {
    vec![prepared_config(
        all_synth_instruments([0; INSTRUMENT_SLOT_COUNT], None),
        None,
        VoiceStealingMode::None,
        DEFAULT_AUDIO_SAMPLE_RATE,
    )]
}

pub(super) fn synth_ramp_events(voices: usize) -> Vec<EngineEvent> {
    let mut events = baseline_events();
    for (slot, count) in distribute(voices, 0, INSTRUMENT_SLOT_COUNT)
        .iter()
        .enumerate()
    {
        push_synth_voices(&mut events, slot, *count);
    }
    events
}

pub(super) fn sample_ramp_events(voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        all_sample_instruments([0; INSTRUMENT_SLOT_COUNT], None),
        Some(all_sample_banks(sample_rate)),
        VoiceStealingMode::None,
        sample_rate,
    )];
    for (slot, count) in distribute(voices, 0, INSTRUMENT_SLOT_COUNT)
        .iter()
        .enumerate()
    {
        push_sample_voices(&mut events, slot, *count);
    }
    events
}

pub(super) fn mixed_ramp_events(voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    mixed_events(VoiceStealingMode::None, voices, sample_rate)
}

pub(super) fn synth_overload_events(voices: usize, slots: usize) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        all_synth_instruments([0; INSTRUMENT_SLOT_COUNT], None),
        None,
        VoiceStealingMode::AutoBalanced,
        DEFAULT_AUDIO_SAMPLE_RATE,
    )];
    for (slot, count) in distribute(voices, 0, slots).iter().enumerate() {
        push_synth_voices(&mut events, slot, *count);
    }
    events
}

pub(super) fn sample_overload_events(
    voices: usize,
    slots: usize,
    sample_rate: u32,
) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        all_sample_instruments([0; INSTRUMENT_SLOT_COUNT], None),
        Some(all_sample_banks(sample_rate)),
        VoiceStealingMode::AutoBalanced,
        sample_rate,
    )];
    for (slot, count) in distribute(voices, 0, slots).iter().enumerate() {
        push_sample_voices(&mut events, slot, *count);
    }
    events
}

pub(super) fn mixed_overload_events(voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    mixed_events(VoiceStealingMode::AutoBalanced, voices, sample_rate)
}

pub(super) fn fx_ramp_events(mode: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mixer = fx_mixer(mode);
    let routes = fx_routes(mode);
    let mut events = vec![prepared_config(
        all_synth_instruments(routes, mixer),
        (mode > 0).then(|| all_sample_banks(sample_rate)),
        VoiceStealingMode::None,
        sample_rate,
    )];
    for (slot, count) in distribute(16, 0, INSTRUMENT_SLOT_COUNT).iter().enumerate() {
        push_synth_voices(&mut events, slot, *count);
    }
    events
}

pub(super) fn momentary_events(mode: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        all_synth_instruments([0; INSTRUMENT_SLOT_COUNT], None),
        Some(all_sample_banks(sample_rate)),
        VoiceStealingMode::None,
        sample_rate,
    )];
    for (slot, count) in distribute(16, 0, INSTRUMENT_SLOT_COUNT).iter().enumerate() {
        push_synth_voices(&mut events, slot, *count);
    }
    for (id, fx_type, params, target) in momentary_fx_specs(mode) {
        events.push(EngineEvent::PreparedMomentaryFxStart(
            prepare_momentary_fx_start(id, fx_type, params, target, sample_rate).unwrap(),
        ));
    }
    events
}

fn mixed_events(mode: VoiceStealingMode, voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        mixed_instruments([0; INSTRUMENT_SLOT_COUNT], None),
        Some(all_sample_banks(sample_rate)),
        mode,
        sample_rate,
    )];
    for (slot, count) in distribute(voices, 0, INSTRUMENT_SLOT_COUNT / 2)
        .iter()
        .enumerate()
    {
        push_synth_voices(&mut events, slot, *count);
    }
    for (slot, count) in distribute(voices, INSTRUMENT_SLOT_COUNT / 2, INSTRUMENT_SLOT_COUNT / 2)
        .iter()
        .enumerate()
    {
        push_sample_voices(&mut events, slot, *count);
    }
    events
}

fn prepared_config(
    instruments: InstrumentsConfig,
    sample_banks: Option<Vec<realtime_engine::synth::SampleBankConfig>>,
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

fn momentary_fx_specs(
    mode: usize,
) -> Vec<(
    String,
    String,
    BTreeMap<String, serde_json::Value>,
    realtime_engine::synth::MomentaryFxTarget,
)> {
    use realtime_engine::synth::MomentaryFxTarget;
    let mut rows = vec![(
        "fx-filter".into(),
        "filter_sweep".into(),
        BTreeMap::new(),
        MomentaryFxTarget::Global,
    )];
    if mode >= 2 {
        rows.push((
            "fx-stutter".into(),
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Instrument { index: 0 },
        ));
    }
    if mode >= 3 {
        rows.push((
            "fx-pitch".into(),
            "pitch_shift".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Instrument { index: 1 },
        ));
    }
    if mode >= 4 && rows.len() < 4 {
        rows.push((
            "fx-freeze".into(),
            "freeze".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Instrument { index: 2 },
        ));
    }
    rows
}

fn distribute(total: usize, start: usize, len: usize) -> [usize; INSTRUMENT_SLOT_COUNT] {
    let mut counts = [0; INSTRUMENT_SLOT_COUNT];
    if len == 0 {
        return counts;
    }
    let base = total / len;
    let extra = total % len;
    for idx in 0..len {
        counts[start + idx] = base + usize::from(idx < extra);
    }
    counts
}

fn push_synth_voices(events: &mut Vec<EngineEvent>, slot: usize, count: usize) {
    for idx in 0..count {
        push_note(events, slot, 60 + idx as u8);
    }
}

fn push_sample_voices(events: &mut Vec<EngineEvent>, slot: usize, count: usize) {
    for _ in 0..count {
        push_note(events, slot, 36);
    }
}

fn push_note(events: &mut Vec<EngineEvent>, slot: usize, note: u8) {
    events.push(EngineEvent::NoteOn {
        instrument_slot: slot as u8,
        note,
        velocity: 100,
        duration_ms: PROFILE_NOTE_DURATION_MS,
    });
}

fn all_synth_instruments(
    routes: [usize; INSTRUMENT_SLOT_COUNT],
    mixer: Option<MixerConfig>,
) -> InstrumentsConfig {
    instruments_config(
        [
            "synth", "synth", "synth", "synth", "synth", "synth", "synth", "synth",
        ],
        routes,
        mixer,
    )
}

fn all_sample_instruments(
    routes: [usize; INSTRUMENT_SLOT_COUNT],
    mixer: Option<MixerConfig>,
) -> InstrumentsConfig {
    instruments_config(
        [
            "sampler", "sampler", "sampler", "sampler", "sampler", "sampler", "sampler", "sampler",
        ],
        routes,
        mixer,
    )
}

fn mixed_instruments(
    routes: [usize; INSTRUMENT_SLOT_COUNT],
    mixer: Option<MixerConfig>,
) -> InstrumentsConfig {
    instruments_config(
        [
            "synth", "synth", "synth", "synth", "sampler", "sampler", "sampler", "sampler",
        ],
        routes,
        mixer,
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
                kind: (*kind).to_string(),
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

fn route_name(route: usize) -> String {
    if route == 0 {
        "direct".into()
    } else {
        format!("fx_bus_{route}")
    }
}
