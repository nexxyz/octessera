use super::audio_output_open::AudioConstructionConfig;
use super::cpal_audio_output::build_cpal_mirror_stream;
use super::AudioSink;
use crate::audio_route::{
    set_status, AudioRouteRegistry, AudioRouteStatus, RouteOpenError, SelectedOutputs,
};
use crate::audio_stream_health::{AudioStreamHealth, AudioStreamStatus};
use rodio_engine_source::{PcmMirrorConsumer, PcmMirrorProducer};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

const STARTUP_FAULT_GRACE: Duration = Duration::from_millis(250);
const RECOVERY_INTERVAL: Duration = Duration::from_secs(2);

struct ManagedStream {
    _stream: Box<dyn std::any::Any>,
    health: AudioStreamHealth,
}

type SideOutputOpener =
    Box<dyn Fn(PcmMirrorConsumer) -> Result<ManagedStream, RouteOpenError> + Send>;

fn should_retry_open(error: &RouteOpenError) -> bool {
    error.is_waiting()
}

/// Runs one side output (USB or HDMI) for as long as the app runs: it plays
/// while System selects it, reopens after the device comes back, and stays
/// quiet while the output is off.
pub(super) struct OptionalRecoveryWorker {
    stop: Arc<AtomicBool>,
    selected: Arc<SelectedOutputs>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Drop for OptionalRecoveryWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.selected.wake();
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
    selected: Arc<SelectedOutputs>,
) -> OptionalRecoveryWorker {
    spawn_with_opener(
        route_registry,
        sink,
        mirror_producer,
        selected,
        Box::new(move |consumer| open(construction, sink, consumer)),
    )
}

fn spawn_with_opener(
    route_registry: AudioRouteRegistry,
    sink: AudioSink,
    mirror_producer: PcmMirrorProducer,
    selected: Arc<SelectedOutputs>,
    open: SideOutputOpener,
) -> OptionalRecoveryWorker {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let worker_selected = selected.clone();
    let join = std::thread::spawn(move || {
        run(
            &open,
            &route_registry,
            sink,
            &mirror_producer,
            &worker_selected,
            &worker_stop,
        )
    });
    OptionalRecoveryWorker {
        stop,
        selected,
        join: Some(join),
    }
}

fn run(
    open: &SideOutputOpener,
    route_registry: &AudioRouteRegistry,
    sink: AudioSink,
    mirror_producer: &PcmMirrorProducer,
    selected: &SelectedOutputs,
    stop: &AtomicBool,
) {
    let mut managed: Option<ManagedStream> = None;
    let (mut outputs, mut generation) = selected.get();
    let mut parked = false;
    while !stop.load(Ordering::Acquire) {
        let enabled = AudioSink::selected(outputs).contains(&sink);
        if !enabled {
            mirror_producer.invalidate();
            if managed.take().is_some() {
                eprintln!("{sink:?} audio stopped");
            }
        } else if !parked {
            parked = service(open, route_registry, sink, mirror_producer, &mut managed);
        }
        let timeout = (enabled && !parked).then_some(RECOVERY_INTERVAL);
        let (next_outputs, next_generation) = selected.wait_for_change(generation, timeout, stop);
        if next_generation != generation {
            parked = false;
        }
        outputs = next_outputs;
        generation = next_generation;
    }
}

/// Keeps a selected side output playing; returns true when it hit a fault that
/// only a new selection should retry.
fn service(
    open: &SideOutputOpener,
    route_registry: &AudioRouteRegistry,
    sink: AudioSink,
    mirror_producer: &PcmMirrorProducer,
    managed: &mut Option<ManagedStream>,
) -> bool {
    match managed
        .as_ref()
        .map(|stream| stream.health.external_status())
    {
        Some(AudioStreamStatus::Healthy) => return false,
        None => {}
        Some(AudioStreamStatus::Recovering) => {
            *managed = None;
            set_status(route_registry, sink, AudioRouteStatus::Waiting);
        }
        Some(AudioStreamStatus::Terminal) => {
            *managed = None;
            set_status(route_registry, sink, AudioRouteStatus::Faulted);
            return true;
        }
    }
    mirror_producer.reactivate();
    match open(mirror_producer.new_consumer()) {
        Ok(stream) => {
            set_status(route_registry, sink, AudioRouteStatus::Active);
            stream.health.clear_recoverable_fault();
            *managed = Some(stream);
            eprintln!("{sink:?} audio stream ready");
            false
        }
        Err(error) => {
            set_status(route_registry, sink, error.status());
            eprintln!("{sink:?} audio unavailable: {error}");
            !should_retry_open(&error)
        }
    }
}

fn open(
    construction: AudioConstructionConfig,
    sink: AudioSink,
    consumer: PcmMirrorConsumer,
) -> Result<ManagedStream, RouteOpenError> {
    let health = AudioStreamHealth::optional(format!("{sink:?}"));
    let built = build_cpal_mirror_stream(
        match construction {
            AudioConstructionConfig::Raspberry(profile) => profile.output_buffer_frames,
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
        _stream: Box::new(built),
        health,
    })
}

#[cfg(test)]
mod tests;
