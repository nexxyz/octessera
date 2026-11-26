use super::*;
use crate::render::HardwareRenderTargets;
use crate::runtime_dispatch::{dispatch_runtime_message, process_runtime_output};
use playback_runtime::{NativeRunnerConfig, RuntimeConfig};
use serde_json::{json, Value};
use std::sync::Arc;

struct InterruptedBoard;

impl BoardLoop for InterruptedBoard {
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

    fn interrupted(&self) -> bool {
        true
    }
}

struct EditedStore {
    root: std::path::PathBuf,
    default_path: std::path::PathBuf,
    edited: Value,
    playback: PlaybackRuntime,
    runner: NativeRunner,
    adapter: PiHostAdapter,
}

/// A runner with one coalesced autosave pending and no `default.patch.json` on disk yet.
fn edited_store(name: &str) -> EditedStore {
    let root = crate::test_temp_dir::unique_temp_path(name);
    let mut full_default: Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    full_default["runtimeConfig"]["autoSaveDefault"] = json!(true);
    full_default["runtimeConfig"]["rollingBackups"] = json!(false);
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(full_default.clone()).unwrap();
    runner.skip_startup_splash();
    runner
        .test_focus_menu_item("aux:0:turn.instruments.0.synth.osc1.levelPct")
        .unwrap();
    runner
        .send_music_first(HostMessage::DeviceInput {
            input: json!({"type":"encoder_press","id":"main"}),
            request_snapshot: Some(false),
        })
        .unwrap();
    let store = root.join("store");
    crate::pi_store_test_support::write_pair(&store, &full_default);
    let default_path = store.join("default.patch.json");
    std::fs::remove_file(&default_path).unwrap();
    let mut playback = PlaybackRuntime::new(RuntimeConfig::default());
    let mut adapter = PiHostAdapter::new(
        None,
        store,
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        playback_runtime::AudioOutputSet::jack(),
    );
    let output = playback
        .dispatch_runner_messages(
            runner.messages_with_snapshot().unwrap(),
            &mut runner,
            &mut adapter,
        )
        .unwrap();
    process_runtime_output(&mut playback, &mut runner, &mut adapter, output).unwrap();
    dispatch_runtime_message(
        &mut playback,
        &mut runner,
        &mut adapter,
        HostMessage::DeviceInput {
            input: json!({"type":"encoder_turn","id":"aux1","delta":-1}),
            request_snapshot: Some(false),
        },
    )
    .unwrap();
    let edited = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    assert!(!default_path.exists());
    EditedStore {
        root,
        default_path,
        edited,
        playback,
        runner,
        adapter,
    }
}

#[test]
fn a_termination_signal_writes_the_coalesced_autosave_before_the_loop_ends() {
    let EditedStore {
        root,
        default_path,
        edited,
        mut playback,
        mut runner,
        mut adapter,
    } = edited_store("octessera-exit-save");

    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    let mut state = LoopState::new(
        HardwareRuntimeScheduler::new(Instant::now(), playback.last_snapshot_revision()),
        None,
    );
    let (_midi_tx, midi_rx) = crate::runtime_handoff::channel();
    let (_input_tx, input_rx) = crate::runtime_handoff::channel();
    let (_encoder_tx, encoder_rx) = crate::runtime_handoff::channel();
    let inputs = LoopInputs {
        midi_rx: &midi_rx,
        input_rx: &input_rx,
        encoder_rx: &encoder_rx,
    };
    run_runtime_loop(
        &mut state,
        &mut playback,
        &mut runner,
        &mut adapter,
        &worker,
        &inputs,
        &mut InterruptedBoard,
    )
    .unwrap();

    let saved: Value = serde_json::from_slice(&std::fs::read(&default_path).unwrap()).unwrap();
    assert_eq!(saved, edited);
    worker.abort().unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn the_exit_save_is_still_written_when_the_first_store_barrier_times_out() {
    let EditedStore {
        root,
        default_path,
        edited,
        mut playback,
        mut runner,
        mut adapter,
    } = edited_store("octessera-exit-save-busy");
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let (release_tx, release_rx) = mpsc::channel();
    adapter
        .core
        .platform_service
        .enqueue(crate::platform_service::PlatformJob::new(
            playback_runtime::RuntimePlatformRequest::new(
                playback_runtime::RuntimePlatformEffect::SystemInfoRequest,
                "busy-store".into(),
                None,
            ),
            crate::platform_service::PlatformJobKind::TestGate {
                entered: entered_tx,
                release: release_rx,
            },
        ))
        .unwrap();
    entered_rx.recv_timeout(Duration::from_secs(1)).unwrap();
    let barrier_timeout = Duration::from_secs(1);
    let queued_jobs = adapter.core.platform_service.queued_jobs_probe();
    // A job behind the first barrier is only queued once that barrier has timed out.
    let releaser = std::thread::spawn(move || {
        let give_up = Instant::now() + 10 * barrier_timeout;
        while queued_jobs() < 2 && Instant::now() < give_up {
            std::thread::sleep(Duration::from_millis(5));
        }
        release_tx.send(()).unwrap();
    });

    crate::host_power_lifecycle::save_pending_persistence_for_exit_within(
        &mut playback,
        &mut runner,
        &mut adapter,
        barrier_timeout,
    );

    releaser.join().unwrap();
    let saved: Value = serde_json::from_slice(&std::fs::read(&default_path).unwrap()).unwrap();
    assert_eq!(saved, edited);
    let _ = std::fs::remove_dir_all(root);
}
