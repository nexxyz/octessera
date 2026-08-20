use crate::midi_host::MidiHost;
use crate::setup_portal::start_failure_message;
use playback_runtime::{
    HostMessage, RuntimeAdapterError, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};

use super::{PiPlatformService, PlatformJob, PlatformJobKind};

#[derive(Clone, Copy)]
pub(crate) enum QueueFailureStyle {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    Pi,
    Orange,
}

pub(crate) fn dispatch(
    service: &PiPlatformService,
    request: &RuntimePlatformRequest,
    failure_style: QueueFailureStyle,
) -> Option<Vec<HostMessage>> {
    let result = match &request.effect {
        RuntimePlatformEffect::StoreListPresets => Some(enqueue(
            service,
            request,
            PlatformJobKind::ListPresets,
            failure_style,
            "Preset list".into(),
        )),
        RuntimePlatformEffect::StoreLoadPreset { name } => Some(enqueue(
            service,
            request,
            PlatformJobKind::LoadPreset { name: name.clone() },
            failure_style,
            preset_operation(failure_style, format!("Load {name}"), "Load preset"),
        )),
        RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => Some(enqueue(
            service,
            request,
            PlatformJobKind::SavePreset {
                name: name.clone(),
                payload: payload.clone(),
            },
            failure_style,
            preset_operation(failure_style, format!("Save {name}"), "Save preset"),
        )),
        RuntimePlatformEffect::StoreDeletePreset { name } => Some(enqueue(
            service,
            request,
            PlatformJobKind::DeletePreset { name: name.clone() },
            failure_style,
            preset_operation(failure_style, format!("Delete {name}"), "Delete preset"),
        )),
        RuntimePlatformEffect::StoreSaveBackup { payload } => Some(enqueue(
            service,
            request,
            PlatformJobKind::SaveBackup {
                payload: payload.clone(),
            },
            failure_style,
            "Save backup".into(),
        )),
        RuntimePlatformEffect::SampleListRequest {
            instrument_slot,
            sample_slot,
            dir,
        } => Some(enqueue(
            service,
            request,
            PlatformJobKind::ListSamples {
                instrument_slot: *instrument_slot,
                sample_slot: *sample_slot,
                dir: dir.clone(),
            },
            failure_style,
            "Sample list".into(),
        )),
        RuntimePlatformEffect::SystemInfoRequest => Some(enqueue(
            service,
            request,
            PlatformJobKind::SystemInfo,
            QueueFailureStyle::Orange,
            "System info".into(),
        )),
        RuntimePlatformEffect::UpdateCheck => Some(enqueue(
            service,
            request,
            PlatformJobKind::UpdateCheck,
            QueueFailureStyle::Orange,
            "Update check".into(),
        )),
        RuntimePlatformEffect::UpdateApply => Some(enqueue(
            service,
            request,
            PlatformJobKind::UpdateApply,
            QueueFailureStyle::Orange,
            "Update apply".into(),
        )),
        RuntimePlatformEffect::Rollback => Some(enqueue(
            service,
            request,
            PlatformJobKind::Rollback,
            QueueFailureStyle::Orange,
            "Rollback".into(),
        )),
        RuntimePlatformEffect::SetupPortalOpen => Some(match service.start_setup_portal(request) {
            Ok(status) => vec![status],
            Err(failure) => vec![start_failure_message(request, failure)],
        }),
        _ => None,
    };
    result
}

pub(crate) fn dispatch_midi_effect(
    midi: &mut MidiHost,
    effect: &RuntimePlatformEffect,
) -> Result<Option<RuntimeStoreResult>, RuntimeAdapterError> {
    let result = match effect {
        RuntimePlatformEffect::MidiListOutputsRequest => {
            Some(RuntimeStoreResult::MidiListOutputsResult {
                outputs: midi
                    .list_outputs()
                    .map_err(RuntimeAdapterError::operation_failed)?,
            })
        }
        RuntimePlatformEffect::MidiListInputsRequest => {
            Some(RuntimeStoreResult::MidiListInputsResult {
                inputs: midi
                    .list_inputs()
                    .map_err(RuntimeAdapterError::operation_failed)?,
            })
        }
        RuntimePlatformEffect::MidiSelectOutput { id } => {
            let result = midi.select_output(id.clone());
            Some(RuntimeStoreResult::MidiStatus {
                ok: result.is_ok(),
                message: result.err(),
                selected_out_id: midi.selected_output_id(),
                selected_in_id: midi.selected_input_id(),
            })
        }
        RuntimePlatformEffect::MidiSelectInput { id } => {
            let result = midi.select_input(id.clone());
            Some(RuntimeStoreResult::MidiStatus {
                ok: result.is_ok(),
                message: result.err(),
                selected_out_id: midi.selected_output_id(),
                selected_in_id: midi.selected_input_id(),
            })
        }
        _ => None,
    };
    Ok(result)
}

fn enqueue(
    service: &PiPlatformService,
    request: &RuntimePlatformRequest,
    kind: PlatformJobKind,
    failure_style: QueueFailureStyle,
    operation: String,
) -> Vec<HostMessage> {
    match service.enqueue(PlatformJob::new(request.clone(), kind)) {
        Ok(()) => Vec::new(),
        Err(message) => vec![failure_message(
            request,
            format!(
                "{operation} {}: {message}",
                queue_failure_suffix(failure_style)
            ),
        )],
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn enqueue_job(
    service: &PiPlatformService,
    request: &RuntimePlatformRequest,
    kind: PlatformJobKind,
    failure_style: QueueFailureStyle,
    operation: String,
) -> Vec<HostMessage> {
    enqueue(service, request, kind, failure_style, operation)
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn preset_operation(style: QueueFailureStyle, pi: String, orange: &str) -> String {
    match style {
        QueueFailureStyle::Pi => pi,
        QueueFailureStyle::Orange => orange.into(),
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn preset_operation(_style: QueueFailureStyle, _pi: String, orange: &str) -> String {
    orange.into()
}

fn queue_failure_suffix(style: QueueFailureStyle) -> &'static str {
    match style {
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        QueueFailureStyle::Pi => "queued failed",
        QueueFailureStyle::Orange => "queue failed",
    }
}

fn failure_message(request: &RuntimePlatformRequest, message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: playback_runtime::RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}
