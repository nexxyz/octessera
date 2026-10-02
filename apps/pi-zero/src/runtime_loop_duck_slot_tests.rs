use super::{dispatch_runtime_message, handle_deferred_host_work, process_runtime_output};
use crate::host_adapter::PiPlaybackHostAdapter;
use playback_runtime::{
    HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RuntimeConfig,
    RuntimeTransportState, SyncSource, UsbDataRole,
};
use serde_json::{json, Value};
use std::sync::Arc;

struct Replay {
    playback: PlaybackRuntime,
    runner: NativeRunner,
    adapter: PiPlaybackHostAdapter,
    _keep: (
        std::sync::mpsc::Receiver<crate::audio::AudioControlRequest>,
        rodio_engine_source::EngineEventReceiver,
        std::sync::mpsc::Sender<HostMessage>,
    ),
}

impl Replay {
    fn new(root: &std::path::Path) -> Self {
        let payload: Value =
            serde_json::from_str(include_str!("../../../config/generated/pi/default.json"))
                .unwrap();
        crate::pi_store_test_support::write_pair(&root.join("store"), &payload);
        let (audio, control_rx, event_rx, prep_tx) =
            crate::audio::test_service_with_recording_dir(root.join("recording"));
        let mut adapter = PiPlaybackHostAdapter::new_with_data_role(
            Some(audio),
            root.join("store"),
            root.join("samples"),
            Arc::new(|_| {}),
            false,
            playback_runtime::AudioOutputSet::jack(),
            UsbDataRole::Gadget,
        );
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.apply_config_payload(payload).unwrap();
        runner.skip_startup_splash();
        let mut playback = PlaybackRuntime::new(RuntimeConfig {
            sync_source: SyncSource::Internal,
            ..RuntimeConfig::default()
        });
        let output = playback
            .dispatch_runner_messages(
                runner.messages_with_snapshot().unwrap(),
                &mut runner,
                &mut adapter,
            )
            .unwrap();
        process_runtime_output(&mut playback, &mut runner, &mut adapter, output).unwrap();
        Self {
            playback,
            runner,
            adapter,
            _keep: (control_rx, event_rx, prep_tx),
        }
    }

    fn input(&mut self, input: Value) {
        let message = HostMessage::DeviceInput {
            input,
            request_snapshot: Some(true),
        };
        dispatch_runtime_message(
            &mut self.playback,
            &mut self.runner,
            &mut self.adapter,
            message,
        )
        .unwrap();
        handle_deferred_host_work(&mut self.playback, &mut self.runner, &mut self.adapter).unwrap();
        self.playback.request_next_snapshot();
        let output = self
            .playback
            .dispatch_runtime_tick(&mut self.runner, &mut self.adapter)
            .unwrap();
        process_runtime_output(
            &mut self.playback,
            &mut self.runner,
            &mut self.adapter,
            output,
        )
        .unwrap();
    }

    fn turn(&mut self, delta: i32) {
        self.input(json!({ "type": "encoder_turn", "id": "main", "delta": delta }));
    }

    fn press(&mut self) {
        self.input(json!({ "type": "encoder_press", "id": "main" }));
    }

    fn back(&mut self) {
        self.input(json!({ "type": "button_a", "pressed": true }));
        self.input(json!({ "type": "button_a", "pressed": false }));
    }

    fn lines(&self) -> Vec<String> {
        self.playback.last_snapshot().unwrap()["display"]["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|line| line.as_str().map(str::to_string))
            .collect()
    }

    fn select(&mut self, label: &str) {
        for _ in 0..64 {
            if self
                .lines()
                .iter()
                .any(|line| line.starts_with('>') && line.contains(label))
            {
                return;
            }
            self.turn(1);
        }
        panic!("`{label}` not reachable in {:?}", self.lines());
    }

    fn open(&mut self, label: &str) {
        self.select(label);
        self.press();
    }

    fn edit_until(&mut self, label: &str) {
        for _ in 0..32 {
            if self.lines().iter().any(|line| {
                line.trim_start().starts_with('*') && line.to_lowercase().contains(label)
            }) {
                self.press();
                return;
            }
            self.turn(1);
        }
        panic!("option `{label}` not reachable in {:?}", self.lines());
    }

    fn back_to_root(&mut self) {
        for _ in 0..8 {
            let lines = self.lines();
            if lines.iter().any(|line| line.contains("Build"))
                && lines.iter().any(|line| line.contains("System"))
            {
                return;
            }
            self.back();
        }
        panic!("root menu not reachable");
    }

    fn assert_healthy(&self, step: &str) {
        let status = self.playback.last_status().expect("runtime status");
        assert!(
            status.error.is_none(),
            "{step}: runtime error {:?}",
            status.error
        );
        assert_eq!(
            status.transport,
            RuntimeTransportState::Playing,
            "{step}: transport"
        );
    }
}

#[test]
fn duck_on_newly_routed_bus_keeps_playback_through_type_scroll_and_amount_edit() {
    let root = std::env::temp_dir().join(format!("octessera-duck-replay-{}", std::process::id()));
    let mut replay = Replay::new(&root);
    for pressed in [true, false] {
        replay.input(json!({ "type": "button_s", "pressed": pressed }));
    }
    replay.assert_healthy("play");
    for group in ["Shape", "Instruments", "I3:", "Mixer"] {
        replay.open(group);
    }
    replay.select("Route");
    replay.press();
    replay.turn(2);
    replay.press();
    replay.assert_healthy("route I3 to B3");
    replay.back_to_root();
    for group in ["Shape", "FX Buses", "B3:", "Slot 1"] {
        replay.open(group);
    }
    replay.select("Type");
    replay.press();
    replay.edit_until("duck");
    replay.assert_healthy("select duck");
    replay.select("Amount");
    replay.press();
    replay.turn(5);
    replay.press();
    replay.assert_healthy("edit duck amount");
    let _ = std::fs::remove_dir_all(root);
}
