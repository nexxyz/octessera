//! Startup shared by both boards: the runtime pair, the first host load, and
//! the first acknowledged OLED snapshot. Board startups differ only in when
//! they wait for audio prep and how they report readiness.

use crate::host_adapter::PiHostAdapter;
use crate::normal_menu::is_normal_menu_snapshot;
use crate::render_loop::RenderWorker;
use crate::runtime_output::{initialize_host_state, process_runtime_output};
use crate::sample_browser::builtin_favourite_dirs;
use playback_runtime::{
    AudioOptimization, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime,
    RuntimeConfig, SyncSource,
};

const STARTUP_RESULT_BUDGET: usize = 4;

pub(crate) fn new_runtime(
    audio_optimization: AudioOptimization,
    boot_applied_usb_midi_out_enabled: bool,
    usb_data_role_available: bool,
) -> Result<(PlaybackRuntime, NativeRunner), String> {
    let playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: false,
    });
    let runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        sample_builtin_favourite_dirs: builtin_favourite_dirs(),
        audio_optimization,
        audio_optimization_capacity_available: true,
        jack_audio_required: true,
        usb_data_role_available,
        boot_applied_usb_midi_out_enabled,
        ..NativeRunnerConfig::default()
    })?;
    Ok((playback, runner))
}

/// Loads the stores into the runner and requests the first snapshot.
pub(crate) fn start_host(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
) -> Result<(), String> {
    initialize_host_state(playback, runner, adapter)?;
    let responses = runner.poll_deferred_menu_apply_music_first()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, adapter)?;
        process_runtime_output(playback, runner, adapter, output)?;
    }
    for result in adapter
        .core
        .platform_service
        .drain_results(STARTUP_RESULT_BUDGET)
    {
        crate::runtime_dispatch::dispatch_runtime_message(playback, runner, adapter, result)?;
    }
    let message = HostMessage::TransportPulseStep {
        pulses: 0,
        source: playback.config().sync_source.clone(),
        at_ppqn_pulse: playback
            .last_status()
            .map(|status| status.current_ppqn_pulse),
        request_snapshot: Some(true),
    };
    crate::runtime_dispatch::dispatch_runtime_message(playback, runner, adapter, message)
}

/// Renders the first normal-menu snapshot and hands its frame to recording.
pub(crate) fn publish_initial_snapshot(
    playback: &PlaybackRuntime,
    runner: &NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
) -> Result<u64, String> {
    let snapshot = playback
        .last_snapshot()
        .cloned()
        .ok_or_else(|| "initial snapshot is missing".to_string())?;
    if !is_normal_menu_snapshot(&snapshot) {
        return Err("initial snapshot is not a normal menu".into());
    }
    if !runner.is_canonical_menu_presentation() {
        return Err("native runner is not presenting the canonical menu".into());
    }
    let revision = playback.last_snapshot_revision();
    if revision == 0 {
        return Err("initial snapshot revision is missing".into());
    }
    let oled = adapter
        .core
        .oled_publication_for_snapshot(&snapshot, true)?;
    render_worker.publish_acknowledged_snapshot(snapshot, oled)?;
    let (frame_revision, pixels) = render_worker.take_acknowledged_startup_oled_frame()?;
    if let Some(audio) = adapter.audio_service() {
        audio.submit_accepted_oled_frame_shared(frame_revision, pixels)?;
        render_worker.set_recording_audio(audio);
    }
    Ok(revision)
}
