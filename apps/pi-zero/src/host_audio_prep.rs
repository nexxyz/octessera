use crate::audio::{AudioControlRequest, AudioService};
use crate::audio_config_parse::{
    parse_audio_config, parse_voice_stealing_mode, sample_banks, sample_signature,
};
use rodio_engine_source::EngineEvent;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, Sender};

pub fn spawn_audio_control_worker(
    rx: Receiver<AudioControlRequest>,
    engine_tx: Sender<EngineEvent>,
    audio: AudioService,
) {
    std::thread::spawn(move || audio_control_loop(rx, engine_tx, audio));
}

fn audio_control_loop(
    rx: Receiver<AudioControlRequest>,
    engine_tx: Sender<EngineEvent>,
    audio: AudioService,
) {
    while let Ok(request) = rx.recv() {
        match request {
            AudioControlRequest::Dynamic(event) => {
                let _ = engine_tx.send(event);
            }
            AudioControlRequest::FullConfig {
                revision,
                config,
                samples_dir,
            } => handle_full_config_request(revision, config, samples_dir, &rx, &engine_tx, &audio),
        }
    }
}

fn handle_full_config_request(
    mut revision: u64,
    mut config: serde_json::Value,
    mut samples_dir: PathBuf,
    rx: &Receiver<AudioControlRequest>,
    engine_tx: &Sender<EngineEvent>,
    audio: &AudioService,
) {
    let mut pending_dynamic = Vec::new();
    drain_pending_requests(
        rx,
        &mut revision,
        &mut config,
        &mut samples_dir,
        &mut pending_dynamic,
    );
    loop {
        let prepared =
            match prepare_audio_config(audio, revision, config.clone(), samples_dir.clone()) {
                Ok(prepared) => prepared,
                Err(error) => {
                    eprintln!("audio config prep failed: {error}");
                    send_dynamic_events(engine_tx, pending_dynamic);
                    return;
                }
            };
        let mut next_revision = revision;
        let mut next_config = config.clone();
        let mut next_samples_dir = samples_dir.clone();
        if drain_pending_requests(
            rx,
            &mut next_revision,
            &mut next_config,
            &mut next_samples_dir,
            &mut pending_dynamic,
        ) {
            revision = next_revision;
            config = next_config;
            samples_dir = next_samples_dir;
            continue;
        }
        apply_prepared_audio_config(audio, engine_tx, prepared);
        send_dynamic_events(engine_tx, pending_dynamic);
        return;
    }
}

fn drain_pending_requests(
    rx: &Receiver<AudioControlRequest>,
    revision: &mut u64,
    config: &mut serde_json::Value,
    samples_dir: &mut PathBuf,
    pending_dynamic: &mut Vec<EngineEvent>,
) -> bool {
    let mut had_full_config = false;
    while let Ok(request) = rx.try_recv() {
        match request {
            AudioControlRequest::FullConfig {
                revision: next_revision,
                config: next_config,
                samples_dir: next_samples_dir,
            } => {
                *revision = next_revision;
                *config = next_config;
                *samples_dir = next_samples_dir;
                had_full_config = true;
                pending_dynamic.clear();
            }
            AudioControlRequest::Dynamic(event) => pending_dynamic.push(event),
        }
    }
    had_full_config
}

struct PreparedAudioConfig {
    event: EngineEvent,
    sample_signature: Option<String>,
}

fn prepare_audio_config(
    audio: &AudioService,
    revision: u64,
    config: serde_json::Value,
    samples_dir: PathBuf,
) -> Result<PreparedAudioConfig, String> {
    let parsed = parse_audio_config(&config)?;
    let next_signature = sample_signature(&parsed.sample_sources);
    let should_update_sample_banks = {
        let current = audio
            .sample_bank_signature
            .lock()
            .map_err(|_| "sample bank signature lock failed".to_string())?;
        *current != next_signature
    };
    if audio.config_revision.load(Ordering::SeqCst) != revision {
        return Err("stale audio config skipped".into());
    }
    let sample_banks = if should_update_sample_banks {
        Some(sample_banks(&parsed.sample_sources, &samples_dir, audio))
    } else {
        None
    };
    if audio.config_revision.load(Ordering::SeqCst) != revision {
        return Err("stale audio config skipped".into());
    }
    Ok(PreparedAudioConfig {
        event: EngineEvent::SetAudioConfig {
            instruments: parsed.instruments,
            sample_banks,
            voice_stealing_mode: parsed
                .voice_stealing_mode
                .as_deref()
                .map(parse_voice_stealing_mode),
        },
        sample_signature: should_update_sample_banks.then_some(next_signature),
    })
}

fn apply_prepared_audio_config(
    audio: &AudioService,
    engine_tx: &Sender<EngineEvent>,
    prepared: PreparedAudioConfig,
) {
    if let Some(signature) = prepared.sample_signature {
        if let Ok(mut current) = audio.sample_bank_signature.lock() {
            *current = signature;
        }
    }
    let _ = engine_tx.send(prepared.event);
}

fn send_dynamic_events(engine_tx: &Sender<EngineEvent>, events: Vec<EngineEvent>) {
    for event in events {
        let _ = engine_tx.send(event);
    }
}
