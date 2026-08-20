use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckId {
    Platform,
    Identity,
    Service,
    Readiness,
    Storage,
    SetupStatus,
    OledHandoff,
    AudioRoute,
    InputApi,
    UsbState,
    Artifacts,
}

impl CheckId {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Identity => "identity_profile_contract",
            Self::Service => "service_readiness",
            Self::Readiness => "runtime_readiness_marker",
            Self::Storage => "store_backup_paths",
            Self::SetupStatus => "setup_status_receipt_hygiene",
            Self::OledHandoff => "oled_native_handoff",
            Self::AudioRoute => "audio_route_status",
            Self::InputApi => "physical_input_observation",
            Self::UsbState => "usb_gadget_port_role",
            Self::Artifacts => "artifact_log_collection",
        }
    }
}

pub(crate) const CHECK_ORDER: &[CheckId] = &[
    CheckId::Platform,
    CheckId::Identity,
    CheckId::Service,
    CheckId::Readiness,
    CheckId::Storage,
    CheckId::SetupStatus,
    CheckId::OledHandoff,
    CheckId::AudioRoute,
    CheckId::InputApi,
    CheckId::UsbState,
    CheckId::Artifacts,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CheckStatus {
    Pass,
    Fail,
    Timeout,
    NotRun,
    OperatorRequired,
}

impl CheckStatus {
    pub(crate) const fn is_automated_failure(self) -> bool {
        matches!(self, Self::Fail | Self::Timeout | Self::NotRun)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DiagnosticStatus {
    Pass,
    NotRun,
    OperatorRequired,
    Fail,
}

impl DiagnosticStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::NotRun => "not_run",
            Self::OperatorRequired => "operator_required",
            Self::Fail => "fail",
        }
    }

    pub(crate) fn from_checks(checks: &[EvidenceCheck]) -> Self {
        if checks
            .iter()
            .any(|check| matches!(check.status, CheckStatus::Fail | CheckStatus::Timeout))
        {
            Self::Fail
        } else if checks
            .iter()
            .any(|check| check.status == CheckStatus::NotRun)
        {
            Self::NotRun
        } else if checks
            .iter()
            .any(|check| check.status == CheckStatus::OperatorRequired)
        {
            Self::OperatorRequired
        } else {
            Self::Pass
        }
    }
}

#[derive(Debug)]
pub(crate) struct CheckOutcome {
    pub(crate) status: CheckStatus,
    pub(crate) message: String,
    pub(crate) artifact: String,
    pub(crate) artifact_content: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct EvidenceCheck {
    pub(crate) id: String,
    pub(crate) status: CheckStatus,
    pub(crate) elapsed_ms: u128,
    pub(crate) message: String,
    pub(crate) artifact: String,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct OperatorObservation {
    pub(crate) id: &'static str,
    pub(crate) status: CheckStatus,
    pub(crate) instruction: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct EvidenceReport {
    pub(crate) schema_version: u8,
    pub(crate) board_profile: String,
    pub(crate) compiled_board_profile: String,
    pub(crate) timeout_seconds: u64,
    pub(crate) started_unix_seconds: u64,
    pub(crate) finished_unix_seconds: u64,
    pub(crate) automated_pass: bool,
    pub(crate) overall_status: DiagnosticStatus,
    pub(crate) operator_observations_pending: bool,
    pub(crate) checks: Vec<EvidenceCheck>,
    pub(crate) operator_observations: Vec<OperatorObservation>,
}

#[cfg(test)]
mod tests {
    use super::{CheckId, CheckStatus, CHECK_ORDER};

    #[test]
    fn check_order_keeps_identity_before_runtime_and_artifacts_last() {
        let names = CHECK_ORDER.iter().map(|id| id.as_str()).collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "platform",
                "identity_profile_contract",
                "service_readiness",
                "runtime_readiness_marker",
                "store_backup_paths",
                "setup_status_receipt_hygiene",
                "oled_native_handoff",
                "audio_route_status",
                "physical_input_observation",
                "usb_gadget_port_role",
                "artifact_log_collection",
            ]
        );
        assert_eq!(CHECK_ORDER.last(), Some(&CheckId::Artifacts));
    }

    #[test]
    fn not_run_is_not_an_automated_pass() {
        assert!(CheckStatus::Fail.is_automated_failure());
        assert!(CheckStatus::Timeout.is_automated_failure());
        assert!(CheckStatus::NotRun.is_automated_failure());
        assert!(!CheckStatus::OperatorRequired.is_automated_failure());
    }

    #[test]
    fn overall_status_distinguishes_pass_not_run_operator_and_failure() {
        let check = |status| super::EvidenceCheck {
            id: "check".into(),
            status,
            elapsed_ms: 0,
            message: String::new(),
            artifact: "check.txt".into(),
        };
        assert_eq!(
            super::DiagnosticStatus::from_checks(&[check(CheckStatus::Pass)]),
            super::DiagnosticStatus::Pass
        );
        assert_eq!(
            super::DiagnosticStatus::from_checks(&[check(CheckStatus::NotRun)]),
            super::DiagnosticStatus::NotRun
        );
        assert_eq!(
            super::DiagnosticStatus::from_checks(&[check(CheckStatus::OperatorRequired)]),
            super::DiagnosticStatus::OperatorRequired
        );
        assert_eq!(
            super::DiagnosticStatus::from_checks(&[check(CheckStatus::Fail)]),
            super::DiagnosticStatus::Fail
        );
    }
}
