use crate::audio::{default_pi_instruments, AudioSink};
use crate::usb_config::UsbAudioOut;
use realtime_engine::synth::SampleBankConfig;
use rodio_engine_source::EngineEvent;
use std::collections::BTreeMap;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub(crate) struct SinkSender {
    pub(crate) sink: AudioSink,
    tx: Sender<EngineEvent>,
}

pub(crate) fn broadcast_event(
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    event: EngineEvent,
) -> Result<(), String> {
    let mut failed = Vec::new();
    let mut guard = txs
        .lock()
        .map_err(|_| "audio sink registry lock failed".to_string())?;
    for sink in guard.iter() {
        if sink.tx.send(event.clone()).is_err() {
            failed.push(sink.sink);
        }
    }
    guard.retain(|sink| !failed.contains(&sink.sink));
    Ok(())
}

pub(crate) fn register_sink(
    txs: &Arc<Mutex<Vec<SinkSender>>>,
    sink: AudioSink,
    tx: Sender<EngineEvent>,
) {
    if let Ok(mut txs) = txs.lock() {
        txs.retain(|entry| entry.sink != sink);
        txs.push(SinkSender { sink, tx });
    }
}

pub(crate) fn remove_sink(txs: &Arc<Mutex<Vec<SinkSender>>>, sink: AudioSink) {
    if let Ok(mut txs) = txs.lock() {
        txs.retain(|entry| entry.sink != sink);
    }
}

pub(crate) fn has_sink(txs: &Arc<Mutex<Vec<SinkSender>>>, sink: AudioSink) -> bool {
    txs.lock()
        .map(|txs| txs.iter().any(|entry| entry.sink == sink))
        .unwrap_or(false)
}

pub(crate) fn replay_to_sink(tx: &Sender<EngineEvent>, replay_events: &Arc<Mutex<ReplayCache>>) {
    if let Ok(events) = replay_events.lock() {
        for event in events.events() {
            let _ = tx.send(event.clone());
        }
    }
}

pub(crate) fn default_replay_events() -> ReplayCache {
    ReplayCache::default()
}

pub(crate) fn startup_sinks(audio_out: UsbAudioOut) -> Vec<AudioSink> {
    match audio_out {
        UsbAudioOut::Jack => vec![AudioSink::Jack],
        UsbAudioOut::Usb => Vec::new(),
        UsbAudioOut::Both => vec![AudioSink::Jack],
    }
}

