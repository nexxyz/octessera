use super::*;
use playback_runtime::AudioOptimization;

pub(crate) struct PreparedRuntime {
    pub(super) playback: PlaybackRuntime,
    pub(super) runner: NativeRunner,
    pub(super) host: PiHostAdapter,
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
    mut host: PiHostAdapter,
    audio_optimization: AudioOptimization,
    skip_startup_splash: bool,
    keyboard_control: Option<crate::keyboard_capture::KeyboardCaptureControl>,
) -> Result<PreparedRuntime, String> {
    let (mut playback, mut runner) = crate::runtime_init::new_runtime(audio_optimization, false)?;
    if skip_startup_splash {
        runner.skip_startup_splash();
    }
    if let Some(control) = keyboard_control {
        host.core.set_keyboard_capture_control(control);
    }
    crate::runtime_init::start_host(&mut playback, &mut runner, &mut host)?;
    Ok(PreparedRuntime {
        playback,
        runner,
        host,
    })
}

pub(crate) fn publish_prepared_initial_scene(
    prepared: &mut PreparedRuntime,
    render: &RenderWorker,
) -> Result<u64, String> {
    crate::runtime_init::publish_initial_scene(
        &prepared.playback,
        &mut prepared.runner,
        &mut prepared.host,
        render,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timing_input::is_awake_menu_snapshot;
    use serde_json::json;

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
    fn orange_startup_builds_the_saved_patch_menu_once() {
        let (audio, _, _) = crate::audio::test_service();
        let prepared = prepare_runtime(
            crate::orange_runtime::tests::default_store_host(audio),
            AudioOptimization::Latency,
            true,
            None,
        )
        .unwrap();

        assert_eq!(prepared.runner.test_menu_build_count(), 1);
        let snapshot = prepared.playback.last_snapshot().unwrap();
        assert!(crate::normal_menu::is_normal_menu_display(snapshot));
        assert_eq!(snapshot["display"]["lines"][0], "  Build >");
    }

    #[test]
    fn orange_startup_uses_canonical_builtin_sample_favourites() {
        let (audio, _, _) = crate::audio::test_service();
        let mut prepared = prepare_runtime(
            crate::orange_runtime::tests::default_store_host(audio),
            AudioOptimization::Latency,
            true,
            None,
        )
        .unwrap();

        crate::sample_browser::assert_builtin_favourite_menu(&mut prepared.runner);
    }
}
