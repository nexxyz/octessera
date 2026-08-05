use crate::setup_portal_files::SetupFileError;
use playback_runtime::RuntimeSetupPortalErrorCode;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SetupPortalFailureKind {
    MarkerExists,
    Permission,
    ReceiptTimeout,
    Malformed,
    Unsafe,
    Stale,
    WrongReceipt,
    NonMonotonic,
    Unavailable,
    Random,
    Operation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SetupPortalFailure {
    pub(crate) kind: SetupPortalFailureKind,
}

impl SetupPortalFailure {
    pub(crate) fn from_file(error: SetupFileError) -> Self {
        Self {
            kind: match error {
                SetupFileError::Missing => SetupPortalFailureKind::Unavailable,
                SetupFileError::Exists => SetupPortalFailureKind::MarkerExists,
                SetupFileError::Permission => SetupPortalFailureKind::Permission,
                SetupFileError::Unsafe => SetupPortalFailureKind::Unsafe,
                SetupFileError::Oversized => SetupPortalFailureKind::Malformed,
                SetupFileError::Io => SetupPortalFailureKind::Operation,
                SetupFileError::Published => SetupPortalFailureKind::Operation,
            },
        }
    }

    pub(crate) fn internal() -> Self {
        Self {
            kind: SetupPortalFailureKind::Operation,
        }
    }

    pub(crate) fn random() -> Self {
        Self {
            kind: SetupPortalFailureKind::Random,
        }
    }

    pub(crate) fn receipt_timeout() -> Self {
        Self {
            kind: SetupPortalFailureKind::ReceiptTimeout,
        }
    }

    pub(crate) fn malformed() -> Self {
        Self {
            kind: SetupPortalFailureKind::Malformed,
        }
    }

    pub(crate) fn stale() -> Self {
        Self {
            kind: SetupPortalFailureKind::Stale,
        }
    }

    pub(crate) fn wrong_receipt() -> Self {
        Self {
            kind: SetupPortalFailureKind::WrongReceipt,
        }
    }

    pub(crate) fn non_monotonic() -> Self {
        Self {
            kind: SetupPortalFailureKind::NonMonotonic,
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self {
            kind: SetupPortalFailureKind::Unavailable,
        }
    }

    pub(crate) fn setup_error_code(&self) -> RuntimeSetupPortalErrorCode {
        match self.kind {
            SetupPortalFailureKind::ReceiptTimeout | SetupPortalFailureKind::Unavailable => {
                RuntimeSetupPortalErrorCode::Unavailable
            }
            SetupPortalFailureKind::Malformed
            | SetupPortalFailureKind::Unsafe
            | SetupPortalFailureKind::Stale
            | SetupPortalFailureKind::WrongReceipt
            | SetupPortalFailureKind::NonMonotonic => RuntimeSetupPortalErrorCode::InvalidPayload,
            SetupPortalFailureKind::MarkerExists
            | SetupPortalFailureKind::Permission
            | SetupPortalFailureKind::Random
            | SetupPortalFailureKind::Operation => RuntimeSetupPortalErrorCode::OperationFailed,
        }
    }
}
