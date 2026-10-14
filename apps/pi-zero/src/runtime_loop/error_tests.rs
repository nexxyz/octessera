use super::*;
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use crate::host_adapter::PiHostAdapter;
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, RuntimeConfig, RuntimeErrorCode,
    RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation, RuntimeStoreResult, UsbDataRole,
};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn playing_save_error_and_dismissal_snapshots_reach_the_physical_worker() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-playing-runtime-error");
    let audio = crate::audio::test_service_with_prep_worker();
    let mut adapter = PiHostAdapter::new_with_data_role(
        Some(audio.clone()),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
        UsbDataRole::Gadget,
    );
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut adapter,
        )
        .unwrap();
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    let initial_revision =
        crate::runtime_init::publish_initial_scene(&playback, &mut runner, &mut adapter, &worker)
            .unwrap();
    worker.mark_first_menu_rendered().unwrap();
    let before_error_frame = audio.latest_physical_oled_frame().unwrap();
    let scheduler = HardwareRuntimeScheduler::new(
        Instant::now()
            .checked_sub(Duration::from_millis(500))
            .expect("test clock should accommodate the initial Play interval"),
        initial_revision,
    );
    let mut state = crate::runtime_loop::LoopState::new(scheduler, None);
    let (input_tx, input_rx) = mpsc::channel();
    for message in [
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_s","pressed":true}),
            request_snapshot: None,
        },
        HostMessage::RuntimeResult {
            result: RuntimeStoreResult::RuntimeFailure {
                error: RuntimeErrorFacts::new(
                    RuntimeErrorDomain::Storage,
                    RuntimeErrorCode::OperationFailed,
                    RuntimeOperation::StoreSaveDefault,
                    Some("save failed".into()),
                ),
            },
        },
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_s","pressed":false}),
            request_snapshot: None,
        },
    ] {
        input_tx.send(message).unwrap();
    }
    crate::runtime_loop::drain_host_messages(&input_rx, &mut playback, &mut runner, &mut adapter);
    advance_until_presented(
        &mut state,
        &mut playback,
        &mut runner,
        &mut adapter,
        &worker,
    );
    assert!(playback.last_status().is_some_and(|status| {
        status.transport == playback_runtime::RuntimeTransportState::Playing
    }));
    assert!(!playback.last_snapshot().unwrap()["runtimeError"].is_null());
    assert!(playback.native_presentation_state().1.is_some());
    let error_frame = wait_for_latest_frame(&audio, |revision, pixels| {
        revision > before_error_frame.0 && pixels != before_error_frame.1.as_ref()
    });

    crate::runtime_dispatch::dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::DeviceInput {
            input: serde_json::json!({"type":"button_a","pressed":true}),
            request_snapshot: None,
        },
    )
    .unwrap();
    assert!(playback.last_snapshot().unwrap()["runtimeError"].is_null());
    assert!(playback.native_presentation_state().1.is_none());
    advance_until_presented(
        &mut state,
        &mut playback,
        &mut runner,
        &mut adapter,
        &worker,
    );
    wait_for_latest_frame(&audio, |revision, pixels| {
        revision > error_frame.0 && pixels != error_frame.1.as_slice()
    });
    worker.publish_shutdown().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn advance_until_presented(
    state: &mut crate::runtime_loop::LoopState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    worker: &RenderWorker,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while state.scheduler.snapshot_unpresented(playback) {
        assert!(
            Instant::now() < deadline,
            "snapshot revision was never presented"
        );
        assert!(!crate::runtime_loop::maybe_advance_runtime(
            state,
            playback,
            runner,
            adapter,
            worker,
            &mut NoPowerBoard,
        ));
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait_for_latest_frame(
    audio: &crate::audio::AudioService,
    matches: impl Fn(u64, &[u8]) -> bool,
) -> (u64, Vec<u8>) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some((revision, pixels)) = audio.latest_physical_oled_frame() {
            if matches(revision, &pixels) {
                return (revision, pixels.to_vec());
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    panic!("render worker did not accept the expected physical OLED frame");
}

struct NoPowerBoard;

impl crate::runtime_loop::BoardLoop for NoPowerBoard {
    fn service_audio(
        &mut self,
        _playback: &mut PlaybackRuntime,
        _runner: &mut NativeRunner,
        _adapter: &mut PiHostAdapter,
    ) -> Result<(), String> {
        Ok(())
    }

    fn handle_power_request(
        &mut self,
        _playback: &PlaybackRuntime,
        _runner: &mut NativeRunner,
        _adapter: &mut PiHostAdapter,
        _render_worker: &RenderWorker,
    ) -> bool {
        false
    }
}
