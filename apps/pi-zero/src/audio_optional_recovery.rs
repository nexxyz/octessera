use super::audio_output_open::AudioConstructionConfig;
use super::cpal_audio_output::build_cpal_mirror_stream;
use super::AudioSink;
use crate::audio_route::{set_status, AudioRouteRegistry, AudioRouteStatus, RouteOpenError};
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use rodio_engine_source::PcmMirrorProducer;
use std::sync::Arc;
use std::time::Duration;

const STARTUP_FAULT_GRACE: Duration = Duration::from_millis(250);
const RECOVERY_INTERVAL: Duration = Duration::from_secs(2);
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(10);

struct ManagedStream {
    _stream: super::cpal_audio_output::BuiltAudioStream,
    health: AudioStreamHealth,
}

fn should_retry_open(error: &RouteOpenError) -> bool {
    error.is_waiting()
}

pub(super) struct OptionalRecoveryWorker {
    stop: Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for OptionalRecoveryWorker {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

pub(super) fn spawn(
    construction: AudioConstructionConfig,
    route_registry: AudioRouteRegistry,
    sink: AudioSink,
    mirror_producer: PcmMirrorProducer,
) -> OptionalRecoveryWorker {
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_stop = stop.clone();
    let join = std::thread::spawn(move || {
        let mut managed: Option<ManagedStream> = None;
        while !worker_stop.load(std::sync::atomic::Ordering::Relaxed) {
            match managed
                .as_ref()
                .map(|stream| stream.health.external_status())
            {
                Some(AudioStreamStatus::Healthy) | None => {}
                Some(AudioStreamStatus::Recovering) => {
                    managed = None;
                    set_status(&route_registry, sink, AudioRouteStatus::Waiting);
                }
                Some(AudioStreamStatus::Terminal) => {
                    set_status(&route_registry, sink, AudioRouteStatus::Faulted);
                    return;
                }
            }
            if managed.is_none() {
                match open(construction, sink, mirror_producer.new_consumer()) {
                    Ok(stream) => {
                        set_status(&route_registry, sink, AudioRouteStatus::Active);
                        stream.health.clear_external_fault();
                        managed = Some(stream);
                        eprintln!("{sink:?} audio stream ready");
                    }
                    Err(error) => {
                        set_status(&route_registry, sink, error.status());
                        eprintln!("{sink:?} audio unavailable: {error}");
                        if !should_retry_open(&error) {
                            return;
                        }
                    }
                }
            }
            sleep_until_retry_or_stop(&worker_stop, RECOVERY_INTERVAL);
        }
    });
    OptionalRecoveryWorker {
        stop,
        join: Some(join),
    }
}

fn sleep_until_retry_or_stop(stop: &std::sync::atomic::AtomicBool, duration: Duration) {
    let deadline = std::time::Instant::now() + duration;
    while !stop.load(std::sync::atomic::Ordering::Relaxed) {
        let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
            break;
        };
        std::thread::sleep(remaining.min(STOP_POLL_INTERVAL));
    }
}

fn open(
    construction: AudioConstructionConfig,
    sink: AudioSink,
    consumer: rodio_engine_source::PcmMirrorConsumer,
) -> Result<ManagedStream, RouteOpenError> {
    let health = AudioStreamHealth::optional(format!("{sink:?}"));
    let built = build_cpal_mirror_stream(
        match construction {
            AudioConstructionConfig::OutputBuffer(frames) => frames,
        },
        sink,
        consumer,
        health.clone(),
    )?;
    if let Err(error) = built.play() {
        if let Err(status) = built.teardown() {
            return Err(super::cpal_audio_output::map_shutdown_error(status));
        }
        return Err(super::cpal_audio_output::map_play_stream_error(error));
    }
    if let Err(error) = crate::audio_priority::qualify_callback_scheduler(
        sink.scheduler_label(),
        &built.scheduler,
        super::audio_output_open::CALLBACK_SCHEDULING_STARTUP_TIMEOUT,
    ) {
        if let Err(status) = built.teardown() {
            return Err(RouteOpenError::Fault(format!(
                "{error}; audio worker teardown failed: {status:?}"
            )));
        }
        return Err(RouteOpenError::Fault(error));
    }
    std::thread::sleep(STARTUP_FAULT_GRACE);
    match health.external_status() {
        AudioStreamStatus::Healthy => {}
        AudioStreamStatus::Recovering => return Err(RouteOpenError::Disconnected),
        AudioStreamStatus::Terminal => {
            return Err(RouteOpenError::Fault(format!(
                "{sink:?} audio stream entered a terminal health state"
            )))
        }
    }
    Ok(ManagedStream {
        _stream: built,
        health,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_absent_and_disconnected_open_failures_retry() {
        assert!(should_retry_open(&RouteOpenError::Absent));
        assert!(should_retry_open(&RouteOpenError::Disconnected));
        assert!(!should_retry_open(&RouteOpenError::Busy));
        assert!(!should_retry_open(&RouteOpenError::Unsupported(
            "format".into()
        )));
        assert!(!should_retry_open(&RouteOpenError::Fault("backend".into())));
    }

    #[test]
    fn terminal_optional_open_failure_is_one_attempt() {
        let mut attempts = 0;
        let error = RouteOpenError::Fault("backend".into());
        loop {
            attempts += 1;
            if !should_retry_open(&error) {
                break;
            }
        }
        assert_eq!(attempts, 1);
    }
}
