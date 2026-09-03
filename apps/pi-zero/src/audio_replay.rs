use crate::audio::default_pi_instruments;
use realtime_engine::synth::SampleBankConfig;
use realtime_engine::synth::{prepare_instruments_config, DEFAULT_AUDIO_SAMPLE_RATE};
use rodio_engine_source::{EngineEvent, EngineEventSender};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

pub(crate) fn default_replay_events() -> ReplayCache {
    ReplayCache::default()
}

#[derive(Clone, Default)]
pub(crate) struct ReplayCache {
    audio_config: Option<EngineEvent>,
    sample_banks: Option<Vec<SampleBankConfig>>,
    keyed: BTreeMap<ReplayKey, EngineEvent>,
}

impl ReplayCache {
    pub(crate) fn remember(&mut self, event: &EngineEvent) {
        if !is_replay_event(event) {
            return;
        }
        if let EngineEvent::SetPreparedAudioConfig(config) = event {
            let sample_banks = config
                .sample_banks()
                .map(<[SampleBankConfig]>::to_vec)
                .or_else(|| self.sample_banks.clone());
            if let Some(banks) = sample_banks.as_ref() {
                self.sample_banks = Some(banks.clone());
            }
            self.audio_config = Some(EngineEvent::SetPreparedAudioConfig(
                config.with_sample_banks(sample_banks),
            ));
            return;
        }
        if let EngineEvent::SetPreparedInstruments(config) = event {
            self.audio_config = Some(EngineEvent::SetPreparedInstruments(config.clone()));
            return;
        }
        if let Some(key) = replay_key(event) {
            if merge_fx_bus_mixer_event(&mut self.keyed, &key, event) {
                return;
            }
            self.keyed.insert(key, event.clone());
        }
    }

    fn events(&self) -> Vec<EngineEvent> {
        let mut events = vec![self.audio_config.clone().unwrap_or_else(|| {
            EngineEvent::SetPreparedInstruments(prepare_instruments_config(
                default_pi_instruments(),
                DEFAULT_AUDIO_SAMPLE_RATE,
            ))
        })];
        events.extend(self.keyed.values().cloned());
        events
    }
}

