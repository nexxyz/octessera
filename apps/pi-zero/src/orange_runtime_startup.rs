use super::*;
use crate::initial_audio_prep::{interpret_initial_audio_prep, InitialAudioPrepBoard};

pub(crate) struct PreparedRuntime {
    pub(super) playback: PlaybackRuntime,
    pub(super) runner: NativeRunner,
    pub(super) host: OrangeHostAdapter,
}

pub(crate) struct OrangeStartupReadinessGate {
    acknowledged_initial_write: bool,
    acknowledged_initial_audio_prep: bool,
    ready: bool,
}

impl OrangeStartupReadinessGate {
    pub(crate) fn new(initial_rendered: bool) -> Self {
        Self {
            acknowledged_initial_write: initial_rendered,
            acknowledged_initial_audio_prep: false,
            ready: false,
        }
    }

    pub(crate) fn acknowledge_initial_write(
        &mut self,
        result: Result<(), String>,
    ) -> Result<(), String> {
        result?;
        self.acknowledged_initial_write = true;
        Ok(())
    }

    pub(crate) fn acknowledge_initial_audio_prep(
        &mut self,
        result: Result<(), String>,
    ) -> Result<(), String> {
        result?;
        self.acknowledged_initial_audio_prep = true;
        Ok(())
    }

    pub(crate) fn try_mark_ready(
        &mut self,
        route_status: OrangeDacStatus,
        candidate_readiness: &mut CandidateReadiness,
    ) -> Result<(), String> {
        if self.ready
            || !self.acknowledged_initial_write
            || !self.acknowledged_initial_audio_prep
            || route_status != OrangeDacStatus::Healthy
        {
            return Ok(());
        }
        candidate_readiness.mark_ready()?;
        self.ready = true;
        Ok(())
    }
}

pub(crate) fn prepare_runtime(
    audio: AudioService,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    usb_midi_out_enabled: bool,
    skip_startup_splash: bool,
) -> Result<PreparedRuntime, String> {
    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: usb_midi_out_enabled,
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        ..NativeRunnerConfig::default()
    })?;
    if skip_startup_splash {
        runner.skip_startup_splash();
    }
    let mut host = OrangeHostAdapter::new(audio, midi_handler, usb_midi_out_enabled)?;
    initialize_host_state(&mut playback, &mut runner, &mut host)?;
    drain_startup_host_work(&mut playback, &mut runner, &mut host)?;
    dispatch(
        &mut playback,
        &mut runner,
        &mut host,
        HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        },
    )?;
    Ok(PreparedRuntime {
        playback,
        runner,
        host,
    })
}

fn drain_startup_host_work(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    let responses = runner.flush_deferred_menu_apply()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        process_runtime_output(playback, runner, host, output)?;
    }
    for follow_up in host.flush_due_default_save()? {
        dispatch(playback, runner, host, follow_up)?;
    }
    for result in host.drain_startup_platform_results(HOST_RESULT_BUDGET) {
        dispatch(playback, runner, host, result)?;
    }
    Ok(())
}

fn initialize_host_state(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    let output = playback.dispatch_runner_messages(
        vec![playback_runtime::RunnerMessage::PlatformEffects {
            effects: vec![
                playback_runtime::RuntimePlatformEffect::StoreLoadDefault,
                playback_runtime::RuntimePlatformEffect::MidiListOutputsRequest,
                playback_runtime::RuntimePlatformEffect::MidiListInputsRequest,
            ],
        }],
        runner,
        host,
    )?;
    process_runtime_output(playback, runner, host, output)?;
    Ok(())
}

pub(crate) fn publish_prepared_acknowledged_snapshot(
    prepared: &mut PreparedRuntime,
    render: &RenderWorker,
) -> Result<u64, String> {
    let snapshot = prepared
        .playback
        .last_snapshot()
        .cloned()
        .ok_or_else(|| "Orange initial snapshot is missing".to_string())?;
    if !is_normal_menu_snapshot(&snapshot) {
        return Err("Orange initial snapshot is not a normal menu".into());
    }
    if !prepared.runner.is_canonical_menu_presentation() {
        return Err("Orange native runner is not presenting the canonical menu".into());
    }
    let revision = prepared.playback.last_snapshot_revision();
    if revision == 0 {
        return Err("Orange initial snapshot revision is missing".into());
    }
    let oled = prepared
        .host
        .oled_publication_for_snapshot(&snapshot, true)?;
    render.publish_acknowledged_snapshot(snapshot, oled)?;
    Ok(revision)
}

pub(crate) fn wait_for_initial_audio_prep(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    const TIMEOUT: Duration = Duration::from_secs(10);
    const POLL: Duration = Duration::from_millis(10);
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let audio = host.audio_service();
        if let Some(message) = audio.drain_prep_results(1).into_iter().next() {
            let outcome = interpret_initial_audio_prep(
                &message,
                audio
                    .config_revision
                    .load(std::sync::atomic::Ordering::SeqCst),
                InitialAudioPrepBoard::Orange,
            );
            super::dispatch(playback, runner, host, message)?;
            if let Some(outcome) = outcome {
                return outcome;
            }
        }
        for message in host.drain_results(HOST_RESULT_BUDGET) {
            let outcome = interpret_initial_audio_prep(
                &message,
                host.audio_service()
                    .config_revision
                    .load(std::sync::atomic::Ordering::SeqCst),
                InitialAudioPrepBoard::Orange,
            );
            super::dispatch(playback, runner, host, message)?;
            if let Some(outcome) = outcome {
                return outcome;
            }
        }
        if Instant::now() >= deadline {
            return Err("initial Orange audio preparation timed out".into());
        }
        thread::sleep(POLL);
    }
}
