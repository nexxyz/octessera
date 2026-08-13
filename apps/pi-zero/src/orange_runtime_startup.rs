use super::*;

pub(crate) struct PreparedRuntime {
    pub(super) playback: PlaybackRuntime,
    pub(super) runner: NativeRunner,
    pub(super) host: OrangeHostAdapter,
}

pub(crate) struct OrangeStartupReadinessGate {
    acknowledged_initial_write: bool,
    ready: bool,
}

impl OrangeStartupReadinessGate {
    pub(crate) fn new(initial_rendered: bool) -> Self {
        Self {
            acknowledged_initial_write: initial_rendered,
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

    pub(crate) fn try_mark_ready(
        &mut self,
        dac_status: OrangeDacStatus,
        candidate_readiness: &mut CandidateReadiness,
    ) -> Result<(), String> {
        if self.ready || !self.acknowledged_initial_write || dac_status != OrangeDacStatus::Healthy
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
    drain_host_work(&mut playback, &mut runner, &mut host)?;
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
