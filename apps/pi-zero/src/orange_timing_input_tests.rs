#![cfg(feature = "hardware-orange-pi-zero-2w")]

use super::*;
use crate::input::encoder_press_message;
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeAudioCommand, RuntimeConfig,
};
use std::ffi::OsString;
use std::sync::{Mutex, MutexGuard};

static ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

struct EnvironmentRestore {
    previous: Vec<(&'static str, Option<OsString>)>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvironmentRestore {
    fn set(values: &[(&'static str, Option<&str>)]) -> Self {
        let lock = ENVIRONMENT_LOCK.lock().unwrap();
        let previous = values
            .iter()
            .map(|(key, value)| {
                let old = std::env::var_os(key);
                if let Some(value) = value {
                    std::env::set_var(key, value);
                } else {
                    std::env::remove_var(key);
                }
                (*key, old)
            })
            .collect();
        Self {
            previous,
            _lock: lock,
        }
    }
}

impl Drop for EnvironmentRestore {
    fn drop(&mut self) {
        for (key, value) in &self.previous {
            if let Some(value) = value {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }
}

struct RuntimeFixture {
    playback: PlaybackRuntime,
    runner: NativeRunner,
    host: OrangeHostAdapter,
    _control_rx: std::sync::mpsc::Receiver<crate::audio::AudioControlRequest>,
    _event_rx: rodio_engine_source::EngineEventReceiver,
    root: std::path::PathBuf,
}

fn runtime_fixture(aux_auto_map: bool) -> RuntimeFixture {
    let root = std::env::temp_dir().join(format!(
        "octessera-orange-autoaux-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let (audio, control_rx, event_rx, _prep_result_tx) =
        crate::audio::test_service_with_recording_dir(root.join("recording"));
    let mut host = OrangeHostAdapter::with_directories(
        audio.clone(),
        root.join("store"),
        root.join("samples"),
        std::sync::Arc::new(|_| {}),
        false,
    )
    .unwrap();
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../config/generated/pi/default.json")).unwrap();
    let mut payload = payload;
    payload["runtimeConfig"]["auxAutoMapEnabled"] = serde_json::json!(aux_auto_map);
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let output = playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut host,
        )
        .unwrap();
    crate::orange_candidate::process_runtime_output(&mut playback, &mut runner, &mut host, output)
        .unwrap();
    RuntimeFixture {
        playback,
        runner,
        host,
        _control_rx: control_rx,
        _event_rx: event_rx,
        root,
    }
}

fn valid_environment(store: Option<&str>) -> EnvironmentRestore {
    EnvironmentRestore::set(&[
        (AUTOAUX_ENV, Some("1")),
        ("OCTESSERA_TIMING_AUTOPLAY", Some("1")),
        ("OCTESSERA_PI_TIMING_KEEP_AWAKE", Some("1")),
        ("OCTESSERA_PI_UI_PROFILE", Some("1")),
        ("OCTESSERA_PI_STORE_DIR", store),
    ])
}

fn prepare_timing(fixture: &mut RuntimeFixture) -> Result<Option<OrangeTimingInput>, String> {
    let auto_aux = OrangeTimingInput::validate_opt_in()?;
    OrangeTimingInput::prepare(
        auto_aux,
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
    )
}

fn timing_tick(
    fixture: &mut RuntimeFixture,
    timing: &mut OrangeTimingInput,
    profiler: &crate::ui_profile::UiProfiler,
    at: Instant,
) -> bool {
    let revision = fixture.playback.last_snapshot_revision();
    let previous_turns = timing.aux_turns;
    let finished = timing
        .tick(
            at,
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            profiler,
        )
        .unwrap();
    assert!(timing.aux_turns.saturating_sub(previous_turns) <= 1);
    assert_eq!(fixture.playback.last_snapshot_revision(), revision);
    finished
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn autoaux_setup_uses_stopped_native_menu_and_routes_cutoff_turns_through_host() {
    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-0123456789abcdef0123456789abcdef.service",
    ));
    let mut fixture = runtime_fixture(true);
    let timing = prepare_timing(&mut fixture).unwrap().unwrap();
    assert_eq!(
        timing.plateau_values,
        [timing.starting_cutoff + 1, timing.starting_cutoff + 2]
    );
    assert!(fixture
        .playback
        .last_status()
        .is_some_and(|status| { status.transport == RuntimeTransportState::Playing }));
    let scene = fixture.runner.capture_display_scene().unwrap();
    let (metrics, error) = fixture.playback.native_presentation_state();
    assert_eq!(
        crate::orange_candidate::native_scene::selected_cutoff_display_value(
            &scene, metrics, error,
        ),
        Some(timing.starting_cutoff)
    );
    for delta in [1, -1] {
        crate::orange_candidate::dispatch(
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            encoder_turn_message("encoder_aux_1", delta),
        )
        .unwrap();
    }
    let audio = fixture
        .host
        .take_autoaux_command_evidence()
        .expect("AutoAux host command profiling should be armed after Play");
    assert!(audio.successful_count >= 2);
    assert!(audio.distinct_values[0].is_some());
    assert!(audio.distinct_values[1].is_some());
    assert_ne!(audio.distinct_values[0], audio.distinct_values[1]);
    let _ = std::fs::remove_dir_all(fixture.root);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn autoaux_command_profile_ignores_runner_commands_rejected_by_host() {
    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-ffeeddccbbaa99887766554433221100.service",
    ));
    let mut fixture = runtime_fixture(true);
    let timing = prepare_timing(&mut fixture).unwrap().unwrap();
    drop(fixture._event_rx);
    let messages = fixture
        .runner
        .send_music_first(HostMessage::DeviceInput {
            input: serde_json::json!({"type":"encoder_turn","id":"aux1","delta":1}),
            request_snapshot: Some(false),
        })
        .unwrap();
    assert!(messages.iter().any(|message| matches!(message,
    RunnerMessage::AudioCommands { commands }
        if commands.iter().any(|command| matches!(command,
            RuntimeAudioCommand::SetSynthParam {
                instrument_slot: 0,
                path,
                ..
            } if path == "synth.filter.cutoffHz"
        )))));
    fixture
        .playback
        .dispatch_runner_messages(messages, &mut fixture.runner, &mut fixture.host)
        .unwrap();
    let evidence = fixture
        .host
        .take_autoaux_command_evidence()
        .expect("AutoAux command profile should remain active");
    assert_eq!(evidence.successful_count, 0);
    assert_eq!(evidence.distinct_values, [None, None]);
    assert_eq!(timing.rapid_turns, 0);
    let _ = std::fs::remove_dir_all(fixture.root);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn autoaux_timing_ticks_are_bounded_and_the_due_save_payload_is_measured() {
    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-fedcba9876543210fedcba9876543210.service",
    ));
    let mut fixture = runtime_fixture(true);
    let mut timing = prepare_timing(&mut fixture).unwrap().unwrap();
    assert!(PLATEAU > crate::hardware_runtime_scheduler::SNAPSHOT_TICK * 2);
    let mut profiler = crate::ui_profile::UiProfiler::from_controls(Some("1"), false);
    let start = Instant::now();
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE
    ));
    let plateau_one = timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + Duration::from_millis(100),
    );
    assert!(!plateau_one);
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU
    ));
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU + PLATEAU
    ));
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU + PLATEAU + TURN_INTERVAL
    ));
    assert_eq!(timing.rapid_turns, 1);
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU + PLATEAU + Duration::from_millis(200)
    ));
    assert_eq!(timing.rapid_turns, 2);
    let rapid_start = start + BASELINE + PLATEAU + PLATEAU;
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        rapid_start + RAPID
    ));
    assert!(timing.missed_turns > 0);

    std::thread::sleep(Duration::from_millis(160));
    let before_post_burst_save = profiler.save_payload_count();
    super::super::host_work::drain_host_work(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
        &mut profiler,
    )
    .unwrap();
    assert_eq!(profiler.save_payload_count(), before_post_burst_save + 1);
    assert!(timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        rapid_start + RAPID + TAIL
    ));
    let _ = std::fs::remove_dir_all(fixture.root);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn post_burst_save_requirement_rejects_a_preflight_only_payload() {
    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-abcdef0123456789abcdef0123456789.service",
    ));
    let mut fixture = runtime_fixture(true);
    let mut timing = prepare_timing(&mut fixture).unwrap().unwrap();
    let mut profiler = crate::ui_profile::UiProfiler::from_controls(Some("1"), false);
    std::thread::sleep(Duration::from_millis(160));
    super::super::host_work::drain_host_work(
        &mut fixture.playback,
        &mut fixture.runner,
        &mut fixture.host,
        &mut profiler,
    )
    .unwrap();
    assert_eq!(profiler.save_payload_count(), 1);
    let start = Instant::now();
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE
    ));
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU
    ));
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU + PLATEAU
    ));
    assert!(!timing_tick(
        &mut fixture,
        &mut timing,
        &profiler,
        start + BASELINE + PLATEAU + PLATEAU + RAPID
    ));
    assert!(timing
        .tick(
            start + BASELINE + PLATEAU + PLATEAU + RAPID + TAIL,
            &mut fixture.playback,
            &mut fixture.runner,
            &mut fixture.host,
            &profiler,
        )
        .is_err());
    let _ = std::fs::remove_dir_all(fixture.root);
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[test]
fn autoaux_refuses_missing_gates_wrong_focus_and_unbound_aux() {
    let missing = EnvironmentRestore::set(&[
        (AUTOAUX_ENV, Some("1")),
        ("OCTESSERA_TIMING_AUTOPLAY", None),
        ("OCTESSERA_PI_TIMING_KEEP_AWAKE", None),
        ("OCTESSERA_PI_UI_PROFILE", None),
        ("OCTESSERA_PI_STORE_DIR", None),
    ]);
    let mut fixture = runtime_fixture(true);
    let initial_revision = fixture.playback.last_snapshot_revision();
    assert!(prepare_timing(&mut fixture).is_err());
    assert_eq!(fixture.playback.last_snapshot_revision(), initial_revision);
    assert!(fixture
        .playback
        .last_status()
        .is_some_and(|status| { status.transport == RuntimeTransportState::Stopped }));
    drop(missing);
    let _ = std::fs::remove_dir_all(fixture.root);

    let bad_store = valid_environment(Some("/var/lib/octessera/presets"));
    let mut fixture = runtime_fixture(true);
    let initial_revision = fixture.playback.last_snapshot_revision();
    assert!(prepare_timing(&mut fixture).is_err());
    assert_eq!(fixture.playback.last_snapshot_revision(), initial_revision);
    drop(bad_store);
    let _ = std::fs::remove_dir_all(fixture.root);

    let _environment = valid_environment(Some(
        "/var/lib/octessera/study-stores/octessera-study-0123456789abcdef0123456789abcdef.service",
    ));
    let mut wrong_focus = runtime_fixture(true);
    crate::orange_candidate::dispatch(
        &mut wrong_focus.playback,
        &mut wrong_focus.runner,
        &mut wrong_focus.host,
        encoder_turn_message("encoder_main", 2),
    )
    .unwrap();
    crate::orange_candidate::dispatch(
        &mut wrong_focus.playback,
        &mut wrong_focus.runner,
        &mut wrong_focus.host,
        encoder_press_message("encoder_main"),
    )
    .unwrap();
    let wrong_revision = wrong_focus.playback.last_snapshot_revision();
    assert!(prepare_timing(&mut wrong_focus).is_err());
    assert_eq!(
        wrong_focus.playback.last_snapshot_revision(),
        wrong_revision
    );
    assert!(wrong_focus
        .playback
        .last_status()
        .is_some_and(|status| { status.transport == RuntimeTransportState::Stopped }));
    let _ = std::fs::remove_dir_all(wrong_focus.root);

    let mut unbound = runtime_fixture(false);
    let initial_revision = unbound.playback.last_snapshot_revision();
    assert!(prepare_timing(&mut unbound).is_err());
    assert!(unbound
        .playback
        .last_status()
        .is_some_and(|status| { status.transport == RuntimeTransportState::Stopped }));
    assert!(unbound.playback.last_snapshot_revision() > initial_revision);
    let _ = std::fs::remove_dir_all(unbound.root);
}
