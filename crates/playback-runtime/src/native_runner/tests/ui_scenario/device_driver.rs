use super::*;
use crate::runtime::CoreRunner;

use super::captured_output::CapturedOutput;

pub(super) struct DeviceDriver {
    runner: NativeRunner,
    latest_snapshot: Value,
    trace: Vec<String>,
    output: CapturedOutput,
}

impl DeviceDriver {
    pub(super) fn new() -> Self {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let messages = runner
            .send(HostMessage::DeviceInput {
                input: json!({ "type": "other" }),
                request_snapshot: Some(true),
            })
            .unwrap();
        let latest_snapshot = snapshot_from(&messages);
        let mut output = CapturedOutput::default();
        output.record(&messages);
        Self {
            runner,
            latest_snapshot,
            trace: vec!["boot snapshot".into()],
            output,
        }
    }

    pub(super) fn note_step(&mut self, name: impl Into<String>) {
        self.trace.push(name.into());
    }

    pub(super) fn turn_main(&mut self, delta: i32) {
        self.send_device(json!({ "type": "encoder_turn", "id": "main", "delta": delta }));
    }

    pub(super) fn press_main(&mut self) {
        self.send_device(json!({ "type": "encoder_press", "id": "main" }));
    }

    pub(super) fn press_grid(&mut self, x: usize, y: usize) {
        self.send_device(json!({ "type": "grid_press", "x": x, "y": y }));
    }

    #[allow(dead_code)]
    pub(super) fn release_grid(&mut self, x: usize, y: usize) {
        self.send_device(json!({ "type": "grid_release", "x": x, "y": y }));
    }

    pub(super) fn press_button(&mut self, id: &str) {
        self.send_device(button_input(id, true));
        self.send_device(button_input(id, false));
    }

    pub(super) fn hold_button(&mut self, id: &str) {
        self.send_device(button_input(id, true));
    }

    #[allow(dead_code)]
    pub(super) fn release_button(&mut self, id: &str) {
        self.send_device(button_input(id, false));
    }

    #[allow(dead_code)]
    pub(super) fn turn_aux(&mut self, id: &str, delta: i32) {
        self.send_device(json!({ "type": "encoder_turn", "id": id, "delta": delta }));
    }

    #[allow(dead_code)]
    pub(super) fn press_aux(&mut self, id: &str) {
        self.send_device(json!({ "type": "encoder_press", "id": id }));
    }

    pub(super) fn start(&mut self) {
        self.send(HostMessage::MidiRealtimeStart);
    }

    pub(super) fn configure_external_clock(&mut self) {
        self.runner.transport.sync_source = SyncSource::External;
        self.runner.midi_clock_in_enabled = true;
        self.runner.midi_respond_to_start_stop = true;
        self.refresh_snapshot();
    }

    pub(super) fn set_external_clock_position(&mut self, pulse: u64) {
        self.runner.transport.current_ppqn_pulse = pulse;
        self.refresh_snapshot();
    }

    pub(super) fn arm_external_resync(&mut self) {
        self.hold_button("shift");
        self.press_button("play");
        self.release_button("shift");
    }

    pub(super) fn external_clock(&mut self, pulses: u32) {
        self.send(HostMessage::MidiRealtimeClock { pulses });
    }

    pub(super) fn pending_resync(&self) -> bool {
        self.runner.transport.pending_resync
    }

    pub(super) fn active_grid_cell(&self, x: usize, y: usize) -> bool {
        self.runner.engine.model().unwrap().cells[platform_core::grid_index(x, y)]
    }

    pub(super) fn clock_pulses(&mut self, pulses: u32) {
        self.send(HostMessage::TransportPulseStep {
            pulses,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        });
    }

    pub(super) fn snapshot(&self) -> &Value {
        &self.latest_snapshot
    }

    pub(super) fn output(&self) -> &CapturedOutput {
        &self.output
    }

    pub(super) fn config_payload(&self) -> Value {
        self.runner.config_payload()
    }

