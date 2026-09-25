use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_fx_bus_slot, prepare_global_fx_slot,
    prepare_instrument_slot_config, DrumConfig, DrumParamId, FmConfig, FmParamId, FxBusConfig,
    FxBusSlotConfig, FxParamId, InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig,
    MixerConfig, PluckConfig, PluckParamId, SampleBankConfig, SampleBankParamId, SynthParamId,
    DEFAULT_PAN_POSITIONS,
};
use std::collections::BTreeMap;

const RATE: u32 = 44_100;

fn full_config() -> realtime_engine::synth::PreparedAudioConfig {
    prepare_audio_config(
        InstrumentsConfig {
            instruments: vec![InstrumentSlotConfig {
                fm: None,
                pluck: None,
                drum: None,
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: None,
            }],
            mixer: Some(MixerConfig {
                buses: vec![FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("none".into())],
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume_pct: 100.0,
                }],
                master: Some(MasterFxConfig { slots: Vec::new() }),
            }),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 100.0,
        },
        Some(vec![SampleBankConfig::default()]),
        None,
        RATE,
    )
}

fn run_sequence(
    replacement: EngineEvent,
    scalar: fn(u64) -> EngineEvent,
    owner: fn(&control_drain::OwnerGenerations) -> u64,
) {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, RATE);

    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 20,
        config: full_config(),
    })
    .unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);
    assert_eq!(owner(&source.owner_generations), 20);

    tx.send(scalar(20)).unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);

    tx.send(replacement).unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);
    assert_eq!(owner(&source.owner_generations), 21);

    tx.send(scalar(21)).unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);

    tx.send(scalar(22)).unwrap();
    assert_eq!(source.drain_control_events().config_events, 0);

    tx.send(EngineEvent::SetPreparedAudioConfig {
        generation: 30,
        config: full_config(),
    })
    .unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);
    assert_eq!(owner(&source.owner_generations), 30);
    assert_eq!(source.drain_control_events().config_events, 0);

    tx.send(scalar(30)).unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);

    tx.send(scalar(29)).unwrap();
    assert_eq!(source.drain_control_events().config_events, 0);
}

fn instrument_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetSynthParam {
        instrument_slot: 0,
        generation,
        param: SynthParamId::AmpGainPct,
        value: 60.0,
    }
}

fn fm_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetFmParam {
        instrument_slot: 0,
        generation,
        param: FmParamId::Index,
        value: 73.0,
    }
}

fn pluck_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetPluckParam {
        instrument_slot: 0,
        generation,
        param: PluckParamId::DecayMs,
        value: 250.0,
    }
}

fn drum_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetDrumParam {
        instrument_slot: 0,
        voice: 3,
        generation,
        param: DrumParamId::DecayMs,
        value: 320.0,
    }
}

#[test]
fn drum_instrument_generation_rejects_stale_voice_scalar_after_replacement() {
    run_sequence(
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot: 0,
            generation: 21,
            config: prepare_instrument_slot_config(InstrumentSlotConfig {
                kind: "drum".into(),
                synth: default_synth_config(),
                fm: None,
                pluck: None,
                drum: Some(DrumConfig::default()),
                mixer: None,
            }),
        },
        drum_scalar,
        instrument_owner,
    );
}

#[test]
fn pluck_instrument_generation_rejects_stale_scalar_after_replacement() {
    run_sequence(
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot: 0,
            generation: 21,
            config: prepare_instrument_slot_config(InstrumentSlotConfig {
                kind: "pluck".into(),
                synth: default_synth_config(),
                fm: None,
                pluck: Some(PluckConfig::default()),
                drum: None,
                mixer: None,
            }),
        },
        pluck_scalar,
        instrument_owner,
    );
}