pub(crate) fn replay_to_sink(
    tx: &EngineEventSender,
    replay_events: &Arc<Mutex<ReplayCache>>,
) -> Result<(), String> {
    let events = replay_events
        .lock()
        .map_err(|_| "audio replay cache lock failed".to_string())?;
    for event in events.events() {
        tx.send(event.clone()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn merge_fx_bus_mixer_event(
    keyed: &mut BTreeMap<ReplayKey, EngineEvent>,
    key: &ReplayKey,
    event: &EngineEvent,
) -> bool {
    let EngineEvent::SetFxBusMixer {
        pan_pos,
        volume_pct,
        ..
    } = event
    else {
        return false;
    };
    let Some(EngineEvent::SetFxBusMixer {
        pan_pos: queued_pan,
        volume_pct: queued_volume,
        ..
    }) = keyed.get_mut(key)
    else {
        return false;
    };
    if pan_pos.is_some() {
        *queued_pan = *pan_pos;
    }
    if volume_pct.is_some() {
        *queued_volume = *volume_pct;
    }
    true
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ReplayKey {
    SampleBank(usize),
    DspConfig,
    VoiceStealingMode,
    MasterVolume,
    InstrumentMixer(usize),
    InstrumentSlot(usize),
    FxBusMixer(usize),
    SynthParam(usize, String),
    SampleBankParam(usize, String),
    FxBusSlot(usize, usize),
    GlobalFxSlot(usize),
}

fn replay_key(event: &EngineEvent) -> Option<ReplayKey> {
    match event {
        EngineEvent::SetPreparedSampleBank {
            instrument_slot, ..
        } => Some(ReplayKey::SampleBank(*instrument_slot)),
        EngineEvent::SetDspConfig(_) => Some(ReplayKey::DspConfig),
        EngineEvent::SetVoiceStealingMode(_) => Some(ReplayKey::VoiceStealingMode),
        EngineEvent::SetMasterVolume { .. } => Some(ReplayKey::MasterVolume),
        EngineEvent::SetInstrumentMixer {
            instrument_slot, ..
        } => Some(ReplayKey::InstrumentMixer(*instrument_slot)),
        EngineEvent::SetPreparedInstrumentSlot {
            instrument_slot, ..
        } => Some(ReplayKey::InstrumentSlot(*instrument_slot)),
        EngineEvent::SetFxBusMixer { bus_index, .. } => Some(ReplayKey::FxBusMixer(*bus_index)),
        EngineEvent::SetSynthParam {
            instrument_slot,
            path,
            ..
        } => Some(ReplayKey::SynthParam(*instrument_slot, path.clone())),
        EngineEvent::SetSampleBankParam {
            instrument_slot,
            path,
            ..
        } => Some(ReplayKey::SampleBankParam(*instrument_slot, path.clone())),
        EngineEvent::SetPreparedFxBusSlot {
            bus_index,
            slot_index,
            ..
        } => Some(ReplayKey::FxBusSlot(*bus_index, *slot_index)),
        EngineEvent::SetPreparedGlobalFxSlot { slot_index, .. } => {
            Some(ReplayKey::GlobalFxSlot(*slot_index))
        }
        _ => None,
    }
}

pub(crate) fn is_replay_event(event: &EngineEvent) -> bool {
    !matches!(
        event,
        EngineEvent::AllNotesOff
            | EngineEvent::NoteOn { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
            | EngineEvent::PreviewSample { .. }
            | EngineEvent::PreparedMomentaryFxStart { .. }
            | EngineEvent::MomentaryFxUpdate { .. }
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::ProbeMark { .. }
    )
}

#[cfg(test)]
pub(crate) fn collect_replay_events(cache: &ReplayCache) -> Vec<EngineEvent> {
    use rodio_engine_source::event_queue;

    let (tx, mut rx) = event_queue();
    let cache = Arc::new(Mutex::new(cache.clone()));
    replay_to_sink(&tx, &cache).unwrap();
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

#[cfg(test)]
mod tests {
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
            volume_pct: 70.0
        }));
    }

    #[test]
    fn replay_cache_keeps_current_sample_banks_when_audio_config_omits_them() {
        let mut cache = ReplayCache::default();
        let bank = SampleBankConfig {
            gain_pct: 42.0,
            ..Default::default()
        };
        cache.remember(&EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
            default_pi_instruments(),
            Some(vec![bank.clone()]),
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        )));
        cache.remember(&EngineEvent::SetPreparedAudioConfig(prepare_audio_config(
            default_pi_instruments(),
            None,
            None,
            DEFAULT_AUDIO_SAMPLE_RATE,
        )));
        let replay = collect_replay_events(&cache);
        assert!(replay.iter().any(|event| matches!(
            event,
            EngineEvent::SetPreparedAudioConfig(config)
                if config
                    .sample_banks()
                    .is_some_and(|banks| banks[0].gain_pct == 42.0)
        )));
    }

    #[test]
    fn replay_cache_keys_incremental_config_by_identity() {
        let mut cache = ReplayCache::default();
        cache.remember(&EngineEvent::SetSynthParam {
            instrument_slot: 1,
            path: "osc.mix".to_string(),
            value: 0.25,
        });
        cache.remember(&EngineEvent::SetSynthParam {
            instrument_slot: 1,
            path: "osc.mix".to_string(),
            value: 0.75,
        });
        cache.remember(&EngineEvent::SetSynthParam {
            instrument_slot: 2,
            path: "osc.mix".to_string(),
            value: 0.5,
        });
        let replay = collect_replay_events(&cache);
        assert_eq!(
            replay
                .iter()
                .filter(|event| matches!(event, EngineEvent::SetSynthParam { .. }))
                .count(),
            2
        );
        assert!(replay.iter().any(|event| matches!(
            event,
            EngineEvent::SetSynthParam {
                instrument_slot: 1,
                value,
                ..
            } if *value == 0.75
        )));
    }

    #[test]
    fn replay_cache_keeps_one_current_dsp_config() {
        let mut cache = ReplayCache::default();
        cache.remember(&EngineEvent::SetDspConfig(
            realtime_engine::synth::DspRuntimeConfig {
                worker_warning_threshold: realtime_engine::synth::WorkerWarningThreshold::Percent70,
                bus_idle_threshold: realtime_engine::synth::BusIdleThreshold::Exact,
            },
        ));
        cache.remember(&EngineEvent::SetDspConfig(
            realtime_engine::synth::DspRuntimeConfig::default(),
        ));

        let replay = collect_replay_events(&cache);
        assert_eq!(
            replay
                .iter()
                .filter(|event| matches!(event, EngineEvent::SetDspConfig(_)))
                .count(),
            1
        );
        assert!(replay.iter().any(|event| matches!(
            event,
            EngineEvent::SetDspConfig(config)
                if *config == realtime_engine::synth::DspRuntimeConfig::default()
        )));
    }

    #[test]
    fn replay_cache_merges_fx_bus_mixer_options() {
        let mut cache = ReplayCache::default();
        cache.remember(&EngineEvent::SetFxBusMixer {
            bus_index: 0,
            pan_pos: Some(13),
            volume_pct: None,
        });
        cache.remember(&EngineEvent::SetFxBusMixer {
            bus_index: 0,
            pan_pos: None,
            volume_pct: Some(55.0),
        });
        let replay = collect_replay_events(&cache);
        assert!(replay.iter().any(|event| matches!(
            event,
            EngineEvent::SetFxBusMixer {
                bus_index: 0,
                pan_pos: Some(13),
                volume_pct: Some(55.0),
            }
        )));
    }
}
