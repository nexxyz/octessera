use super::*;
use realtime_engine::synth::prepare_audio_config;

#[test]
fn replay_events_skip_transient_realtime_messages() {
    assert!(!is_replay_event(&EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 100,
    }));
    assert!(is_replay_event(&EngineEvent::SetMasterVolume {
        generation: 1,
        volume_pct: 70.0,
    }));
}

#[test]
fn accepted_full_invalidates_prior_deltas_and_rebases_generation() {
    let mut cache = ReplayCache::default();
    cache.remember(&EngineEvent::SetMasterVolume {
        generation: 2,
        volume_pct: 20.0,
    });
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 3,
        config: prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        ),
    });
    cache.remember(&EngineEvent::SetMasterVolume {
        generation: 2,
        volume_pct: 30.0,
    });
    cache.remember(&EngineEvent::SetMasterVolume {
        generation: 3,
        volume_pct: 40.0,
    });
    let replay = collect_replay_events(&cache);
    assert!(replay.iter().any(|event| matches!(
        event,
        EngineEvent::SetMasterVolume { generation: 3, volume_pct } if *volume_pct == 40.0
    )));
    assert!(!replay.iter().any(|event| matches!(
        event,
        EngineEvent::SetMasterVolume { volume_pct, .. } if *volume_pct == 20.0 || *volume_pct == 30.0
    )));
}

#[test]
fn repeated_full_retains_equal_generation_deltas() {
    let mut cache = ReplayCache::default();
    let config = prepare_audio_config(
        default_pi_instruments(),
        None,
        None,
        DEFAULT_AUDIO_SAMPLE_RATE,
    );
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 3,
        config: config.clone(),
    });
    cache.remember(&EngineEvent::SetMasterVolume {
        generation: 3,
        volume_pct: 40.0,
    });
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 3,
        config,
    });

    assert!(collect_replay_events(&cache).iter().any(|event| matches!(
        event,
        EngineEvent::SetMasterVolume { generation: 3, volume_pct } if *volume_pct == 40.0
    )));
}

#[test]
fn owner_replacement_drops_old_matching_scalars() {
    let mut cache = ReplayCache::default();
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 1,
        config: prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        ),
    });
    cache.remember(&EngineEvent::SetInstrumentMixer {
        instrument_slot: 0,
        generation: 1,
        volume_pct: Some(12.0),
        pan_pos: None,
    });
    cache.remember(&EngineEvent::SetPreparedInstrumentSlot {
        instrument_slot: 0,
        generation: 2,
        config: realtime_engine::synth::prepare_instrument_slot_config(
            realtime_engine::synth::InstrumentSlotConfig {
                kind: "synth".into(),
                synth: realtime_engine::synth::default_synth_config(),
                mixer: None,
            },
        ),
    });
    let replay = collect_replay_events(&cache);
    assert!(!replay.iter().any(|event| matches!(
        event,
        EngineEvent::SetInstrumentMixer {
            volume_pct: Some(12.0),
            ..
        }
    )));
}

#[test]
fn sample_owner_replacement_drops_old_sample_parameters() {
    let mut cache = ReplayCache::default();
    cache.remember(&EngineEvent::SetPreparedSampleBank {
        instrument_slot: 0,
        generation: 1,
        bank: SampleBankConfig::default(),
    });
    cache.remember(&EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        generation: 1,
        param: SampleBankParamId::TuneSemis,
        value: 3.0,
    });
    cache.remember(&EngineEvent::SetPreparedSampleBank {
        instrument_slot: 0,
        generation: 2,
        bank: SampleBankConfig::default(),
    });
    cache.remember(&EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        generation: 1,
        param: SampleBankParamId::TuneSemis,
        value: 4.0,
    });
    cache.remember(&EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        generation: 2,
        param: SampleBankParamId::TuneSemis,
        value: 5.0,
    });

    let replay = collect_replay_events(&cache);
    assert!(!replay
        .iter()
        .any(|event| matches!(event, EngineEvent::SetSampleBankParam { generation: 1, .. })));
    assert!(replay.iter().any(|event| matches!(
        event,
        EngineEvent::SetSampleBankParam {
            generation: 2,
            value: 5.0,
            ..
        }
    )));
}

#[test]
fn atomic_owner_preserves_sample_bank_across_synth_replacement() {
    let mut cache = ReplayCache::default();
    let config = realtime_engine::synth::prepare_instrument_slot_config(
        realtime_engine::synth::InstrumentSlotConfig {
            kind: "synth".into(),
            synth: realtime_engine::synth::default_synth_config(),
            mixer: None,
        },
    );
    cache.remember(&EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 1,
        config: config.clone(),
        sample_bank: Some(SampleBankConfig::default()),
    });
    cache.remember(&EngineEvent::SetSampleBankParam {
        instrument_slot: 0,
        generation: 1,
        param: SampleBankParamId::TuneSemis,
        value: 3.0,
    });
    cache.remember(&EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 2,
        config: config.clone(),
        sample_bank: None,
    });

    let replay = collect_replay_events(&cache);
    assert_eq!(
        replay
            .iter()
            .filter(|event| matches!(event, EngineEvent::SetPreparedInstrumentOwner { .. }))
            .count(),
        1
    );
    let bank_index = replay
        .iter()
        .position(|event| {
            matches!(
                event,
                EngineEvent::SetPreparedSampleBank { generation: 1, .. }
            )
        })
        .unwrap();
    let owner_index = replay
        .iter()
        .position(|event| {
            matches!(
                event,
                EngineEvent::SetPreparedInstrumentOwner {
                    generation: 2,
                    sample_bank: None,
                    ..
                }
            )
        })
        .unwrap();
    assert!(bank_index < owner_index);
    assert!(replay
        .iter()
        .any(|event| matches!(event, EngineEvent::SetSampleBankParam { generation: 1, .. })));

    cache.remember(&EngineEvent::SetPreparedInstrumentOwner {
        instrument_slot: 0,
        generation: 3,
        config,
        sample_bank: Some(SampleBankConfig::default()),
    });
    let replay = collect_replay_events(&cache);
    assert!(replay.iter().any(|event| matches!(
        event,
        EngineEvent::SetPreparedInstrumentOwner {
            generation: 3,
            sample_bank: Some(_),
            ..
        }
    )));
    assert_eq!(
        replay
            .iter()
            .filter(|event| matches!(event, EngineEvent::SetPreparedSampleBank { .. }))
            .count(),
        0
    );
    assert!(!replay
        .iter()
        .any(|event| matches!(event, EngineEvent::SetSampleBankParam { .. })));

    let config = prepare_audio_config(
        default_pi_instruments(),
        None,
        None,
        DEFAULT_AUDIO_SAMPLE_RATE,
    );
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 3,
        config: config.clone(),
    });
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 3,
        config,
    });
    let replay = collect_replay_events(&cache);
    assert_eq!(
        replay
            .iter()
            .filter(|event| matches!(event, EngineEvent::SetPreparedInstrumentOwner { .. }))
            .count(),
        1
    );
}
