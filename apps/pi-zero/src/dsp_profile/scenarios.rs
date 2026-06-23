use realtime_engine::synth::{
    default_synth_config, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
    VoiceStealingMode, DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

const PROFILE_NOTE_DURATION_MS: u32 = 60_000;

pub struct ScenarioSpec {
    pub name: String,
    pub events: Vec<EngineEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMode {
    Full,
    Overload,
    Soak,
}

impl ProfileMode {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" => Some(Self::Full),
            "overload" | "steal" | "stealing" => Some(Self::Overload),
            "soak" => Some(Self::Soak),
            _ => None,
        }
    }
}

pub fn profile_scenarios(sample_rate: u32, mode: ProfileMode) -> Vec<ScenarioSpec> {
    match mode {
        ProfileMode::Full => full_scenarios(sample_rate),
        ProfileMode::Overload => overload_scenarios(sample_rate),
        ProfileMode::Soak => soak_scenarios(sample_rate),
    }
}

fn full_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    let mut scenarios = Vec::new();
    scenarios.push(ScenarioSpec {
        name: "baseline_idle".into(),
        events: baseline_events(),
    });

    for voices in [1, 2, 4, 8, 16, 32, 64] {
        scenarios.push(ScenarioSpec {
            name: format!("synth_ramp_{voices}"),
            events: synth_ramp_events(voices),
        });
    }

    for voices in [1, 2, 4, 8, 16, 32, 64] {
        scenarios.push(ScenarioSpec {
            name: format!("sample_ramp_{voices}"),
            events: sample_ramp_events(voices, sample_rate),
        });
    }

    for voices in [4, 8, 16, 32] {
        scenarios.push(ScenarioSpec {
            name: format!("mixed_ramp_{voices}_{voices}"),
            events: mixed_ramp_events(voices, sample_rate),
        });
    }

    for mode in 0..=4 {
        let name = match mode {
            0 => "fx_ramp_none".into(),
            1 => "fx_ramp_1_bus_delay".into(),
            2 => "fx_ramp_4_buses_1_slot".into(),
            3 => "fx_ramp_4_buses_2_slots".into(),
            _ => "fx_ramp_master_global".into(),
        };
        scenarios.push(ScenarioSpec {
            name,
            events: fx_ramp_events(mode, sample_rate),
        });
    }

    for mode in 1..=4 {
        let name = match mode {
            1 => "momentary_filter".into(),
            2 => "momentary_stutter".into(),
            3 => "momentary_pitch_shift".into(),
            _ => "momentary_combined".into(),
        };
        scenarios.push(ScenarioSpec {
            name,
            events: momentary_events(mode, sample_rate),
        });
    }

    scenarios
}

fn overload_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    vec![
        ScenarioSpec {
            name: "synth_one_slot_12_steal".into(),
            events: synth_overload_events(12, 1),
        },
        ScenarioSpec {
            name: "synth_cross_slot_96_steal".into(),
            events: synth_overload_events(96, INSTRUMENT_SLOT_COUNT),
        },
        ScenarioSpec {
            name: "sample_one_slot_12_steal".into(),
            events: sample_overload_events(12, 1, sample_rate),
        },
        ScenarioSpec {
            name: "sample_cross_slot_96_steal".into(),
            events: sample_overload_events(96, INSTRUMENT_SLOT_COUNT, sample_rate),
        },
        ScenarioSpec {
            name: "mixed_cross_slot_48_48_steal".into(),
            events: mixed_overload_events(48, sample_rate),
        },
    ]
}

fn soak_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    vec![
        ScenarioSpec {
            name: "safe_soak_mixed_8_8".into(),
            events: mixed_ramp_events(8, sample_rate),
        },
        ScenarioSpec {
            name: "safe_soak_fx_16".into(),
            events: fx_ramp_events(2, sample_rate),
        },
        ScenarioSpec {
            name: "risky_soak_momentary_combined".into(),
            events: momentary_events(4, sample_rate),
        },
    ]
}

pub fn runtime_step_scenario() -> ScenarioSpec {
    ScenarioSpec {
        name: "runtime_step_default".into(),
        events: Vec::new(),
    }
}

fn baseline_events() -> Vec<EngineEvent> {
    vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Off),
        EngineEvent::SetInstruments(all_synth_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
    ]
}

fn synth_ramp_events(voices: usize) -> Vec<EngineEvent> {
    let mut events = baseline_events();
    for (slot, count) in distribute(voices, 0, INSTRUMENT_SLOT_COUNT)
        .iter()
        .enumerate()
    {
        push_synth_voices(&mut events, slot, *count);
    }
    events
}

fn sample_ramp_events(voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Off),
        EngineEvent::SetInstruments(all_sample_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
        EngineEvent::SetSampleBanks(all_sample_banks(sample_rate)),
    ];
    for (slot, count) in distribute(voices, 0, INSTRUMENT_SLOT_COUNT)
        .iter()
        .enumerate()
    {
        push_sample_voices(&mut events, slot, *count);
    }
    events
}

fn mixed_ramp_events(voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Off),
        EngineEvent::SetInstruments(mixed_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
        EngineEvent::SetSampleBanks(all_sample_banks(sample_rate)),
    ];
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

