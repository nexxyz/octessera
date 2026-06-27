use crate::audio_config::{
    build_audio_slot_configs, decode_sample_file, parse_voice_stealing_mode, sample_bank_signature,
    sample_banks, synth_payload, AudioInstrumentsConfig,
};
use crate::samples::resolve_sample_file;
use crate::types::QueuedAudioEvent;
use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub(crate) struct DesktopAudioControl {
    tx: Sender<AudioControlRequest>,
}

pub(crate) struct DesktopAudioPrepState {
    pub(crate) synth_slots: Arc<Mutex<[bool; INSTRUMENT_SLOT_COUNT]>>,
    pub(crate) sample_cache: Arc<Mutex<HashMap<String, realtime_engine::synth::SampleBuffer>>>,
    pub(crate) sample_bank_signature: Arc<Mutex<String>>,
}

enum AudioControlRequest {
    FullConfig { revision: u64, config: Value },
    Dynamic(QueuedAudioEvent),
}

struct PreparedAudioConfig {
    event: QueuedAudioEvent,
    synth_slots: [bool; INSTRUMENT_SLOT_COUNT],
    sample_signature: Option<String>,
}

pub(crate) fn spawn_desktop_audio_control(
    trigger_tx: Sender<QueuedAudioEvent>,
    state: DesktopAudioPrepState,
) -> DesktopAudioControl {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || audio_control_loop(rx, trigger_tx, state));
    DesktopAudioControl { tx }
}

impl DesktopAudioControl {
    pub(crate) fn enqueue_full_config(&self, revision: u64, config: Value) -> Result<(), String> {
        self.tx
            .send(AudioControlRequest::FullConfig { revision, config })
            .map_err(|e| format!("audio prep queue send failed: {e}"))
    }

    pub(crate) fn enqueue_dynamic(&self, event: QueuedAudioEvent) -> Result<(), String> {
        self.tx
            .send(AudioControlRequest::Dynamic(event))
            .map_err(|e| format!("audio control queue send failed: {e}"))
    }
}

fn audio_control_loop(
    rx: Receiver<AudioControlRequest>,
    trigger_tx: Sender<QueuedAudioEvent>,
    state: DesktopAudioPrepState,
) {
    while let Ok(request) = rx.recv() {
        match request {
            AudioControlRequest::Dynamic(event) => {
                let _ = trigger_tx.send(event);
            }
            AudioControlRequest::FullConfig { revision, config } => {
                handle_full_config_request(revision, config, &rx, &trigger_tx, &state);
            }
        }
    }
}

fn handle_full_config_request(
    mut revision: u64,
    mut config: Value,
    rx: &Receiver<AudioControlRequest>,
    trigger_tx: &Sender<QueuedAudioEvent>,
    state: &DesktopAudioPrepState,
) {
    let mut pending_dynamic = Vec::new();
    drain_pending_requests(rx, &mut revision, &mut config, &mut pending_dynamic);
    loop {
        let prepared = match prepare_full_audio_config(config.clone(), state) {
            Ok(prepared) => prepared,
            Err(error) => {
                eprintln!("audio config prep failed: {error}");
                send_dynamic_events(trigger_tx, pending_dynamic);
                return;
            }
        };
        let mut newer_revision = revision;
        let mut newer_config = config.clone();
        let had_newer = drain_pending_requests(
            rx,
            &mut newer_revision,
            &mut newer_config,
            &mut pending_dynamic,
        );
        if had_newer {
            revision = newer_revision;
            config = newer_config;
            continue;
        }
        apply_prepared_audio_config(prepared, trigger_tx, state);
        send_dynamic_events(trigger_tx, pending_dynamic);
        return;
    }
}

fn drain_pending_requests(
    rx: &Receiver<AudioControlRequest>,
    revision: &mut u64,
    config: &mut Value,
    pending_dynamic: &mut Vec<QueuedAudioEvent>,
) -> bool {
    let mut had_full_config = false;
    while let Ok(request) = rx.try_recv() {
        match request {
            AudioControlRequest::FullConfig {
                revision: next_revision,
                config: next_config,
            } => {
                *revision = next_revision;
                *config = next_config;
                had_full_config = true;
                pending_dynamic.clear();
            }
            AudioControlRequest::Dynamic(event) => pending_dynamic.push(event),
        }
    }
    had_full_config
}

