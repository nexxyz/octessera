use cpal::StreamError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const AUDIO_STREAM_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);
const AUDIO_STREAM_FAULT_WINDOW: Duration = Duration::from_millis(250);
const AUDIO_STREAM_FAULT_ERROR_THRESHOLD: u64 = 2_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioStreamRequirement {
    Required,
    Optional,
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
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
    faulted: Arc<AtomicBool>,
    terminal: Arc<AtomicBool>,
    state: Arc<Mutex<AudioStreamHealthState>>,
}

struct AudioStreamHealthState {
    last_log: Option<Instant>,
    suppressed: u64,
    fault_window_started: Instant,
    fault_window_errors: u64,
    fault_reported: bool,
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
            faulted: Arc::new(AtomicBool::new(false)),
            terminal: Arc::new(AtomicBool::new(false)),
            state: Arc::new(Mutex::new(AudioStreamHealthState {
                last_log: None,
                suppressed: 0,
                fault_window_started: Instant::now(),
                fault_window_errors: 0,
                fault_reported: false,
            })),
        }
    }

    pub(crate) fn is_faulted(&self) -> bool {
        self.faulted.load(Ordering::Relaxed)
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn status(&self) -> AudioStreamStatus {
        if self.is_terminal() {
            AudioStreamStatus::Terminal
        } else if self.is_faulted() {
            AudioStreamStatus::Recovering
        } else {
            AudioStreamStatus::Healthy
        }
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn is_terminal(&self) -> bool {
        self.terminal.load(Ordering::Relaxed)
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn clear_faulted(&self) {
        self.faulted.store(false, Ordering::Relaxed);
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn clear_recoverable_fault(&self) {
        if self.is_terminal() {
            return;
        }
        self.faulted.store(false, Ordering::Relaxed);
        if let Ok(mut state) = self.state.lock() {
            state.last_log = None;
            state.suppressed = 0;
            state.fault_window_started = Instant::now();
            state.fault_window_errors = 0;
            state.fault_reported = false;
        }
    }

    pub(crate) fn mark_terminal(&self) {
        self.terminal.store(true, Ordering::Relaxed);
        self.faulted.store(true, Ordering::Relaxed);
    }

    pub(crate) fn log(&self, error: StreamError) {
        if matches!(error, StreamError::DeviceNotAvailable)
            && self.requirement == AudioStreamRequirement::Required
        {
            self.mark_terminal();
        } else if matches!(error, StreamError::DeviceNotAvailable) {
            self.faulted.store(true, Ordering::Relaxed);
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let now = Instant::now();
        if !matches!(error, StreamError::DeviceNotAvailable) {
            self.update_fault_window(&mut state, now);
        }
        self.log_rate_limited(&mut state, now, error);
    }

    fn update_fault_window(&self, state: &mut AudioStreamHealthState, now: Instant) {
        if now.duration_since(state.fault_window_started) > AUDIO_STREAM_FAULT_WINDOW {
            state.fault_window_started = now;
            state.fault_window_errors = 0;
        }
        state.fault_window_errors = state.fault_window_errors.saturating_add(1);
        if state.fault_window_errors < AUDIO_STREAM_FAULT_ERROR_THRESHOLD {
            return;
        }
        self.faulted.store(true, Ordering::Relaxed);
        if !state.fault_reported {
            state.fault_reported = true;
            eprintln!(
                "{} audio stream faulted after {} errors in {:?}; disabling this sink",
                self.label, state.fault_window_errors, AUDIO_STREAM_FAULT_WINDOW
            );
        }
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

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn optional_device_loss_is_recoverable() {
        let health = AudioStreamHealth::optional("UAC2Gadget".into());

        health.log(StreamError::DeviceNotAvailable);

        assert!(health.is_faulted());
        assert!(!health.is_terminal());
        assert_eq!(health.status(), AudioStreamStatus::Recovering);
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn device_loss_is_terminal_for_orange_recovery() {
        let health = AudioStreamHealth::new("InternalDac".into());

        health.log(StreamError::DeviceNotAvailable);

        assert!(health.is_terminal());
        assert_eq!(health.status(), AudioStreamStatus::Terminal);
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    #[test]
    fn healthy_audio_status_is_explicit() {
        let health = AudioStreamHealth::new("InternalDac".into());

        assert_eq!(health.status(), AudioStreamStatus::Healthy);
    }
}
