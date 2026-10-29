use crate::midi_host::MidiHost;
use crate::setup_portal::start_failure_message;
use playback_runtime::{HostMessage, RuntimePlatformEffect, RuntimePlatformRequest};

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

/// MIDI list and select requests answer later, from the connection worker.
pub(crate) fn dispatch_midi_effect_messages(
    midi: &mut MidiHost,
    effect: &RuntimePlatformEffect,
) -> Option<Vec<HostMessage>> {
    midi.request(effect).then(Vec::new)
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
