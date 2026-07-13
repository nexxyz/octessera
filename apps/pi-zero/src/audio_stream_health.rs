use cpal::StreamError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const AUDIO_STREAM_ERROR_LOG_INTERVAL: Duration = Duration::from_secs(1);
const AUDIO_STREAM_FAULT_WINDOW: Duration = Duration::from_millis(250);
const AUDIO_STREAM_FAULT_ERROR_THRESHOLD: u64 = 2_000;

#[derive(Clone)]
pub(crate) struct AudioStreamHealth {
    label: String,
    faulted: Arc<AtomicBool>,
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
        Self {
            label,
            faulted: Arc::new(AtomicBool::new(false)),
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

    pub(crate) fn log(&self, error: StreamError) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let now = Instant::now();
        self.update_fault_window(&mut state, now);
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
