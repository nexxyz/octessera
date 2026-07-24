use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_momentary_fx_start, FxBusConfig,
    FxBusSlotConfig, InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig,
    MasterFxConfig, MixerConfig, MomentaryFxTarget, VoiceStealingMode, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;

const PROFILE_NOTE_DURATION_MS: u32 = 60_000;

pub fn bus_heavy_events() -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        bus_heavy_instruments(),
        None,
        VoiceStealingMode::None,
        44_100,
    )];
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

pub fn fx_limit_events(bus_slots: usize, momentary: usize, sample_rate: u32) -> Vec<EngineEvent> {
    let mut events = vec![prepared_config(
        fx_limit_instruments(bus_slots),
        Some(crate::dsp_profile::samples::all_sample_banks(sample_rate)),
        VoiceStealingMode::None,
        sample_rate,
    )];
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
    for event in fx_limit_momentary_events(momentary) {
        events.push(event);
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

fn fx_limit_instruments(bus_slots: usize) -> InstrumentsConfig {
    let active_buses = bus_slots.clamp(0, 24).div_ceil(3).max(1);
    InstrumentsConfig {
        instruments: (0..INSTRUMENT_SLOT_COUNT)
            .map(|slot| InstrumentSlotConfig {
                kind: if slot % 2 == 0 { "synth" } else { "sampler" }.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: if bus_slots == 0 {
                        "direct".into()
                    } else {
                        format!("fx_bus_{}", (slot % active_buses) + 1)
                    },
                    pan_pos: slot.min(DEFAULT_PAN_POSITIONS - 1),
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: fx_limit_buses(bus_slots),
            master: Some(master(vec!["compressor", "reverb"])),
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn fx_limit_buses(bus_slots: usize) -> Vec<FxBusConfig> {
    let kinds = [
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
    kinds
        .iter()
        .cycle()
        .take(bus_slots.clamp(0, 24))
        .enumerate()
        .fold(Vec::<Vec<&str>>::new(), |mut buses, (index, kind)| {
            let bus_index = index / 3;
            if buses.len() <= bus_index {
                buses.push(Vec::new());
            }
            buses[bus_index].push(*kind);
            buses
        })
        .into_iter()
        .enumerate()
        .map(|(index, slots)| bus(slots, index + 1))
        .collect()
}

fn fx_limit_momentary_events(momentary: usize) -> Vec<EngineEvent> {
    let specs = [
        (
            "momentary-filter",
            "filter_sweep",
            MomentaryFxTarget::Global,
        ),
        (
            "momentary-pitch",
            "pitch_shift",
            MomentaryFxTarget::Instrument { index: 1 },
        ),
    ];
    specs
        .into_iter()
        .take(momentary.clamp(0, 2))
        .map(|(id, fx_type, target)| {
            EngineEvent::PreparedMomentaryFxStart(
                prepare_momentary_fx_start(
                    id.into(),
                    fx_type.into(),
                    BTreeMap::new(),
                    target,
                    44_100,
                )
                .unwrap(),
            )
        })
        .collect()
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

fn bus(slots: Vec<&str>, pan_pos: usize) -> FxBusConfig {
    FxBusConfig {
        slots: slots
            .into_iter()
            .map(|kind| FxBusSlotConfig::Kind(kind.to_string()))
            .collect(),
        pan_pos,
        volume_pct: 100.0,
    }
}

fn master(slots: Vec<&str>) -> MasterFxConfig {
    MasterFxConfig {
        slots: slots
            .into_iter()
            .take(2)
            .map(|kind| FxBusSlotConfig::Kind(kind.to_string()))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx_limit_buses_pack_three_slots_per_bus_up_to_twenty_four() {
        let buses = fx_limit_buses(24);

        assert_eq!(buses.len(), 8);
        assert!(buses.iter().all(|bus| bus.slots.len() == 3));
    }

    #[test]
    fn fx_limit_buses_preserve_positional_partial_bus_slots() {
        let buses = fx_limit_buses(8);

        assert_eq!(buses.len(), 3);
        assert_eq!(buses[0].slots.len(), 3);
        assert_eq!(buses[1].slots.len(), 3);
        assert_eq!(buses[2].slots.len(), 2);
    }

    #[test]
    fn fx_limit_master_stays_two_slots() {
        let master = master(vec!["compressor", "reverb", "eq"]);

        assert_eq!(master.slots.len(), 2);
    }
}
