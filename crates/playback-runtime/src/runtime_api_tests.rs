use super::*;

fn identity_layers(result: &RuntimeStoreResult) -> usize {
    match result {
        RuntimeStoreResult::Identified { result, .. } => 1 + identity_layers(result),
        _ => 0,
    }
}

#[test]
fn save_results_have_exactly_one_identity_layer() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let request = runtime.next_platform_request(RuntimePlatformEffect::StoreSaveDefault {
        payload: serde_json::json!({"revision": 7}),
        mode: None,
    });
    let immediate = runtime.identify_result(
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SaveDefaultResult {
                ok: true,
                is_auto: None,
            },
        },
        &request,
    );
    let deferred = HostMessage::RuntimeResult {
        result: RuntimeStoreResult::SaveDefaultResult {
            ok: true,
            is_auto: Some(true),
        }
        .with_identity(request.request_id.clone(), request.revision),
    };

    let results = [immediate, deferred];
    for message in results {
        let HostMessage::RuntimeResult { result } = message else {
            panic!("expected runtime result");
        };
        assert_eq!(identity_layers(&result), 1);
    }
}

#[test]
fn identify_result_normalizes_unwrapped_and_matching_results() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let request = runtime.next_platform_request(RuntimePlatformEffect::SetupPortalOpen);
    let unwrapped = runtime.identify_result(
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SetupPortalStatus {
                status: crate::RuntimeSetupPortalStatus {
                    phase: crate::RuntimeSetupPortalPhase::Starting,
                    disposition: Some(crate::RuntimeSetupPortalDisposition::Accepted),
                    portal_suffix: None,
                    reboot_required: false,
                    error_code: None,
                },
            },
        },
        &request,
    );
    let matching = runtime.identify_result(
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SetupPortalStatus {
                status: crate::RuntimeSetupPortalStatus {
                    phase: crate::RuntimeSetupPortalPhase::Starting,
                    disposition: Some(crate::RuntimeSetupPortalDisposition::Accepted),
                    portal_suffix: None,
                    reboot_required: false,
                    error_code: None,
                },
            }
            .with_identity(request.request_id.clone(), request.revision),
        },
        &request,
    );

    for message in [unwrapped, matching] {
        let HostMessage::RuntimeResult { result } = message else {
            panic!("expected runtime result");
        };
        assert_eq!(identity_layers(&result), 1);
        assert!(matches!(result, RuntimeStoreResult::Identified { .. }));
    }
}

#[test]
fn identify_result_replaces_mismatched_and_nested_results_with_one_typed_error() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let request = runtime.next_platform_request(RuntimePlatformEffect::SetupPortalOpen);
    let mismatched = RuntimeStoreResult::OperationSucceeded {
        operation: RuntimeOperation::SetupPortal,
        request_id: None,
        revision: None,
    }
    .with_identity("other-request".into(), request.revision);
    let nested = RuntimeStoreResult::OperationSucceeded {
        operation: RuntimeOperation::SetupPortal,
        request_id: None,
        revision: None,
    }
    .with_identity(request.request_id.clone(), request.revision)
    .with_identity(request.request_id.clone(), request.revision);

    for result in [mismatched, nested] {
        let HostMessage::RuntimeResult { result } =
            runtime.identify_result(HostMessage::RuntimeResult { result }, &request)
        else {
            panic!("expected runtime result");
        };
        assert_eq!(identity_layers(&result), 1);
        let RuntimeStoreResult::Identified { result, .. } = result else {
            panic!("expected identified result");
        };
        let RuntimeStoreResult::RuntimeFailure { error } = *result else {
            panic!("expected typed runtime failure");
        };
        assert_eq!(error.domain, RuntimeErrorDomain::Serialization);
        assert_eq!(error.code, RuntimeErrorCode::InvalidPayload);
        assert_eq!(error.operation, RuntimeOperation::SetupPortal);
    }
}
