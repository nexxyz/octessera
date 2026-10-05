use crate::audio::AudioService;
use crate::audio_config_parse::prepare_sample_preview;
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult,
};
use std::path::Path;
use std::sync::mpsc::Sender;

pub(crate) struct PreviewRequest {
    pub(crate) sequence: u64,
    pub(crate) instrument_slot: usize,
    pub(crate) path: String,
    pub(crate) velocity: u8,
    pub(crate) samples_dir: std::path::PathBuf,
    pub(crate) preview_token: u64,
}

pub(crate) fn process_request(
    audio: &AudioService,
    instrument_slot: usize,
    path: &str,
    velocity: u8,
    samples_dir: &Path,
    preview_token: u64,
    result_tx: &Sender<HostMessage>,
) {
    if audio
        .preview_generation
        .load(std::sync::atomic::Ordering::Acquire)
        != preview_token
    {
        return;
    }
    let sample_generation = match audio.sample_owner_generation(instrument_slot) {
        Ok(generation) => generation,
        Err(error) => {
            send_result(
                result_tx,
                sample_preview_failure(RuntimeErrorCode::OperationFailed, error),
            );
            return;
        }
    };
    match prepare_sample_preview(
        audio,
        instrument_slot,
        path,
        velocity,
        samples_dir,
        sample_generation,
    ) {
        Ok(event) => {
            match preview_is_current(audio, instrument_slot, preview_token, sample_generation) {
                Ok(true) => match audio.broadcast(event) {
                    Ok(()) => send_result(
                        result_tx,
                        RuntimeStoreResult::OperationSucceeded {
                            operation: RuntimeOperation::SamplePreview,
                            request_id: None,
                            revision: None,
                        },
                    ),
                    Err(error) => send_result(
                        result_tx,
                        sample_preview_failure(RuntimeErrorCode::OperationFailed, error),
                    ),
                },
                Ok(false) => {}
                Err(error) => send_result(
                    result_tx,
                    sample_preview_failure(RuntimeErrorCode::OperationFailed, error),
                ),
            }
        }
        Err(error) => {
            match preview_is_current(audio, instrument_slot, preview_token, sample_generation) {
                Ok(true) => send_result(
                    result_tx,
                    sample_preview_failure(error.code(), error.message()),
                ),
                Ok(false) => {}
                Err(error) => send_result(
                    result_tx,
                    sample_preview_failure(RuntimeErrorCode::OperationFailed, error),
                ),
            }
        }
    }
}

fn preview_is_current(
    audio: &AudioService,
    instrument_slot: usize,
    preview_token: u64,
    sample_generation: u64,
) -> Result<bool, String> {
    if audio
        .preview_generation
        .load(std::sync::atomic::Ordering::Acquire)
        != preview_token
    {
        return Ok(false);
    }
    Ok(audio.sample_owner_generation(instrument_slot)? == sample_generation)
}

fn send_result(result_tx: &Sender<HostMessage>, result: RuntimeStoreResult) {
    let _ = result_tx.send(HostMessage::RuntimeResult { result });
}

fn sample_preview_failure(code: RuntimeErrorCode, message: String) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            playback_runtime::RuntimeErrorDomain::Sample,
            code,
            RuntimeOperation::SamplePreview,
            Some(message),
        ),
    }
}
