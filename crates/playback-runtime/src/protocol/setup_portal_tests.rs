use super::*;
use serde_json::{json, Value};

fn setup_status(
    phase: RuntimeSetupPortalPhase,
    disposition: Option<RuntimeSetupPortalDisposition>,
    portal_suffix: Option<&str>,
    reboot_required: bool,
    error_code: Option<RuntimeSetupPortalErrorCode>,
) -> RuntimeStoreResult {
    RuntimeStoreResult::SetupPortalStatus {
        status: RuntimeSetupPortalStatus {
            phase,
            disposition,
            portal_suffix: portal_suffix.map(str::to_owned),
            transfer: None,
            reboot_required,
            error_code,
        },
    }
}

#[test]
fn setup_portal_effect_has_typed_operation_and_runtime_error_domain() {
    let effect = RuntimePlatformEffect::SetupPortalOpen;
    assert_eq!(
        serde_json::to_value(&effect).unwrap(),
        json!({ "type": "setup_portal_open" })
    );
    assert_eq!(effect.operation(), RuntimeOperation::SetupPortal);
    assert_eq!(effect.error_domain(), RuntimeErrorDomain::Runtime);
    assert_eq!(
        serde_json::to_value(RuntimeStoreResult::OperationSucceeded {
            operation: RuntimeOperation::SetupPortal,
            request_id: Some("setup-portal-1".into()),
            revision: Some(1),
        })
        .unwrap(),
        json!({
            "type": "operation_succeeded",
            "operation": "setup_portal",
            "requestId": "setup-portal-1",
            "revision": 1
        })
    );
    assert_eq!(
        effect.failure_facts("setup portal failed".into()).operation,
        RuntimeOperation::SetupPortal
    );
    assert_eq!(
        effect
            .unsupported_facts("setup portal unavailable".into())
            .code,
        RuntimeErrorCode::Unsupported
    );
}

#[test]
fn setup_portal_status_serializes_and_round_trips_every_phase() {
    let statuses = vec![
        setup_status(
            RuntimeSetupPortalPhase::Starting,
            Some(RuntimeSetupPortalDisposition::Accepted),
            None,
            false,
            None,
        ),
        setup_status(
            RuntimeSetupPortalPhase::Starting,
            Some(RuntimeSetupPortalDisposition::AlreadyRunning),
            None,
            false,
            None,
        ),
        setup_status(
            RuntimeSetupPortalPhase::PortalReady,
            None,
            Some("abcd"),
            false,
            None,
        ),
        setup_status(RuntimeSetupPortalPhase::Finalizing, None, None, false, None),
        setup_status(RuntimeSetupPortalPhase::Succeeded, None, None, false, None),
        setup_status(
            RuntimeSetupPortalPhase::Failed,
            None,
            None,
            false,
            Some(RuntimeSetupPortalErrorCode::InvalidPayload),
        ),
        setup_status(
            RuntimeSetupPortalPhase::Failed,
            None,
            None,
            false,
            Some(RuntimeSetupPortalErrorCode::OperationFailed),
        ),
        setup_status(
            RuntimeSetupPortalPhase::Failed,
            None,
            None,
            false,
            Some(RuntimeSetupPortalErrorCode::Unavailable),
        ),
        setup_status(
            RuntimeSetupPortalPhase::TimedOut,
            None,
            None,
            false,
            Some(RuntimeSetupPortalErrorCode::Unavailable),
        ),
        setup_status(
            RuntimeSetupPortalPhase::Unsupported,
            None,
            None,
            false,
            Some(RuntimeSetupPortalErrorCode::Unsupported),
        ),
    ];
    let expected = vec![
        json!({
            "type": "setup_portal_status",
            "phase": "starting",
            "disposition": "accepted",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "starting",
            "disposition": "already_running",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "portal_ready",
            "portalSuffix": "abcd",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "finalizing",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "succeeded",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "failed",
            "errorCode": "invalid_payload",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "failed",
            "errorCode": "operation_failed",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "failed",
            "errorCode": "unavailable",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "timed_out",
            "errorCode": "unavailable",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "unsupported",
            "errorCode": "unsupported",
            "rebootRequired": false
        }),
    ];

    for (status, expected) in statuses.into_iter().zip(expected) {
        let encoded = serde_json::to_value(&status).unwrap();
        assert_eq!(encoded, expected);
        assert_eq!(
            serde_json::from_value::<RuntimeStoreResult>(encoded.clone()).unwrap(),
            status
        );
        let object = encoded.as_object().unwrap();
        assert!(!object.contains_key("output"));
        assert!(!object.contains_key("password"));
        assert!(!object.contains_key("passphrase"));
        assert!(!object.contains_key("secret"));
        assert!(!object.contains_key("credential"));
    }
}

