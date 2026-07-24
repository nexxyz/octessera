use crate::audio::AudioService;
use crate::audio_config_parse::prepare_sample_preview;
use playback_runtime::{
    HostMessage, RuntimeErrorCode, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult,
};
use std::path::Path;
use std::sync::mpsc::Sender;

pub(crate) struct PreviewRequest {
    pub(crate) instrument_slot: usize,
    pub(crate) path: String,
    pub(crate) velocity: u8,
    pub(crate) samples_dir: std::path::PathBuf,
}

pub(crate) fn process_request(
    audio: &AudioService,
    instrument_slot: usize,
    path: &str,
    velocity: u8,
    samples_dir: &Path,
    result_tx: &Sender<HostMessage>,
) {
    match prepare_sample_preview(audio, instrument_slot, path, velocity, samples_dir) {
        Ok(event) => match audio.broadcast(event) {
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
        Err(error) => send_result(
            result_tx,
            sample_preview_failure(error.code(), error.message()),
        ),
    }
}

pub(crate) fn process_requests(
    audio: &AudioService,
    requests: Vec<PreviewRequest>,
    result_tx: &Sender<HostMessage>,
) {
    for request in requests {
        process_request(
            audio,
            request.instrument_slot,
            &request.path,
            request.velocity,
            &request.samples_dir,
            result_tx,
        );
    }
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