fn prepare_full_audio_config(
    config: Value,
    state: &DesktopAudioPrepState,
) -> Result<PreparedAudioConfig, String> {
    let config = serde_json::from_value::<AudioInstrumentsConfig>(config)
        .map_err(|e| format!("invalid audio config payload: {e}"))?;
    let (next_slots, _) = build_audio_slot_configs(&config.instruments);
    let next_sample_signature = sample_bank_signature(&config);
    let should_update_sample_banks = {
        let current = state
            .sample_bank_signature
            .lock()
            .map_err(|_| "sample bank signature lock failed".to_string())?;
        *current != next_sample_signature
    };
    let next_sample_banks = if should_update_sample_banks {
        Some(sample_banks(&config, resolve_sample_file, |path| {
            if let Ok(cache) = state.sample_cache.lock() {
                if let Some(buffer) = cache.get(path) {
                    return Some(buffer.clone());
                }
            } else {
                return None;
            }
            let buffer = decode_sample_file(path)?;
            if let Ok(mut cache) = state.sample_cache.lock() {
                cache.insert(path.to_string(), buffer.clone());
            }
            Some(buffer)
        }))
    } else {
        None
    };
    let voice_stealing_mode = config
        .voice_stealing_mode
        .as_deref()
        .map(parse_voice_stealing_mode);
    Ok(PreparedAudioConfig {
        event: QueuedAudioEvent::SetAudioConfig {
            instruments: synth_payload(&config),
            sample_banks: next_sample_banks,
            voice_stealing_mode,
        },
        synth_slots: next_slots,
        sample_signature: should_update_sample_banks.then_some(next_sample_signature),
    })
}

fn apply_prepared_audio_config(
    prepared: PreparedAudioConfig,
    trigger_tx: &Sender<QueuedAudioEvent>,
    state: &DesktopAudioPrepState,
) {
    if let Ok(mut slots) = state.synth_slots.lock() {
        *slots = prepared.synth_slots;
    }
    if let Some(signature) = prepared.sample_signature {
        if let Ok(mut current) = state.sample_bank_signature.lock() {
            *current = signature;
        }
    }
    let _ = trigger_tx.send(prepared.event);
}

fn send_dynamic_events(trigger_tx: &Sender<QueuedAudioEvent>, events: Vec<QueuedAudioEvent>) {
    for event in events {
        let _ = trigger_tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn full_config_replays_queued_dynamic_after_prepared_config() {
        let (request_tx, request_rx) = mpsc::channel();
        let (audio_tx, audio_rx) = mpsc::channel();
        let state = test_state();
        request_tx
            .send(AudioControlRequest::Dynamic(
                QueuedAudioEvent::SetMasterVolume { volume_pct: 44.0 },
            ))
            .unwrap();

        handle_full_config_request(1, audio_config(70), &request_rx, &audio_tx, &state);

        assert!(matches!(
            audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            QueuedAudioEvent::SetAudioConfig { instruments, .. } if instruments.master_volume == 70.0
        ));
        assert!(matches!(
            audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            QueuedAudioEvent::SetMasterVolume { volume_pct } if volume_pct == 44.0
        ));
    }

    #[test]
    fn newer_full_config_wins_before_prepare_starts() {
        let (request_tx, request_rx) = mpsc::channel();
        let (audio_tx, audio_rx) = mpsc::channel();
        let state = test_state();
        request_tx
            .send(AudioControlRequest::FullConfig {
                revision: 2,
                config: audio_config(91),
            })
            .unwrap();

        handle_full_config_request(1, audio_config(70), &request_rx, &audio_tx, &state);

        assert!(matches!(
            audio_rx.recv_timeout(Duration::from_secs(1)).unwrap(),
            QueuedAudioEvent::SetAudioConfig { instruments, .. } if instruments.master_volume == 91.0
        ));
        assert!(audio_rx.try_recv().is_err());
    }

    fn test_state() -> DesktopAudioPrepState {
        DesktopAudioPrepState {
            synth_slots: Arc::new(Mutex::new([true; INSTRUMENT_SLOT_COUNT])),
            sample_cache: Arc::new(Mutex::new(HashMap::new())),
            sample_bank_signature: Arc::new(Mutex::new(String::new())),
        }
    }

    fn audio_config(master_volume: u8) -> Value {
        serde_json::json!({
            "masterVolume": master_volume,
            "panPositions": 33,
            "instruments": [{ "type": "synth" }],
            "mixer": { "buses": [], "master": { "slots": [] } }
        })
    }
}
