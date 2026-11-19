use crate::audio_prep_service::AudioPrepEnqueueResult;
use crate::host_adapter::DesktopPlaybackHostAdapter;
use playback_runtime::{
    RuntimeAdapterError, RuntimeAudioCommand, RuntimeErrorCode, RuntimeErrorDomain,
    RuntimeErrorFacts, RuntimeOperation,
};
use rodio_engine_source::{
    apply_runtime_audio_command, EngineEvent, MomentaryFxTypes, RuntimeAudioCommandHost,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn audio_queue_error(
    error: rodio_engine_source::QueueSendError,
    operation: RuntimeOperation,
) -> RuntimeAdapterError {
    let code = match error {
        rodio_engine_source::QueueSendError::Full { .. } => RuntimeErrorCode::OperationFailed,
        rodio_engine_source::QueueSendError::Disconnected { .. } => {
            RuntimeErrorCode::AudioThreadFailed
        }
    };
    RuntimeAdapterError::from_facts(RuntimeErrorFacts::new(
        RuntimeErrorDomain::Audio,
        code,
        operation,
        Some(error.to_string()),
    ))
}

impl DesktopPlaybackHostAdapter {
    pub(super) fn handle_runtime_audio_command(
        &mut self,
        command: &RuntimeAudioCommand,
    ) -> Result<(), RuntimeAdapterError> {
        apply_runtime_audio_command(self, command)
    }
}

impl RuntimeAudioCommandHost for DesktopPlaybackHostAdapter {
    fn send_audio_event(&mut self, event: EngineEvent) -> Result<(), RuntimeAdapterError> {
        self.send_engine_event(event, RuntimeOperation::AudioCommand)
    }

    fn enqueue_full_config(
        &mut self,
        revision: u64,
        generation: u64,
        request_id: Option<String>,
        config: Value,
    ) -> Result<(), RuntimeAdapterError> {
        prep_result(
            self.audio
                .audio_control
                .enqueue_full_config(revision, generation, request_id, config),
            "full audio configuration",
        )
    }

    fn enqueue_instrument_slot(
        &mut self,
        instrument_slot: usize,
        generation: u64,
        config: Value,
    ) -> Result<(), RuntimeAdapterError> {
        prep_result(
            self.audio
                .audio_control
                .enqueue_instrument_slot(instrument_slot, generation, config),
            "instrument configuration",
        )
    }

    fn enqueue_fx_bus_slot(
        &mut self,
        bus_index: usize,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError> {
        prep_result(
            self.audio
                .audio_control
                .enqueue_fx_bus_slot(bus_index, slot_index, generation, fx_type, params),
            "FX bus configuration",
        )
    }

    fn enqueue_global_fx_slot(
        &mut self,
        slot_index: usize,
        generation: u64,
        fx_type: String,
        params: BTreeMap<String, Value>,
    ) -> Result<(), RuntimeAdapterError> {
        prep_result(
            self.audio
                .audio_control
                .enqueue_global_fx_slot(slot_index, generation, fx_type, params),
            "global FX configuration",
        )
    }

    fn enqueue_sample_preview(
        &mut self,
        instrument_slot: usize,
        path: String,
        velocity: u8,
    ) -> Result<(), RuntimeAdapterError> {
        prep_result(
            self.audio
                .audio_control
                .enqueue_sample_preview(instrument_slot, path, velocity),
            "sample preview",
        )
    }

    fn momentary_fx_types(&mut self) -> &mut MomentaryFxTypes {
        &mut self.momentary_fx_types
    }
}

fn prep_result(result: AudioPrepEnqueueResult, operation: &str) -> Result<(), RuntimeAdapterError> {
    match result {
        AudioPrepEnqueueResult::Accepted => Ok(()),
        AudioPrepEnqueueResult::Full => Err(prep_queue_error(
            playback_runtime::RuntimeErrorCode::OperationFailed,
            format!("{operation} preparation queue is full"),
        )),
        AudioPrepEnqueueResult::Disconnected => Err(prep_queue_error(
            playback_runtime::RuntimeErrorCode::AudioThreadFailed,
            format!("{operation} preparation queue disconnected"),
        )),
    }
}

fn prep_queue_error(
    code: playback_runtime::RuntimeErrorCode,
    message: String,
) -> RuntimeAdapterError {
    RuntimeAdapterError::from_facts(playback_runtime::RuntimeErrorFacts::new(
        playback_runtime::RuntimeErrorDomain::Audio,
        code,
        playback_runtime::RuntimeOperation::AudioCommand,
        Some(message),
    ))
}
