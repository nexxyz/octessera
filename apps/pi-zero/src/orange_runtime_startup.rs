use super::*;
use crate::sample_browser::builtin_favourite_dirs;
use playback_runtime::AudioOptimization;

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

pub(super) fn ensure_timing_keep_awake(playback: &PlaybackRuntime) -> Result<(), String> {
    if std::env::var("OCTESSERA_PI_TIMING_KEEP_AWAKE").as_deref() != Ok("1") {
        return Ok(());
    }
    let snapshot = playback
        .last_snapshot()
        .ok_or("Orange AWAKE candidate has no native snapshot")?;
    if !is_awake_menu_snapshot(snapshot) {
        return Err("Orange AWAKE candidate did not load awake settings".into());
    }
    Ok(())
}

fn is_awake_menu_snapshot(snapshot: &serde_json::Value) -> bool {
    is_normal_menu_snapshot(snapshot)
        && snapshot["settings"]["dimTimerSeconds"] == 0
        && snapshot["settings"]["screenSleepSeconds"] == 0
        && snapshot["settings"]["ledsDimmed"] == false
}

pub(crate) fn prepare_runtime(
    audio: AudioService,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    usb_midi_out_enabled: bool,
    audio_optimization: AudioOptimization,
    skip_startup_splash: bool,
    keyboard_control: Option<crate::usb_keyboard::KeyboardCaptureControl>,
) -> Result<PreparedRuntime, String> {
    let mut playback = PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: usb_midi_out_enabled,
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        sample_builtin_favourite_dirs: builtin_favourite_dirs(),
        audio_optimization,
        audio_optimization_capacity_available: true,
        jack_audio_required: true,
        boot_applied_usb_midi_out_enabled: usb_midi_out_enabled,
        ..NativeRunnerConfig::default()
    })?;
    if skip_startup_splash {
        runner.skip_startup_splash();
    }
    let mut host = if cfg!(test) {
        static NEXT_TEST_STORE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "octessera-orange-runtime-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_TEST_STORE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        OrangeHostAdapter::with_directories(
            audio,
            root.join("presets"),
            root.join("samples"),
            midi_handler,
            usb_midi_out_enabled,
        )?
    } else {
        OrangeHostAdapter::new(audio, midi_handler, usb_midi_out_enabled)?
    };
    if let Some(control) = keyboard_control {
        host.set_keyboard_capture_control(control);
    }
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
    let responses = runner.poll_deferred_menu_apply_music_first()?;
    if !responses.is_empty() {
        let output = playback.dispatch_runner_messages(responses, runner, host)?;
        process_runtime_output(playback, runner, host, output)?;
    }
    for result in host.drain_startup_platform_results(HOST_RESULT_BUDGET) {
        dispatch(playback, runner, host, result)?;
    }
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
    let audio = prepared.host.audio_service();
    let (frame_revision, pixels) = render.take_acknowledged_startup_oled_frame()?;
    audio.submit_accepted_oled_frame_shared(frame_revision, pixels)?;
    render.set_recording_audio(audio);
    Ok(revision)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;

    #[test]
    fn awake_candidate_requires_both_zero_timers_and_lit_normal_menu() {
        let awake = json!({
            "settings": { "dimTimerSeconds": 0, "screenSleepSeconds": 0, "ledsDimmed": false },
            "display": { "off": false, "splash": "", "title": "Build", "lines": ["ready"] }
        });
        assert!(is_awake_menu_snapshot(&awake));
        for (path, value) in [
            ("/settings/dimTimerSeconds", json!(60)),
            ("/settings/screenSleepSeconds", json!(60)),
            ("/settings/ledsDimmed", json!(true)),
            ("/display/off", json!(true)),
        ] {
            let mut rejected = awake.clone();
            *rejected.pointer_mut(path).unwrap() = value;
            assert!(!is_awake_menu_snapshot(&rejected), "{path}");
        }
    }

    #[test]
    fn orange_startup_uses_canonical_builtin_sample_favourites() {
        let (audio, _, _) = crate::audio::test_service();
        let mut prepared = prepare_runtime(
            audio,
            Arc::new(|_| {}),
            false,
            AudioOptimization::Latency,
            true,
            None,
        )
        .unwrap();

        crate::sample_browser::assert_builtin_favourite_menu(&mut prepared.runner);
    }
}
