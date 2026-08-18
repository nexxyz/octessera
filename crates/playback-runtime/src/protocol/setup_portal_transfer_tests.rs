use super::*;
use serde_json::json;

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
