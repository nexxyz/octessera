use crate::midi_host::MidiHost;
use crate::setup_portal::start_failure_message;
use playback_runtime::{
    HostMessage, RuntimeAdapterError, RuntimePlatformEffect, RuntimePlatformRequest,
    RuntimeStoreResult,
};

use super::{PiPlatformService, PlatformJob, PlatformJobKind};

pub(crate) fn usb_sd_transfer_output_block_reason(
    usb_audio_enabled: bool,
    usb_midi_enabled: bool,
) -> Option<&'static str> {
    if usb_audio_enabled {
        Some("USB SD2 transfer blocked while USB audio out is active")
    } else if usb_midi_enabled {
        Some("USB SD2 transfer blocked while USB MIDI out is enabled")
    } else {
        None
    }
}

pub(crate) fn dispatch(
    service: &PiPlatformService,
    request: &RuntimePlatformRequest,
) -> Option<Vec<HostMessage>> {
    let result = match &request.effect {
        RuntimePlatformEffect::StoreListPresets => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::ListPresets,
            "Preset list".into(),
        )),
        RuntimePlatformEffect::StoreSavePreset { name, payload, .. } => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::SavePreset {
                name: name.clone(),
                payload: payload.clone(),
            },
            format!("Save {name}"),
        )),
        RuntimePlatformEffect::StoreDeletePreset { name } => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::DeletePreset { name: name.clone() },
            format!("Delete {name}"),
        )),
        RuntimePlatformEffect::StoreSaveBackup { payload } => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::SaveBackup {
                payload: payload.clone(),
            },
            "Save backup".into(),
        )),
        RuntimePlatformEffect::SampleListRequest {
            instrument_slot,
            sample_slot,
            dir,
        } => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::ListSamples {
                instrument_slot: *instrument_slot,
                sample_slot: *sample_slot,
                dir: dir.clone(),
            },
            "Sample list".into(),
        )),
        RuntimePlatformEffect::SystemInfoRequest => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::SystemInfo,
            "System info".into(),
        )),
        RuntimePlatformEffect::UpdateCheck => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::UpdateCheck,
            "Update check".into(),
        )),
        RuntimePlatformEffect::UpdateApply => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::UpdateApply,
            "Update apply".into(),
        )),
        RuntimePlatformEffect::Rollback => Some(enqueue_job(
            service,
            request,
            PlatformJobKind::Rollback,
            "Rollback".into(),
        )),
        RuntimePlatformEffect::SetupPortalOpen => Some(match service.start_setup_portal(request) {
            Ok(status) => vec![status],
            Err(failure) => vec![start_failure_message(request, failure)],
        }),
        RuntimePlatformEffect::UserDataTransferOpen => {
            Some(vec![service.open_user_data_transfer(request)])
        }
        RuntimePlatformEffect::UserDataTransferClose => {
            Some(vec![service.close_user_data_transfer(request)])
        }
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
            midi.selection_status(result)
                .map(|(ok, message)| RuntimeStoreResult::MidiStatus {
                    ok,
                    message,
                    selected_out_id: midi.selected_output_id(),
                    selected_in_id: midi.selected_input_id(),
                })
        }
        RuntimePlatformEffect::MidiSelectInput { id } => {
            let result = midi.select_input(id.clone());
            midi.selection_status(result)
                .map(|(ok, message)| RuntimeStoreResult::MidiStatus {
                    ok,
                    message,
                    selected_out_id: midi.selected_output_id(),
                    selected_in_id: midi.selected_input_id(),
                })
        }
        _ => None,
    };
    Ok(result)
}

pub(crate) fn dispatch_midi_effect_messages(
    midi: &mut MidiHost,
    effect: &RuntimePlatformEffect,
) -> Result<Option<Vec<HostMessage>>, RuntimeAdapterError> {
    let midi_selection = matches!(
        effect,
        RuntimePlatformEffect::MidiSelectOutput { .. }
            | RuntimePlatformEffect::MidiSelectInput { .. }
    );
    match dispatch_midi_effect(midi, effect)? {
        Some(result) => Ok(Some(vec![HostMessage::RuntimeResult { result }])),
        None if midi_selection => Ok(Some(Vec::new())),
        None => Ok(None),
    }
}

pub(crate) fn enqueue_job(
    service: &PiPlatformService,
    request: &RuntimePlatformRequest,
    kind: PlatformJobKind,
    operation: String,
) -> Vec<HostMessage> {
    match service.enqueue(PlatformJob::new(request.clone(), kind)) {
        Ok(()) => Vec::new(),
        Err(message) => vec![failure_message(
            request,
            format!("{operation} queue failed: {message}"),
        )],
    }
}

fn failure_message(request: &RuntimePlatformRequest, message: String) -> HostMessage {
    HostMessage::RuntimeResult {
        result: playback_runtime::RuntimeStoreResult::RuntimeFailure {
            error: request.failure_facts(message),
        },
    }
}
