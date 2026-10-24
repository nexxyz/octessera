use super::{failure, NativeDefaultCompletion, NativePresetCompletion, PlatformResult};
use playback_runtime::{
    NativeManualSaveRequest, NativeRunner, NativeStoreRequest, RuntimeOperation, RuntimeStoreResult,
};

pub(crate) fn finish_platform_result(
    service: &super::super::PiPlatformService,
    runner: &mut NativeRunner,
    result: PlatformResult,
) -> Option<playback_runtime::HostMessage> {
    match result {
        PlatformResult::Legacy(message) => finish_legacy_result(service, message),
        PlatformResult::NativeDefaultCompletion(completion) => {
            let NativeDefaultCompletion { result, prepared } = completion;
            let current = service.native_default_write()?;
            let identity = match &result {
                RuntimeStoreResult::Identified {
                    request_id,
                    revision,
                    ..
                } => Some((request_id.as_str(), *revision)),
                _ => None,
            };
            if identity.is_some_and(|(request_id, revision)| {
                request_id != current.request_id()
                    || revision.is_some_and(|revision| revision != current.revision())
            }) {
                return None;
            }
            let Some((request_id, revision)) = identity else {
                return malformed_current(
                    service,
                    &current,
                    "native default result has no identity",
                );
            };
            let operation_matches = matches!(
                &result,
                RuntimeStoreResult::Identified { result, .. }
                    if result.operation() == RuntimeOperation::StoreSaveDefault
            );
            if !operation_matches {
                return malformed_current(
                    service,
                    &current,
                    "native default result has the wrong operation",
                );
            }
            if revision.is_none()
                && matches!(
                    &result,
                    RuntimeStoreResult::Identified { result, .. }
                        if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { .. })
                )
            {
                return malformed_current(
                    service,
                    &current,
                    "native default failure is missing its revision",
                );
            }
            let success = matches!(&result, RuntimeStoreResult::Identified { result, .. } if matches!(result.as_ref(), RuntimeStoreResult::SaveDefaultResult { ok: true, .. }));
            if success {
                let Some(prepared) = prepared else {
                    return malformed_current(
                        service,
                        &current,
                        "native default result is missing its prepared payload",
                    );
                };
                let Some(revision) = revision else {
                    return malformed_current(
                        service,
                        &current,
                        "native default result is missing its revision",
                    );
                };
                if !runner.attach_native_default_write_payload(request_id, revision, prepared) {
                    return malformed_current(
                        service,
                        &current,
                        "native default result no longer matches the pending save",
                    );
                }
            } else if !matches!(
                &result,
                RuntimeStoreResult::Identified { result, .. }
                    if matches!(result.as_ref(), RuntimeStoreResult::RuntimeFailure { .. })
            ) {
                return malformed_current(
                    service,
                    &current,
                    "native default worker returned an invalid result",
                );
            }
            service.clear_native_default_write(&current);
            Some(playback_runtime::HostMessage::RuntimeResult { result })
        }
        PlatformResult::NativePresetCompletion(completion) => {
            finish_native_preset_result(service, runner, completion)
        }
    }
}

fn finish_legacy_result(
    service: &super::super::PiPlatformService,
    message: playback_runtime::HostMessage,
) -> Option<playback_runtime::HostMessage> {
    let playback_runtime::HostMessage::RuntimeResult { result } = message else {
        return Some(message);
    };
    let (request_id, revision, operation, failed) = match &result {
        RuntimeStoreResult::Identified {
            result,
            request_id,
            revision,
        } => (
            request_id.clone(),
            *revision,
            result.operation(),
            result.error_facts().is_some(),
        ),
        _ => return Some(playback_runtime::HostMessage::RuntimeResult { result }),
    };
    if !request_id.starts_with("native-preset-") {
        return Some(playback_runtime::HostMessage::RuntimeResult { result });
    }
    let current = service.native_preset_write()?;
    if request_id != current.request.request_id()
        || revision.is_some_and(|revision| revision != current.request.revision())
    {
        return None;
    }
    if revision.is_none() {
        service.clear_native_preset_write(&current);
        return Some(failure(
            &current.request,
            "native preset failure is missing its revision".into(),
        ));
    }
    service.clear_native_preset_write(&current);
    if failed && operation == RuntimeOperation::StoreSavePreset {
        Some(playback_runtime::HostMessage::RuntimeResult { result })
    } else {
        Some(failure(
            &current.request,
            "native preset worker returned an invalid completion".into(),
        ))
    }
}

fn finish_native_preset_result(
    service: &super::super::PiPlatformService,
    runner: &mut NativeRunner,
    completion: NativePresetCompletion,
) -> Option<playback_runtime::HostMessage> {
    let current = service.native_preset_write()?;
    let (request_id, revision) = match &completion.result {
        RuntimeStoreResult::Identified {
            request_id,
            revision,
            ..
        } => (request_id.as_str(), *revision),
        _ => {
            service.clear_native_preset_write(&current);
            return Some(failure(
                &current.request,
                "native preset result has no identity".into(),
            ));
        }
    };
    if request_id != current.request.request_id()
        || revision.is_some_and(|revision| revision != current.request.revision())
    {
        return None;
    }
    let valid_success = matches!(
        &completion.result,
        RuntimeStoreResult::Identified { result, .. }
            if matches!(result.as_ref(), RuntimeStoreResult::SavePresetResult { name, .. }
                if matches!(&current.manual, NativeManualSaveRequest::Preset { name: target, .. } if name == target))
    );
    let Some(revision) = revision else {
        service.clear_native_preset_write(&current);
        return Some(failure(
            &current.request,
            "native preset result is missing its revision".into(),
        ));
    };
    if !valid_success
        || !runner.attach_native_preset_catalog(
            request_id,
            revision,
            completion.names,
            completion.cleanup_error,
        )
    {
        service.clear_native_preset_write(&current);
        return Some(failure(
            &current.request,
            "native preset result does not match its pending save or catalog".into(),
        ));
    }
    service.clear_native_preset_write(&current);
    Some(playback_runtime::HostMessage::RuntimeResult {
        result: completion.result,
    })
}

fn malformed_current(
    service: &super::super::PiPlatformService,
    request: &NativeStoreRequest,
    message: &str,
) -> Option<playback_runtime::HostMessage> {
    service.clear_native_default_write(request);
    Some(failure(request, message.into()))
}