    pub(super) fn set_preset_draft_name(&mut self, name: &str) {
        self.runner.preset_draft_name = name.into();
        self.runner.menu.rebuild(self.runner.menu_config());
        self.refresh_snapshot();
    }

    pub(super) fn send_store_result(&mut self, result: RuntimeStoreResult) {
        self.send(HostMessage::RuntimeResult { result });
    }

    pub(super) fn latest_saved_preset(&self) -> Option<(String, Value)> {
        self.output.saved_presets.last().cloned()
    }

    pub(super) fn latest_load_preset_request(&self) -> Option<&str> {
        self.output.load_preset_requests.last().map(String::as_str)
    }

    pub(super) fn select_layer_with_fn(&mut self, layer_index: usize) {
        self.hold_button("fn");
        self.press_grid(0, layer_index);
        self.release_grid(0, layer_index);
        self.release_button("fn");
    }

    pub(super) fn select_sparks_page_with_fn(&mut self, y: usize) {
        self.hold_button("fn");
        self.press_grid(7, y);
        self.release_grid(7, y);
        self.release_button("fn");
    }

    pub(super) fn toggle_active_layer_mute(&mut self) {
        self.hold_button("fn");
        self.press_button("play");
        self.release_button("fn");
    }

    pub(super) fn respond_to_latest_sample_list_request(&mut self, entries: Vec<SampleEntry>) {
        let Some((instrument_slot, sample_slot, dir)) =
            self.output.sample_list_requests.last().cloned()
        else {
            self.fail("no sample list request captured");
        };
        self.send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SampleListResult {
                instrument_slot,
                sample_slot,
                dir,
                entries,
            },
        });
    }

    pub(super) fn fail(&self, message: &str) -> ! {
        panic!(
            "{message}\ntrace:\n{}\nactive behavior: {}\nactive sparks: {}\ntransport: {}\ntoast: {}\noutput counts: audio={} synth={} sample={} fx_start={} fx_stop={}\nlatest OLED:\n{}",
            self.trace.join("\n"),
            self.latest_snapshot["activeBehavior"]
                .as_str()
                .unwrap_or("?"),
            self.latest_snapshot["activeSparksMode"]
                .as_str()
                .unwrap_or("?"),
            self.latest_snapshot["transportIcon"]
                .as_str()
                .unwrap_or("?"),
            self.latest_snapshot["display"]["toast"]
                .as_str()
                .unwrap_or(""),
            self.output.audio_command_count,
            self.output.synth_param_count,
            self.output.sample_bank_param_count,
            self.output.momentary_fx_start_count,
            self.output.momentary_fx_stop_count,
            self.oled_lines().join("\n")
        );
    }

    fn send_device(&mut self, input: Value) {
        self.send(HostMessage::DeviceInput {
            input,
            request_snapshot: Some(true),
        });
    }

    fn refresh_snapshot(&mut self) {
        let messages = self.runner.snapshot().map_or_else(
            |error| self.fail(&format!("snapshot failed: {error}")),
            |snapshot| vec![RunnerMessage::Snapshot { snapshot }],
        );
        self.output.record(&messages);
        if let RunnerMessage::Snapshot { snapshot } = &messages[0] {
            self.latest_snapshot = snapshot.clone();
        }
    }

    fn send(&mut self, message: HostMessage) {
        let messages = self.runner.send(message).unwrap_or_else(|error| {
            self.fail(&format!("send failed: {error}"));
        });
        self.output.record(&messages);
        if let Some(snapshot) = messages.iter().rev().find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot.clone()),
            _ => None,
        }) {
            self.latest_snapshot = snapshot;
        } else {
            self.fail("input did not emit a natural snapshot");
        }
    }

    fn oled_lines(&self) -> Vec<String> {
        self.latest_snapshot["display"]["lines"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect()
    }
}

fn button_input(id: &str, pressed: bool) -> Value {
    let input_type = match id {
        "back" | "a" => "button_a",
        "play" | "s" => "button_s",
        "shift" => "button_shift",
        "fn" => "button_fn",
        "combined" => "button_combined_modifier",
        _ => "other",
    };
    json!({ "type": input_type, "pressed": pressed })
}
