use super::{RuntimeStoreResult, RuntimeUserDataRestorePhase, RuntimeUserDataRestoreStatus};

#[test]
fn user_data_restore_status_has_a_typed_wire_discriminator() {
    let result = RuntimeStoreResult::UserDataRestoreStatus {
        status: RuntimeUserDataRestoreStatus {
            phase: RuntimeUserDataRestorePhase::Restoring,
        },
    }
    .with_identity("restore-1".into(), Some(4));
    let encoded = serde_json::to_value(&result).unwrap();
    assert_eq!(encoded["result"]["type"], "user_data_restore_status");
    assert_eq!(encoded["result"]["phase"], "restoring");
    assert_eq!(encoded["requestId"], "restore-1");
    assert_eq!(
        serde_json::from_value::<RuntimeStoreResult>(encoded).unwrap(),
        result
    );
}
