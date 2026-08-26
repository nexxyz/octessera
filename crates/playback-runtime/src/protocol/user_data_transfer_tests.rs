use super::{
    RuntimeErrorCode, RuntimeOperation, RuntimePlatformEffect, RuntimeStoreResult,
    RuntimeUserDataTransferPhase, RuntimeUserDataTransferStatus,
};
use serde_json::json;

fn ready_status() -> RuntimeUserDataTransferStatus {
    RuntimeUserDataTransferStatus {
        phase: RuntimeUserDataTransferPhase::Ready,
        url: Some("http://192.168.42.1:8081".into()),
        code: Some("Ab2Cd3Ef4G".into()),
        expires_in_seconds: Some(900),
    }
}

#[test]
fn user_data_transfer_status_round_trips_with_exact_wire_fields() {
    let result = RuntimeStoreResult::UserDataTransferStatus {
        status: ready_status(),
    };
    assert_eq!(
        serde_json::to_value(&result).unwrap(),
        json!({
            "type": "user_data_transfer_status",
            "phase": "ready",
            "url": "http://192.168.42.1:8081",
            "code": "Ab2Cd3Ef4G",
            "expiresInSeconds": 900
        })
    );
    assert_eq!(
        serde_json::from_value::<RuntimeStoreResult>(serde_json::to_value(result).unwrap())
            .unwrap(),
        RuntimeStoreResult::UserDataTransferStatus {
            status: ready_status()
        }
    );
}

#[test]
fn user_data_transfer_status_enforces_phase_fields_and_existing_code_policy() {
    for value in [
        json!({
            "type": "user_data_transfer_status",
            "phase": "ready",
            "url": "http://",
            "code": "Ab2Cd3Ef4G",
            "expiresInSeconds": 900
        }),
        json!({
            "type": "user_data_transfer_status",
            "phase": "ready",
            "url": "https://192.168.42.1:8081",
            "code": "Ab2Cd3Ef4G",
            "expiresInSeconds": 900
        }),
        json!({
            "type": "user_data_transfer_status",
            "phase": "ready",
            "url": "http://192.168.42.1:8081",
            "code": "0123456789",
            "expiresInSeconds": 900
        }),
        json!({
            "type": "user_data_transfer_status",
            "phase": "ready",
            "url": "http://192.168.42.1:8081",
            "code": "Ab2Cd3Ef4G",
            "expiresInSeconds": 0
        }),
        json!({
            "type": "user_data_transfer_status",
            "phase": "closed",
            "url": "http://192.168.42.1:8081"
        }),
        json!({
            "type": "user_data_transfer_status",
            "phase": "unsupported",
            "expiresInSeconds": 15
        }),
    ] {
        assert!(serde_json::from_value::<RuntimeStoreResult>(value).is_err());
    }
}

#[test]
fn user_data_transfer_effects_and_unsupported_status_bind_operation_metadata() {
    for effect in [
        RuntimePlatformEffect::UserDataTransferOpen,
        RuntimePlatformEffect::UserDataTransferClose,
    ] {
        assert_eq!(effect.operation(), RuntimeOperation::UserDataTransfer);
        assert_eq!(effect.error_domain(), super::RuntimeErrorDomain::Runtime);
    }
    let unsupported = RuntimeStoreResult::UserDataTransferStatus {
        status: RuntimeUserDataTransferStatus {
            phase: RuntimeUserDataTransferPhase::Unsupported,
            url: None,
            code: None,
            expires_in_seconds: None,
        },
    };
    let facts = unsupported.error_facts().unwrap();
    assert_eq!(facts.code, RuntimeErrorCode::Unsupported);
    assert_eq!(facts.operation, RuntimeOperation::UserDataTransfer);
}
