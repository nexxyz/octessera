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

#[test]
fn a_long_layer_toast_scrolls_on_the_oled_while_stopped() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-stopped-toast-scroll");
    let mut adapter = adapter(&root, None);
    let mut payload: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    payload["runtimeConfig"]["layers"][1]["autoName"] = false.into();
    payload["runtimeConfig"]["layers"][1]["name"] = "slowly wandering sequencer".into();
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
    let mut state = LoopState::new(
        HardwareRuntimeScheduler::new(Instant::now(), playback.last_snapshot_revision()),
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
    for input in [
        serde_json::json!({"type": "button_fn", "pressed": true}),
        serde_json::json!({"type": "grid_press", "x": 0, "y": 1}),
        serde_json::json!({"type": "grid_release", "x": 0, "y": 1}),
        serde_json::json!({"type": "button_fn", "pressed": false}),
    ] {
        crate::runtime_dispatch::dispatch(
            &mut playback,
            &mut runner,
            &mut adapter,
            HostMessage::DeviceInput {
                input,
                request_snapshot: None,
            },
        )
        .unwrap();
    }
    let mut scroll_captures = 0;
    let mut captures = state.native_scenes.capture_count;
    while input_at.elapsed() < Duration::from_millis(1_900) {
        advance(
            &mut state,
            &mut playback,
            &mut runner,
            &mut adapter,
            &worker,
        );
        eprintln!(
            "DBG due={:?} {:?} rev={} toast={:?} dl={:?}",
            state
                .scheduler
                .display_snapshot_due(Instant::now(), &runner, &playback),
            input_at.elapsed(),
            playback.last_snapshot_revision(),
            playback
                .last_snapshot()
                .map(|s| s["display"]["toast"].clone()),
            runner
                .next_continuous_display_snapshot_deadline(
                    Instant::now(),
                    Duration::from_millis(33)
                )
                .map(|d| d.saturating_duration_since(Instant::now()))
        );
        if state.native_scenes.capture_count > captures {
            captures = state.native_scenes.capture_count;
            if input_at.elapsed() > Duration::from_millis(850) {
                scroll_captures += 1;
            }
        }
    }

    assert!(
        scroll_captures >= 4,
        "the scroll steps after the start pause reach the OLED: {scroll_captures}"
    );
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