#[test]
fn fm_instrument_generation_rejects_stale_scalar_after_replacement() {
    run_sequence(
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot: 0,
            generation: 21,
            config: prepare_instrument_slot_config(InstrumentSlotConfig {
                kind: "fm".into(),
                synth: default_synth_config(),
                fm: Some(FmConfig::default()),
                pluck: None,
                drum: None,
                mixer: None,
            }),
        },
        fm_scalar,
        instrument_owner,
    );
}

fn sample_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        generation,
        param: SampleBankParamId::AmpGainPct,
        value: 60.0,
    }
}

fn bus_mixer_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetFxBusMixer {
        bus_index: 0,
        generation,
        pan_pos: None,
        volume_pct: Some(60.0),
    }
}

fn bus_slot_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetFxBusParam {
        bus_index: 0,
        slot_index: 0,
        generation,
        param: FxParamId::MixPct,
        value: 60.0,
    }
}

fn global_scalar(generation: u64) -> EngineEvent {
    EngineEvent::SetGlobalFxParam {
        slot_index: 0,
        generation,
        param: FxParamId::MixPct,
        value: 60.0,
    }
}

fn instrument_owner(generations: &control_drain::OwnerGenerations) -> u64 {
    generations.instrument[0]
}

fn sample_owner(generations: &control_drain::OwnerGenerations) -> u64 {
    generations.sample[0]
}

fn bus_mixer_owner(generations: &control_drain::OwnerGenerations) -> u64 {
    generations.bus_mixer[0]
}

fn bus_slot_owner(generations: &control_drain::OwnerGenerations) -> u64 {
    generations.fx_bus[0][0]
}

fn global_owner(generations: &control_drain::OwnerGenerations) -> u64 {
    generations.global_fx[0]
}

#[test]
fn instrument_generation_sequence_preserves_scalar_parity() {
    run_sequence(
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot: 0,
            generation: 21,
            config: prepare_instrument_slot_config(InstrumentSlotConfig {
                fm: None,
                pluck: None,
                drum: None,
                kind: "synth".into(),
                synth: default_synth_config(),
                mixer: None,
            }),
        },
        instrument_scalar,
        instrument_owner,
    );
}

#[test]
fn instrument_owner_generation_gates_scalar_until_atomic_apply() {
    let (tx, rx) = event_queue();
    let mut source = EngineSource::new(rx, RATE);
    tx.send(EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 21,
        config: prepare_instrument_slot_config(InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        }),
        sample_bank: Some(SampleBankConfig::default()),
    })
    .unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);
    assert_eq!(source.owner_generations.instrument[0], 21);
    assert_eq!(source.owner_generations.sample[0], 21);

    tx.send(instrument_scalar(21)).unwrap();
    assert_eq!(source.drain_control_events().config_events, 1);
}

#[test]
fn sample_generation_sequence_preserves_scalar_parity() {
    run_sequence(
        EngineEvent::SetPreparedSampleBank {
            instrument_slot: 0,
            generation: 21,
            bank: SampleBankConfig::default(),
        },
        sample_scalar,
        sample_owner,
    );
}

#[test]
fn bus_mixer_generation_sequence_preserves_scalar_parity() {
    run_sequence(
        EngineEvent::SetPreparedAudioConfig {
            generation: 21,
            config: full_config(),
        },
        bus_mixer_scalar,
        bus_mixer_owner,
    );
}

#[test]
fn bus_slot_generation_sequence_preserves_scalar_parity() {
    run_sequence(
        EngineEvent::SetPreparedFxBusSlot {
            bus_index: 0,
            slot_index: 0,
            generation: 21,
            config: prepare_fx_bus_slot("none".into(), BTreeMap::new(), RATE),
        },
        bus_slot_scalar,
        bus_slot_owner,
    );
}

#[test]
fn global_generation_sequence_preserves_scalar_parity() {
    run_sequence(
        EngineEvent::SetPreparedGlobalFxSlot {
            slot_index: 0,
            generation: 21,
            config: prepare_global_fx_slot("none".into(), BTreeMap::new()),
        },
        global_scalar,
        global_owner,
    );
}