#[test]
fn setup_portal_transfer_is_typed_and_only_present_during_active_phases() {
    let result = RuntimeStoreResult::SetupPortalStatus {
        status: RuntimeSetupPortalStatus {
            phase: RuntimeSetupPortalPhase::PortalReady,
            disposition: None,
            portal_suffix: Some("abcd".into()),
            transfer: Some(RuntimeSetupPortalTransfer {
                url: "http://192.168.42.1:8081".into(),
                code: "Ab12Cd".into(),
            }),
            reboot_required: false,
            error_code: None,
        },
    };
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(
        encoded["transfer"],
        json!({
            "url": "http://192.168.42.1:8081",
            "code": "Ab12Cd"
        })
    );
    assert_eq!(
        serde_json::from_value::<RuntimeStoreResult>(encoded).unwrap(),
        result
    );

    for phase in [
        RuntimeSetupPortalPhase::Succeeded,
        RuntimeSetupPortalPhase::Failed,
        RuntimeSetupPortalPhase::TimedOut,
        RuntimeSetupPortalPhase::Unsupported,
    ] {
        let error_code = match &phase {
            RuntimeSetupPortalPhase::Succeeded => None,
            RuntimeSetupPortalPhase::Failed => Some(RuntimeSetupPortalErrorCode::OperationFailed),
            RuntimeSetupPortalPhase::TimedOut => Some(RuntimeSetupPortalErrorCode::Unavailable),
            RuntimeSetupPortalPhase::Unsupported => Some(RuntimeSetupPortalErrorCode::Unsupported),
            _ => unreachable!(),
        };
        let status = RuntimeSetupPortalStatus {
            phase,
            disposition: None,
            portal_suffix: None,
            transfer: Some(RuntimeSetupPortalTransfer {
                url: "http://192.168.42.1:8081".into(),
                code: "Ab12Cd".into(),
            }),
            reboot_required: false,
            error_code,
        };
        assert!(status.validate().is_err());
    }
}

#[test]
fn setup_portal_status_errors_are_classified_exhaustively() {
    let cases = [
        (
            RuntimeSetupPortalPhase::Failed,
            None,
            RuntimeErrorCode::OperationFailed,
        ),
        (
            RuntimeSetupPortalPhase::TimedOut,
            None,
            RuntimeErrorCode::Unavailable,
        ),
        (
            RuntimeSetupPortalPhase::Unsupported,
            None,
            RuntimeErrorCode::Unsupported,
        ),
        (
            RuntimeSetupPortalPhase::Failed,
            Some(RuntimeSetupPortalErrorCode::InvalidPayload),
            RuntimeErrorCode::InvalidPayload,
        ),
    ];
    for (phase, error_code, expected_code) in cases {
        let result = setup_status(phase, None, None, false, error_code);
        let facts = result.error_facts().unwrap();
        assert_eq!(facts.domain, RuntimeErrorDomain::Runtime);
        assert_eq!(facts.operation, RuntimeOperation::SetupPortal);
        assert_eq!(facts.code, expected_code);
        assert_eq!(facts.message, None);
    }

    for phase in [
        RuntimeSetupPortalPhase::Starting,
        RuntimeSetupPortalPhase::PortalReady,
        RuntimeSetupPortalPhase::Finalizing,
        RuntimeSetupPortalPhase::Succeeded,
    ] {
        assert!(setup_status(phase, None, None, false, None)
            .error_facts()
            .is_none());
    }
}

