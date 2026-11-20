use super::*;
use serde_json::json;

#[test]
fn a_failed_preset_save_carries_the_preset_name_into_its_error_facts() {
    let save = RuntimePlatformEffect::StoreSavePreset {
        name: "Pad Wash".into(),
        payload: json!({}),
        mode: None,
    };
    let facts = save.failure_facts("disk full".into());
    assert_eq!(facts.preset_name.as_deref(), Some("Pad Wash"));
    assert_eq!(
        facts
            .into_metadata(RuntimeRecovery::RetainLastGood)
            .preset_name
            .as_deref(),
        Some("Pad Wash")
    );
    assert!(RuntimePlatformEffect::StoreListPresets
        .failure_facts("x".into())
        .preset_name
        .is_none());
}

#[test]
fn a_latched_error_logs_its_domain_code_operation_and_full_message() {
    let error = RuntimeErrorMetadata::new(
        RuntimeErrorDomain::Audio,
        RuntimeErrorCode::OperationFailed,
        RuntimeOperation::AudioThread,
        RuntimeRecovery::StopAndSilence,
        Some("device lost".into()),
    );
    assert_eq!(
        error.log_line(),
        "runtime error: domain=audio code=operation_failed operation=audio_thread message=device lost"
    );
    let without_message = RuntimeErrorMetadata {
        message: None,
        ..error
    };
    assert!(without_message.log_line().ends_with("message=-"));
}