pub(crate) fn recovery_enabled(audio_out: UsbAudioOut) -> bool {
    matches!(audio_out, UsbAudioOut::Usb | UsbAudioOut::Both)
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
        if let EngineEvent::SetSampleBanks(banks) = event {
            self.sample_banks = Some(banks.clone());
        }
        if let EngineEvent::SetAudioConfig { sample_banks, .. } = event {
            if let Some(banks) = sample_banks {
                self.sample_banks = Some(banks.clone());
            }
            self.audio_config = Some(self.materialized_audio_config(event));
            return;
        }
        if let Some(key) = replay_key(event) {
            self.keyed.insert(key, event.clone());
        }
    }

    fn events(&self) -> Vec<EngineEvent> {
        let mut events = vec![self
            .audio_config
            .clone()
            .unwrap_or_else(|| EngineEvent::SetInstruments(default_pi_instruments()))];
        events.extend(self.keyed.values().cloned());
        events
    }

    fn materialized_audio_config(&self, event: &EngineEvent) -> EngineEvent {
        let EngineEvent::SetAudioConfig {
            instruments,
            sample_banks,
            voice_stealing_mode,
        } = event
        else {
            return event.clone();
        };
        EngineEvent::SetAudioConfig {
            instruments: instruments.clone(),
            sample_banks: sample_banks.clone().or_else(|| self.sample_banks.clone()),
            voice_stealing_mode: *voice_stealing_mode,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ReplayKey {
    SampleBanks,
    SampleBank(usize),
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
        EngineEvent::SetSampleBanks(_) => Some(ReplayKey::SampleBanks),
        EngineEvent::SetSampleBank {
            instrument_slot, ..
        } => Some(ReplayKey::SampleBank(*instrument_slot)),
        EngineEvent::SetVoiceStealingMode(_) => Some(ReplayKey::VoiceStealingMode),
        EngineEvent::SetMasterVolume { .. } => Some(ReplayKey::MasterVolume),
        EngineEvent::SetInstrumentMixer {
            instrument_slot, ..
        } => Some(ReplayKey::InstrumentMixer(*instrument_slot)),
        EngineEvent::SetInstrumentSlot {
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
        EngineEvent::SetFxBusSlot {
            bus_index,
            slot_index,
            ..
        } => Some(ReplayKey::FxBusSlot(*bus_index, *slot_index)),
        EngineEvent::SetGlobalFxSlot { slot_index, .. } => {
            Some(ReplayKey::GlobalFxSlot(*slot_index))
        }
        _ => None,
    }
}

pub(crate) fn is_replay_event(event: &EngineEvent) -> bool {
    !matches!(
        event,
        EngineEvent::NoteOn { .. }
            | EngineEvent::NoteOff { .. }
            | EngineEvent::Cc { .. }
            | EngineEvent::PreviewSample { .. }
            | EngineEvent::MomentaryFxStart { .. }
            | EngineEvent::MomentaryFxUpdate { .. }
            | EngineEvent::MomentaryFxStop { .. }
            | EngineEvent::ProbeMark { .. }
    )
}

pub(crate) fn usb_uses_recording_tap(audio_out: UsbAudioOut) -> bool {
    audio_out == UsbAudioOut::Usb
}

#[cfg(test)]
pub(crate) fn collect_replay_events(cache: &ReplayCache) -> Vec<EngineEvent> {
    use std::sync::mpsc;

    let (tx, rx) = mpsc::channel();
    let cache = Arc::new(Mutex::new(cache.clone()));
    replay_to_sink(&tx, &cache);
    rx.try_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::audio_sinks;

    #[test]
    fn audio_sink_plan_preserves_desired_usb_modes() {
        assert_eq!(audio_sinks(UsbAudioOut::Jack), vec![AudioSink::Jack]);
        assert_eq!(audio_sinks(UsbAudioOut::Usb), vec![AudioSink::Usb]);
        assert_eq!(
            audio_sinks(UsbAudioOut::Both),
            vec![AudioSink::Jack, AudioSink::Usb]
        );
    }

    #[test]
    fn usb_recording_tap_policy_keeps_single_callback_owner() {
        assert!(usb_uses_recording_tap(UsbAudioOut::Usb));
        assert!(!usb_uses_recording_tap(UsbAudioOut::Both));
        assert!(!usb_uses_recording_tap(UsbAudioOut::Jack));
    }

    #[test]
    fn startup_and_recovery_plan_keeps_usb_mode_usb_only() {
        assert_eq!(startup_sinks(UsbAudioOut::Jack), vec![AudioSink::Jack]);
        assert!(!recovery_enabled(UsbAudioOut::Jack));
        assert_eq!(startup_sinks(UsbAudioOut::Both), vec![AudioSink::Jack]);
        assert!(recovery_enabled(UsbAudioOut::Both));
        assert_eq!(startup_sinks(UsbAudioOut::Usb), Vec::<AudioSink>::new());
        assert!(recovery_enabled(UsbAudioOut::Usb));
    }

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
        let bank = realtime_engine::synth::SampleBankConfig {
            gain_pct: 42.0,
            ..Default::default()
        };
        cache.remember(&EngineEvent::SetAudioConfig {
            instruments: default_pi_instruments(),
            sample_banks: Some(vec![bank.clone()]),
            voice_stealing_mode: None,
        });
        cache.remember(&EngineEvent::SetAudioConfig {
            instruments: default_pi_instruments(),
            sample_banks: None,
            voice_stealing_mode: None,
        });
        let replay = collect_replay_events(&cache);
        assert!(replay.iter().any(|event| matches!(
            event,
            EngineEvent::SetAudioConfig {
                sample_banks: Some(banks),
                ..
            } if banks[0].gain_pct == 42.0
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
}
