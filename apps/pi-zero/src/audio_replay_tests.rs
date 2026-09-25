use super::*;
use realtime_engine::synth::prepare_audio_config;
use realtime_engine::synth::{
    default_synth_config, prepare_instrument_slot_config, FmConfig, InstrumentSlotConfig,
    PluckConfig,
};

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
                fm: None,
                pluck: None,
                drum: None,
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
fn fm_and_pluck_scalar_replay_keeps_latest_per_slot_and_parameter() {
    let mut cache = ReplayCache::default();
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 7,
        config: prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        ),
    });
    for (slot, param, value) in [
        (0, FmParamId::Index, 25.0),
        (0, FmParamId::Index, 50.0),
        (0, FmParamId::AmpGainPct, 80.0),
        (2, FmParamId::Index, 75.0),
    ] {
        cache.remember(&EngineEvent::SetFmParam {
            instrument_slot: slot,
            generation: 7,
            param,
            value,
        });
    }
    for (slot, param, value) in [
        (1, PluckParamId::DecayMs, 300.0),
        (1, PluckParamId::DecayMs, 600.0),
        (1, PluckParamId::BrightnessPct, 65.0),
        (3, PluckParamId::PickPositionPct, 40.0),
    ] {
        cache.remember(&EngineEvent::SetPluckParam {
            instrument_slot: slot,
            generation: 7,
            param,
            value,
        });
    }
    let events = collect_replay_events(&cache);
    let fm = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::SetFmParam {
                instrument_slot,
                generation,
                param,
                value,
            } => Some((*instrument_slot, *generation, *param, *value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let pluck = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::SetPluckParam {
                instrument_slot,
                generation,
                param,
                value,
            } => Some((*instrument_slot, *generation, *param, *value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(fm.len(), 3);
    assert!(fm.contains(&(0, 7, FmParamId::Index, 50.0)));
    assert!(fm.contains(&(0, 7, FmParamId::AmpGainPct, 80.0)));
    assert!(fm.contains(&(2, 7, FmParamId::Index, 75.0)));
    assert_eq!(pluck.len(), 3);
    assert!(pluck.contains(&(1, 7, PluckParamId::DecayMs, 600.0)));
    assert!(pluck.contains(&(1, 7, PluckParamId::BrightnessPct, 65.0)));
    assert!(pluck.contains(&(3, 7, PluckParamId::PickPositionPct, 40.0)));
}

#[test]
fn fm_and_pluck_replay_obeys_slot_and_full_owner_generations() {
    let mut cache = ReplayCache::default();
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 10,
        config: prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        ),
    });
    cache.remember(&EngineEvent::SetFmParam {
        instrument_slot: 0,
        generation: 10,
        param: FmParamId::Index,
        value: 25.0,
    });
    cache.remember(&EngineEvent::SetPluckParam {
        instrument_slot: 1,
        generation: 10,
        param: PluckParamId::DecayMs,
        value: 200.0,
    });
    for (slot, kind, fm, pluck) in [
        (0, "fm", Some(FmConfig::default()), None),
        (1, "pluck", None, Some(PluckConfig::default())),
    ] {
        cache.remember(&EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot: slot,
            generation: 11,
            config: prepare_instrument_slot_config(InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                fm,
                pluck,
                drum: None,
                mixer: None,
            }),
        });
    }
    for event in [
        EngineEvent::SetFmParam {
            instrument_slot: 0,
            generation: 10,
            param: FmParamId::Index,
            value: 50.0,
        },
        EngineEvent::SetPluckParam {
            instrument_slot: 1,
            generation: 10,
            param: PluckParamId::DecayMs,
            value: 300.0,
        },
    ] {
        cache.remember(&event);
    }
    assert!(!collect_replay_events(&cache).iter().any(|event| matches!(
        event,
        EngineEvent::SetFmParam { .. } | EngineEvent::SetPluckParam { .. }
    )));
    cache.remember(&EngineEvent::SetFmParam {
        instrument_slot: 0,
        generation: 11,
        param: FmParamId::Index,
        value: 75.0,
    });
    cache.remember(&EngineEvent::SetPluckParam {
        instrument_slot: 1,
        generation: 11,
        param: PluckParamId::DecayMs,
        value: 500.0,
    });
    let replay = collect_replay_events(&cache);
    assert!(replay.iter().any(|event| matches!(event,
        EngineEvent::SetFmParam { instrument_slot: 0, generation: 11, param: FmParamId::Index, value } if *value == 75.0)));
    assert!(replay.iter().any(|event| matches!(event,
        EngineEvent::SetPluckParam { instrument_slot: 1, generation: 11, param: PluckParamId::DecayMs, value } if *value == 500.0)));
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 12,
        config: prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        ),
    });
    cache.remember(&EngineEvent::SetFmParam {
        instrument_slot: 0,
        generation: 11,
        param: FmParamId::Index,
        value: 85.0,
    });
    cache.remember(&EngineEvent::SetPluckParam {
        instrument_slot: 1,
        generation: 11,
        param: PluckParamId::DecayMs,
        value: 700.0,
    });
    assert!(!collect_replay_events(&cache).iter().any(|event| matches!(
        event,
        EngineEvent::SetFmParam { .. } | EngineEvent::SetPluckParam { .. }
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
            fm: None,
            pluck: None,
            drum: None,
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
