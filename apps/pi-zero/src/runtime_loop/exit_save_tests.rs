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

#[test]
fn a_termination_signal_writes_the_coalesced_autosave_before_the_loop_ends() {
    let root = crate::test_temp_dir::unique_temp_path("octessera-exit-save");
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
    let (_midi_tx, midi_rx) = mpsc::channel();
    let (_input_tx, input_rx) = mpsc::channel();
    let (_encoder_tx, encoder_rx) = mpsc::channel();
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
