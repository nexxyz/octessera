use cpal::StreamError;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const AUDIO_STREAM_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioStreamRequirement {
    Required,
    Optional,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioStreamStatus {
    Healthy,
    Recovering,
    Terminal,
}

#[derive(Clone)]
pub(crate) struct AudioStreamHealth {
    label: String,
    requirement: AudioStreamRequirement,
    state: Arc<Mutex<AudioStreamHealthState>>,
}

struct AudioStreamHealthState {
    status: AudioStreamStatus,
    last_log: Option<Instant>,
    suppressed: u64,
}

impl AudioStreamHealth {
    pub(crate) fn new(label: String) -> Self {
        Self::with_requirement(label, AudioStreamRequirement::Required)
    }

    pub(crate) fn optional(label: String) -> Self {
        Self::with_requirement(label, AudioStreamRequirement::Optional)
    }

    fn with_requirement(label: String, requirement: AudioStreamRequirement) -> Self {
        Self {
            label,
            requirement,
            state: Arc::new(Mutex::new(AudioStreamHealthState {
                status: AudioStreamStatus::Healthy,
                last_log: None,
                suppressed: 0,
            })),
        }
    }

    #[cfg_attr(feature = "hardware-orange-pi-zero-2w", allow(dead_code))]
    pub(crate) fn is_faulted(&self) -> bool {
        self.status() != AudioStreamStatus::Healthy
    }

    pub(crate) fn status(&self) -> AudioStreamStatus {
        self.state
            .lock()
            .map(|state| state.status)
            .unwrap_or(AudioStreamStatus::Terminal)
    }

    #[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    pub(crate) fn is_terminal(&self) -> bool {
        self.status() == AudioStreamStatus::Terminal
    }

    #[cfg_attr(feature = "hardware-orange-pi-zero-2w", allow(dead_code))]
    pub(crate) fn clear_faulted(&self) {
        self.clear_recoverable_fault();
    }

    pub(crate) fn clear_recoverable_fault(&self) {
        if let Ok(mut state) = self.state.lock() {
            if state.status != AudioStreamStatus::Recovering {
                return;
            }
            state.status = AudioStreamStatus::Healthy;
            state.last_log = None;
            state.suppressed = 0;
        }
    }

    #[cfg_attr(not(feature = "hardware-orange-pi-zero-2w"), allow(dead_code))]
    pub(crate) fn mark_terminal(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.status = AudioStreamStatus::Terminal;
        }
    }

    pub(crate) fn log(&self, error: StreamError) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        if state.status != AudioStreamStatus::Terminal {
            state.status = match (&error, self.requirement) {
                (StreamError::DeviceNotAvailable, AudioStreamRequirement::Optional) => {
                    AudioStreamStatus::Recovering
                }
                _ => AudioStreamStatus::Terminal,
            };
        }
        self.log_rate_limited(&mut state, Instant::now(), error);
    }

    fn log_rate_limited(
        &self,
        state: &mut AudioStreamHealthState,
        now: Instant,
        error: StreamError,
    ) {
        if state
            .last_log
            .is_some_and(|last| now.duration_since(last) < AUDIO_STREAM_ERROR_LOG_INTERVAL)
        {
            state.suppressed = state.suppressed.saturating_add(1);
            return;
        }
        let suppressed = state.suppressed;
        state.last_log = Some(now);
        state.suppressed = 0;
        if suppressed == 0 {
            eprintln!("{} audio stream error: {error}", self.label);
        } else {
            eprintln!(
                "{} audio stream error: {error} ({suppressed} similar errors suppressed)",
                self.label
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_loss_faults_immediately() {
        let health = AudioStreamHealth::new("usb".into());

        health.log(StreamError::DeviceNotAvailable);

        assert!(health.is_faulted());
    }

    #[test]
    fn optional_device_loss_is_recoverable() {
        let health = AudioStreamHealth::optional("UAC2Gadget".into());

        health.log(StreamError::DeviceNotAvailable);

        assert!(health.is_faulted());
        assert!(!health.is_terminal());
        assert_eq!(health.status(), AudioStreamStatus::Recovering);
    }

    #[test]
    fn device_loss_is_terminal_for_orange_recovery() {
        let health = AudioStreamHealth::new("Jack".into());

        health.log(StreamError::DeviceNotAvailable);

        assert!(health.is_terminal());
        assert_eq!(health.status(), AudioStreamStatus::Terminal);
    }

    #[test]
    fn healthy_audio_status_is_explicit() {
        let health = AudioStreamHealth::new("Jack".into());

        assert_eq!(health.status(), AudioStreamStatus::Healthy);
    }

    #[test]
    fn terminal_errors_are_sticky_for_optional_streams() {
        for error in [
            StreamError::DeviceBusy,
            StreamError::Unsupported("unsupported".into()),
            StreamError::Fault("fault".into()),
            StreamError::BackendSpecific {
                err: cpal::BackendSpecificError {
                    description: "unknown".into(),
                },
            },
        ] {
            let health = AudioStreamHealth::optional("USB".into());
            health.log(error);
            assert_eq!(health.status(), AudioStreamStatus::Terminal);
            health.clear_recoverable_fault();
            assert_eq!(health.status(), AudioStreamStatus::Terminal);
        }
    }

    #[test]
    fn recoverable_disconnect_can_be_cleared_but_terminal_cannot() {
        let health = AudioStreamHealth::optional("HDMI".into());
        health.log(StreamError::DeviceNotAvailable);
        assert_eq!(health.status(), AudioStreamStatus::Recovering);
        health.clear_recoverable_fault();
        assert_eq!(health.status(), AudioStreamStatus::Healthy);
        health.mark_terminal();
        health.clear_recoverable_fault();
        assert_eq!(health.status(), AudioStreamStatus::Terminal);
    }

    #[test]
    fn runtime_classification_matrix_covers_required_and_optional_routes() {
        for label in ["Jack", "USB", "HDMI"] {
            let required = AudioStreamHealth::new(label.into());
            required.log(StreamError::DeviceNotAvailable);
            assert_eq!(required.status(), AudioStreamStatus::Terminal);

            let optional = AudioStreamHealth::optional(label.into());
            optional.log(StreamError::DeviceNotAvailable);
            assert_eq!(optional.status(), AudioStreamStatus::Recovering);

            for error in [
                StreamError::DeviceBusy,
                StreamError::Unsupported("unsupported".into()),
                StreamError::Fault("fault".into()),
                StreamError::BackendSpecific {
                    err: cpal::BackendSpecificError {
                        description: "unknown".into(),
                    },
                },
            ] {
                let optional = AudioStreamHealth::optional(label.into());
                optional.log(error);
                assert_eq!(optional.status(), AudioStreamStatus::Terminal);
            }
        }
    }
}