#[test]
fn setup_portal_status_preserves_request_identity() {
    let result = setup_status(
        RuntimeSetupPortalPhase::PortalReady,
        None,
        Some("abcd"),
        false,
        None,
    )
    .with_identity("setup-portal-4".into(), Some(12));
    assert_eq!(
        result.success_identity(),
        Some((
            RuntimeOperation::SetupPortal,
            Some("setup-portal-4".into()),
            Some(12)
        ))
    );
    let failed = setup_status(
        RuntimeSetupPortalPhase::Failed,
        None,
        None,
        false,
        Some(RuntimeSetupPortalErrorCode::OperationFailed),
    )
    .with_identity("setup-portal-5".into(), Some(13));
    let facts = failed.error_facts().unwrap();
    assert_eq!(facts.request_id.as_deref(), Some("setup-portal-5"));
    assert_eq!(facts.revision, Some(13));
    assert_eq!(facts.operation, RuntimeOperation::SetupPortal);
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "type": "identified",
            "result": {
                "type": "setup_portal_status",
                "phase": "portal_ready",
                "portalSuffix": "abcd",
                "rebootRequired": false
            },
            "requestId": "setup-portal-4",
            "revision": 12
        })
    );
}

#[test]
fn setup_portal_status_rejects_unknown_or_malformed_values() {
    let unknown_effect = serde_json::from_value::<RuntimePlatformEffect>(json!({
        "type": "setup_portal"
    }));
    assert!(unknown_effect.is_err());

    for value in [
        json!({
            "type": "setup_portal_status",
            "phase": "unknown",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "starting",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "failed",
            "rebootRequired": false,
            "errorCode": "unknown"
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "failed",
            "rebootRequired": false,
            "errorCode": "audio_thread_failed"
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "succeeded"
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "portal_ready",
            "portalSuffix": "123456789012345678901234567890123",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "starting",
            "portalSuffix": "abcd",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "portal_ready",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "portal_ready",
            "portalSuffix": null,
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "portal_ready",
            "disposition": "accepted",
            "portalSuffix": "abcd",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "finalizing",
            "errorCode": "operation_failed",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "succeeded",
            "errorCode": null,
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "failed",
            "errorCode": "unsupported",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "timed_out",
            "errorCode": "operation_failed",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "unsupported",
            "errorCode": "unavailable",
            "rebootRequired": false
        }),
        json!({
            "type": "setup_portal_status",
            "phase": "succeeded",
            "rebootRequired": false,
            "output": "secret-bearing helper output"
        }),
    ] {
        assert!(serde_json::from_value::<RuntimeStoreResult>(value).is_err());
    }

    for suffix in ["éééé", "ABCD", "abc", "abcde", "ab-g"] {
        assert!(serde_json::from_value::<RuntimeStoreResult>(json!({
            "type": "setup_portal_status",
            "phase": "portal_ready",
            "portalSuffix": suffix,
            "rebootRequired": false
        }))
        .is_err());
    }

    let overlong_suffix = "x".repeat(SETUP_PORTAL_SUFFIX_MAX_CHARS + 1);
    assert!(serde_json::to_value(setup_status(
        RuntimeSetupPortalPhase::PortalReady,
        None,
        Some(&overlong_suffix),
        false,
        None,
    ))
    .is_err());
}

#[test]
fn setup_portal_status_uses_no_free_form_root_fields() {
    let value = serde_json::to_value(setup_status(
        RuntimeSetupPortalPhase::Succeeded,
        None,
        None,
        false,
        None,
    ))
    .unwrap();
    let keys = value
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            "phase".to_string(),
            "rebootRequired".to_string(),
            "type".to_string()
        ]
    );
    assert!(matches!(value.get("phase"), Some(Value::String(_))));
}

#[test]
fn setup_portal_status_rejects_true_reboot_required() {
    assert!(serde_json::to_value(setup_status(
        RuntimeSetupPortalPhase::Succeeded,
        None,
        None,
        true,
        None,
    ))
    .is_err());
    assert!(serde_json::from_value::<RuntimeStoreResult>(json!({
        "type": "setup_portal_status",
        "phase": "succeeded",
        "rebootRequired": true
    }))
    .is_err());
}
