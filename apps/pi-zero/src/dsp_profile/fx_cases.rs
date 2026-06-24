use realtime_engine::synth::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, VoiceStealingMode,
    DEFAULT_PAN_POSITIONS, INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;

const PROFILE_NOTE_DURATION_MS: u32 = 60_000;

pub fn bus_heavy_events() -> Vec<EngineEvent> {
    let mut events = vec![
        EngineEvent::SetVoiceStealingMode(VoiceStealingMode::None),
        EngineEvent::SetInstruments(bus_heavy_instruments()),
    ];
    for slot in 0..INSTRUMENT_SLOT_COUNT {
        for note in [60, 67] {
            events.push(EngineEvent::NoteOn {
                instrument_slot: slot as u8,
                note,
                velocity: 100,
                duration_ms: PROFILE_NOTE_DURATION_MS,
            });
        }
    }
    events
}

pub fn fx_routes(mode: usize) -> [usize; INSTRUMENT_SLOT_COUNT] {
    match mode {
        1 => [1; INSTRUMENT_SLOT_COUNT],
        2..=4 => std::array::from_fn(|slot| (slot % 4) + 1),
        _ => [0; INSTRUMENT_SLOT_COUNT],
    }
}

pub fn fx_mixer(mode: usize) -> Option<MixerConfig> {
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

fn bus_heavy_instruments() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|slot| InstrumentSlotConfig {
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: format!("fx_bus_{}", (slot % 6) + 1),
                    pan_pos: slot.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: vec![
                bus(vec!["delay"], 1),
                bus(vec!["reverb"], 2),
                bus(vec!["filter_lfo"], 3),
                bus(vec!["chorus"], 4),
                bus(vec!["compressor"], 5),
                bus(vec!["eq"], 6),
            ],
            master: Some(master(vec!["compressor", "eq"])),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn bus(slots: Vec<&str>, pan_pos: usize) -> FxBusConfig {
    FxBusConfig {
        slots: slots
            .into_iter()
            .map(|kind| FxBusSlotConfig::Kind(kind.to_string()))
            .collect(),
        pan_pos,
    }
}

fn master(slots: Vec<&str>) -> MasterFxConfig {
    MasterFxConfig {
        slots: slots
            .into_iter()
            .map(|kind| FxBusSlotConfig::Kind(kind.to_string()))
            .collect(),
    }
}
