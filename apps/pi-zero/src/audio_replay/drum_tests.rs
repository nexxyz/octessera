use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_audio_config, prepare_instrument_slot_config, DrumConfig,
    InstrumentSlotConfig,
};

#[test]
fn drum_hits_are_transient_and_kit_scalars_replay_per_voice_and_param() {
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
    cache.remember(&EngineEvent::DrumHit {
        instrument_slot: 0,
        voice: 2,
        tune_semis: -24,
        velocity: 100,
    });
    for (voice, param, value) in [
        (2, DrumParamId::DecayMs, 120.0),
        (2, DrumParamId::DecayMs, 220.0),
        (3, DrumParamId::DecayMs, 320.0),
        (2, DrumParamId::TonePct, 75.0),
        (0, DrumParamId::AmpGainPct, 60.0),
    ] {
        cache.remember(&EngineEvent::SetDrumParam {
            instrument_slot: 0,
            voice,
            generation: 10,
            param,
            value,
        });
    }
    let events = collect_replay_events(&cache);
    assert!(!events
        .iter()
        .any(|event| matches!(event, EngineEvent::DrumHit { .. })));
    let values = events
        .iter()
        .filter_map(|event| match event {
            EngineEvent::SetDrumParam {
                instrument_slot,
                voice,
                generation,
                param,
                value,
            } => Some((*instrument_slot, *voice, *generation, *param, *value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 4);
    assert!(values.contains(&(0, 2, 10, DrumParamId::DecayMs, 220.0)));
    assert!(values.contains(&(0, 3, 10, DrumParamId::DecayMs, 320.0)));
    assert!(values.contains(&(0, 2, 10, DrumParamId::TonePct, 75.0)));
    assert!(values.contains(&(0, 0, 10, DrumParamId::AmpGainPct, 60.0)));
}

#[test]
fn drum_scalar_replay_respects_slot_and_full_owner_generations() {
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
    cache.remember(&EngineEvent::SetDrumParam {
        instrument_slot: 0,
        voice: 4,
        generation: 10,
        param: DrumParamId::TuneSemis,
        value: 5.0,
    });
    cache.remember(&EngineEvent::SetPreparedInstrumentSlot {
        instrument_slot: 0,
        generation: 11,
        config: prepare_instrument_slot_config(InstrumentSlotConfig {
            kind: "drum".into(),
            synth: default_synth_config(),
            fm: None,
            pluck: None,
            drum: Some(DrumConfig::default()),
            mixer: None,
        }),
    });
    cache.remember(&EngineEvent::SetDrumParam {
        instrument_slot: 0,
        voice: 4,
        generation: 10,
        param: DrumParamId::TuneSemis,
        value: 6.0,
    });
    assert!(!collect_replay_events(&cache)
        .iter()
        .any(|event| matches!(event, EngineEvent::SetDrumParam { .. })));
    cache.remember(&EngineEvent::SetDrumParam {
        instrument_slot: 0,
        voice: 4,
        generation: 11,
        param: DrumParamId::TuneSemis,
        value: 7.0,
    });
    assert!(collect_replay_events(&cache)
        .iter()
        .any(|event| matches!(event,
        EngineEvent::SetDrumParam { instrument_slot: 0, voice: 4, generation: 11,
            param: DrumParamId::TuneSemis, value } if *value == 7.0)));
    cache.remember(&EngineEvent::SetPreparedAudioConfig {
        generation: 12,
        config: prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        ),
    });
    assert!(!collect_replay_events(&cache)
        .iter()
        .any(|event| matches!(event, EngineEvent::SetDrumParam { .. })));
}
