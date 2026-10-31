use super::{adapter, worker};
use crate::hardware_runtime_scheduler::HardwareRuntimeScheduler;
use crate::host_adapter::PiHostAdapter;
use crate::render_loop::RenderWorker;
use crate::runtime_loop::{maybe_advance_runtime, BoardLoop, LoopState};
use playback_runtime::{HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime};
use std::time::{Duration, Instant};

struct IdleBoard;

impl BoardLoop for IdleBoard {
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

#[test]
fn stopped_animation_steps_reach_the_oled_as_scene_captures_without_runtime_snapshots() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-stopped-animation");
    let mut adapter = adapter(&root, None);
    let payload: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    let mut playback = PlaybackRuntime::new(playback_runtime::RuntimeConfig::default());
    playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut adapter,
        )
        .unwrap();
    let worker = worker();
    let presented = playback.last_snapshot_revision();
    let mut state = LoopState::new(
        HardwareRuntimeScheduler::new(Instant::now(), presented),
        None,
    );

    let settle_until = Instant::now() + Duration::from_millis(200);
    while Instant::now() < settle_until {
        advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &worker,
        );
    }

    let input_at = Instant::now();
    crate::runtime_dispatch::dispatch(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::DeviceInput {
            input: serde_json::json!({"type": "encoder_turn", "id": "main", "delta": 1}),
            request_snapshot: None,
        },
    )
    .unwrap();
    assert!(
        playback.last_snapshot().unwrap()["oledAnimation"]["body"]["kind"] == "select",
        "the select move animates"
    );
    let snapshot_revision = playback.last_snapshot_revision();
    let raster_revision = playback.oled_frame_revision();
    let captures = state.native_scenes.capture_count;

    let mut first_capture_at = None;
    while input_at.elapsed() < Duration::from_millis(400) {
        advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &worker,
        );
        if state.native_scenes.capture_count > captures {
            first_capture_at.get_or_insert_with(Instant::now);
        }
    }

    assert_eq!(playback.last_snapshot_revision(), snapshot_revision);
    assert_eq!(playback.oled_frame_revision(), raster_revision);
    let first_capture_at = first_capture_at.expect("the move reaches the OLED");
    if first_capture_at.duration_since(input_at) < Duration::from_millis(30) {
        assert!(
            state.native_scenes.capture_count >= captures + 2,
            "the animation steps after the first capture are captured too"
        );
    }
    worker.abort().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

fn advance(
    state: &mut LoopState,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    worker: &RenderWorker,
) {
    assert!(!maybe_advance_runtime(
        state,
        playback,
        runner,
        adapter,
        worker,
        &mut IdleBoard,
    ));
    std::thread::sleep(Duration::from_millis(1));
}