fn synth_overload_events(voices: usize, slots: usize) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Balanced),
        EngineEvent::SetInstruments(all_synth_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
    ];
    for (slot, count) in distribute(voices, 0, slots).iter().enumerate() {
        push_synth_voices(&mut events, slot, *count);
    }
    events
}

fn sample_overload_events(voices: usize, slots: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Balanced),
        EngineEvent::SetInstruments(all_sample_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
        EngineEvent::SetSampleBanks(all_sample_banks(sample_rate)),
    ];
    for (slot, count) in distribute(voices, 0, slots).iter().enumerate() {
        push_sample_voices(&mut events, slot, *count);
    }
    events
}

fn mixed_overload_events(voices: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Balanced),
        EngineEvent::SetInstruments(mixed_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
        EngineEvent::SetSampleBanks(all_sample_banks(sample_rate)),
    ];
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

fn fx_ramp_events(mode: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mixer = fx_mixer(mode);
    let routes = fx_routes(mode);
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Off),
        EngineEvent::SetInstruments(all_synth_instruments(routes, mixer)),
    ];
    if mode > 0 {
        events.push(EngineEvent::SetSampleBanks(all_sample_banks(sample_rate)));
    }
    for (slot, count) in distribute(16, 0, INSTRUMENT_SLOT_COUNT).iter().enumerate() {
        push_synth_voices(&mut events, slot, *count);
    }
    events
}

fn momentary_events(mode: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::Off),
        EngineEvent::SetInstruments(all_synth_instruments([0; INSTRUMENT_SLOT_COUNT], None)),
        EngineEvent::SetSampleBanks(all_sample_banks(sample_rate)),
    ];
    for (slot, count) in distribute(16, 0, INSTRUMENT_SLOT_COUNT).iter().enumerate() {
        push_synth_voices(&mut events, slot, *count);
    }
    for (id, fx_type, params, target) in momentary_fx_specs(mode) {
        events.push(EngineEvent::MomentaryFxStart {
            id,
            fx_type,
            params,
            target,
        });
    }
    events
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
        events.push(EngineEvent::NoteOn {
            instrument_slot: slot as u8,
            note: 60 + idx as u8,
            velocity: 100,
            duration_ms: PROFILE_NOTE_DURATION_MS,
        });
    }
}

fn push_sample_voices(events: &mut Vec<EngineEvent>, slot: usize, count: usize) {
    for _ in 0..count {
        events.push(EngineEvent::NoteOn {
            instrument_slot: slot as u8,
            note: 36,
            velocity: 100,
            duration_ms: PROFILE_NOTE_DURATION_MS,
        });
    }
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

fn fx_routes(mode: usize) -> [usize; INSTRUMENT_SLOT_COUNT] {
    match mode {
        1 => [1; INSTRUMENT_SLOT_COUNT],
        2..=4 => std::array::from_fn(|slot| (slot % 4) + 1),
        _ => [0; INSTRUMENT_SLOT_COUNT],
    }
}

fn all_sample_banks(sample_rate: u32) -> Vec<SampleBankConfig> {
    (0..INSTRUMENT_SLOT_COUNT)
        .map(|_| sample_bank(sample_rate))
        .collect()
}

fn sample_bank(sample_rate: u32) -> SampleBankConfig {
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: sample_buffer_data().into_boxed_slice().into(),
            channels: 1,
            sample_rate,
        }),
    };
    bank
}

fn sample_buffer_data() -> Vec<f32> {
    let frames = 16_384;
    (0..frames)
        .map(|i| ((i as f32 / 11.0).sin() * 0.2) + ((i as f32 / 37.0).cos() * 0.1))
        .collect()
}

fn fx_mixer(mode: usize) -> Option<MixerConfig> {
    let bus = |slots: Vec<&str>, pan_pos: usize| -> realtime_engine::synth::FxBusConfig {
        realtime_engine::synth::FxBusConfig {
            slots: slots
                .into_iter()
                .map(|kind| realtime_engine::synth::FxBusSlotConfig::Kind(kind.to_string()))
                .collect(),
            pan_pos,
        }
    };
    let master = |slots: Vec<&str>| -> MasterFxConfig {
        MasterFxConfig {
            slots: slots
                .into_iter()
                .map(|kind| realtime_engine::synth::FxBusSlotConfig::Kind(kind.to_string()))
                .collect(),
        }
    };
    match mode {
        0 => None,
        1 => Some(MixerConfig {
            buses: vec![bus(vec!["delay"], DEFAULT_PAN_POSITIONS / 2)],
            master: None,
        }),
        2 => Some(MixerConfig {
            buses: vec![
                bus(vec!["delay"], 1),
                bus(vec!["delay"], 2),
                bus(vec!["delay"], 3),
                bus(vec!["delay"], 4),
            ],
            master: None,
        }),
        3 => Some(MixerConfig {
            buses: vec![
                bus(vec!["delay", "reverb"], 1),
                bus(vec!["delay", "reverb"], 2),
                bus(vec!["delay", "reverb"], 3),
                bus(vec!["delay", "reverb"], 4),
            ],
            master: None,
        }),
        _ => Some(MixerConfig {
            buses: vec![
                bus(vec!["delay", "reverb"], 1),
                bus(vec!["delay", "reverb"], 2),
                bus(vec!["delay", "reverb"], 3),
                bus(vec!["delay", "reverb"], 4),
            ],
            master: Some(master(vec!["compressor", "reverb"])),
        }),
    }
}
