use crate::setup_portal_files::SetupFileError;
use playback_runtime::RuntimeSetupPortalErrorCode;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum SetupPortalFailureKind {
    AlreadyRunning,
    Permission,
    Malformed,
    Unsafe,
    Unavailable,
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
                SetupFileError::Exists => SetupPortalFailureKind::AlreadyRunning,
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

    pub(crate) fn already_running() -> Self {
        Self {
            kind: SetupPortalFailureKind::AlreadyRunning,
        }
    }

    pub(crate) fn malformed() -> Self {
        Self {
            kind: SetupPortalFailureKind::Malformed,
        }
    }

    pub(crate) fn unavailable() -> Self {
        Self {
            kind: SetupPortalFailureKind::Unavailable,
        }
    }

    pub(crate) fn setup_error_code(&self) -> RuntimeSetupPortalErrorCode {
        match self.kind {
            SetupPortalFailureKind::Unavailable => RuntimeSetupPortalErrorCode::Unavailable,
            SetupPortalFailureKind::Malformed | SetupPortalFailureKind::Unsafe => {
                RuntimeSetupPortalErrorCode::InvalidPayload
            }
            SetupPortalFailureKind::AlreadyRunning
            | SetupPortalFailureKind::Permission
            | SetupPortalFailureKind::Operation => RuntimeSetupPortalErrorCode::OperationFailed,
        }
    }

    pub(crate) fn is_already_running(&self) -> bool {
        self.kind == SetupPortalFailureKind::AlreadyRunning
    }
}
