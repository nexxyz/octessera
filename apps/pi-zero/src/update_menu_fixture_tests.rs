use crate::device_update::UpdateExecutor;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::runtime_loop::handle_deferred_host_work;
use crate::usb_config::UsbAudioOut;
use platform_core::DeviceInput;
use playback_runtime::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, PlaybackRuntime, RunnerMessage,
    RuntimeConfig, RuntimeDispatchInput, RuntimePlatformEffect, RuntimeTransportState,
};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SERVICE_BARRIER_TIMEOUT: Duration = Duration::from_secs(1);
const UPDATE_RESULT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, PartialEq, Eq)]
struct CommandCall {
    program: String,
    args: Vec<String>,
}

#[derive(Clone)]
struct UpdateResponse {
    success: bool,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

struct ScriptedExecutor {
    calls: Mutex<Vec<CommandCall>>,
    responses: Mutex<VecDeque<UpdateResponse>>,
    outputs: Mutex<Vec<Vec<u8>>>,
}

impl ScriptedExecutor {
    fn new(responses: impl IntoIterator<Item = UpdateResponse>) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            responses: Mutex::new(responses.into_iter().collect()),
            outputs: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<CommandCall> {
        self.calls.lock().unwrap().clone()
    }

    fn outputs(&self) -> Vec<Vec<u8>> {
        self.outputs.lock().unwrap().clone()
    }
}

impl UpdateExecutor for ScriptedExecutor {
    fn output(&self, command: &mut Command) -> io::Result<Output> {
        self.calls.lock().unwrap().push(CommandCall {
            program: command.get_program().to_string_lossy().into_owned(),
            args: command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect(),
        });
        let response = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("fixture response for updater command");
        self.outputs.lock().unwrap().push(response.stdout.clone());
        Ok(Output {
            status: exit_status(response.success),
            stdout: response.stdout,
            stderr: response.stderr,
        })
    }
}

struct UpdateMenuFixture {
    root: PathBuf,
    executor: Arc<ScriptedExecutor>,
    playback: PlaybackRuntime,
    runner: NativeRunner,
    adapter: PiPlaybackHostAdapter,
    platform_effects: Vec<RuntimePlatformEffect>,
}

impl UpdateMenuFixture {
    fn new(responses: impl IntoIterator<Item = UpdateResponse>) -> Self {
        let root = temporary_root();
        std::fs::create_dir_all(&root).unwrap();
        let executor = Arc::new(ScriptedExecutor::new(responses));
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.skip_startup_splash();
        let mut fixture = Self {
            root: root.clone(),
            executor: executor.clone(),
            playback: PlaybackRuntime::new(RuntimeConfig::default()),
            adapter: PiPlaybackHostAdapter::new_with_update_executor(
                None,
                root.join("presets"),
                root.join("samples"),
                Arc::new(|_| {}),
                false,
                UsbAudioOut::Jack,
                executor,
            ),
            runner,
            platform_effects: Vec::new(),
        };
        fixture.dispatch_input(DeviceInput::Other);
        fixture
    }

    fn dispatch_input(&mut self, input: DeviceInput) {
        let runner_messages = self
            .runner
            .send(HostMessage::DeviceInput {
                input: serde_json::to_value(input).unwrap(),
                request_snapshot: Some(true),
            })
            .unwrap();
        for message in &runner_messages {
            if let RunnerMessage::PlatformEffects { effects } = message {
                self.platform_effects.extend(effects.iter().cloned());
            }
        }
        self.playback
            .dispatch(
                RuntimeDispatchInput::RunnerMessages(runner_messages),
                &mut self.runner,
                &mut self.adapter,
            )
            .unwrap();
    }

    fn focus_and_press(&mut self, key: &str, label: &str) {
        assert_eq!(self.runner.test_focus_menu_item(key).unwrap(), label);
        self.dispatch_input(DeviceInput::EncoderPress {
            id: Some("main".into()),
        });
    }

    fn assert_dialog(&self, title: &str, lines: Value) {
        assert_eq!(self.snapshot()["display"]["title"], title);
        assert_eq!(self.snapshot()["display"]["lines"], lines);
    }

    fn confirm_current_dialog(&mut self) {
        self.dispatch_input(DeviceInput::EncoderTurn {
            delta: 1,
            id: Some("main".into()),
        });
        self.dispatch_input(DeviceInput::EncoderPress {
            id: Some("main".into()),
        });
    }

    fn cancel_current_dialog(&mut self) {
        self.dispatch_input(DeviceInput::ButtonA {
            pressed: Some(true),
        });
    }

    fn default_current_dialog(&mut self) {
        self.dispatch_input(DeviceInput::EncoderPress {
            id: Some("main".into()),
        });
    }

    fn wait_for_toast(&mut self, message: &str) {
        let deadline = Instant::now() + UPDATE_RESULT_TIMEOUT;
        loop {
            handle_deferred_host_work(&mut self.playback, &mut self.runner, &mut self.adapter)
                .unwrap();
            if self
                .toast()
                .is_some_and(|toast| toast == message || message.contains(toast))
            {
                return;
            }
            if Instant::now() >= deadline {
                panic!(
                    "timed out waiting for native toast {message:?}; calls={:?}; effects={:?}; status={:?}; snapshot={:?}",
                    self.calls(),
                    self.platform_effects,
                    self.playback.last_status(),
                    self.snapshot()
                );
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn wait_for_service_barrier(&self) {
        let completed = self
            .adapter
            .platform_service
            .enqueue_test_barrier()
            .unwrap();
        if completed.recv_timeout(SERVICE_BARRIER_TIMEOUT).is_err() {
            panic!(
                "timed out waiting for Pi platform FIFO barrier; calls={:?}; effects={:?}; status={:?}; snapshot={:?}",
                self.calls(),
                self.platform_effects,
                self.playback.last_status(),
                self.snapshot()
            );
        }
    }

    fn snapshot(&self) -> &Value {
        self.playback.last_snapshot().expect("fixture snapshot")
    }

    fn toast(&self) -> Option<&str> {
        self.snapshot()["display"]["toast"].as_str()
    }

    fn calls(&self) -> Vec<CommandCall> {
        self.executor.calls()
    }

    fn assert_last_update_output(&self, expected: &str) {
        assert_eq!(
            self.executor
                .outputs()
                .last()
                .map(|output| { String::from_utf8_lossy(output).trim().to_string() }),
            Some(expected.into())
        );
    }

    fn assert_last_platform_effect(&self, expected: RuntimePlatformEffect) {
        assert_eq!(self.platform_effects.last(), Some(&expected));
    }

    fn assert_playing(&self) {
        assert_eq!(
            self.playback.last_status().map(|status| &status.transport),
            Some(&RuntimeTransportState::Playing)
        );
    }

    fn cleanup(self) {
        let root = self.root.clone();
        drop(self);
        let _ = std::fs::remove_dir_all(root);
    }
}

#[test]
fn native_update_menu_fixture_exercises_pi_update_flow() {
    let mut fixture = UpdateMenuFixture::new([
        response(false, b"", b"check unavailable\n"),
        response(true, b"Update health validation scheduled.\n", b""),
        response(false, b"", b"rollback broke\n"),
    ]);

    fixture.dispatch_input(DeviceInput::ButtonS {
        pressed: Some(true),
    });
    fixture.dispatch_input(DeviceInput::ButtonS {
        pressed: Some(false),
    });
    fixture.assert_playing();

    fixture.focus_and_press("system.updateCheck", "Check");
    assert!(!fixture.runner.test_confirmation_is_open());
    fixture.assert_last_platform_effect(RuntimePlatformEffect::UpdateCheck);
    fixture.wait_for_toast("check unavailable");
    assert_eq!(fixture.calls(), vec![expected_call("check")]);
    fixture.assert_playing();

    fixture.focus_and_press("system.updateApply", "Apply");
    fixture.assert_dialog(
        "Confirm Update",
        json!(["Apply the update now?", "> Cancel", "  Confirm"]),
    );
    fixture.default_current_dialog();
    fixture.wait_for_service_barrier();
    assert_eq!(fixture.calls(), vec![expected_call("check")]);

    fixture.focus_and_press("system.updateApply", "Apply");
    fixture.assert_dialog(
        "Confirm Update",
        json!(["Apply the update now?", "> Cancel", "  Confirm"]),
    );
    fixture.cancel_current_dialog();
    fixture.wait_for_service_barrier();
    assert_eq!(fixture.calls(), vec![expected_call("check")]);

    fixture.focus_and_press("system.updateApply", "Apply");
    fixture.assert_dialog(
        "Confirm Update",
        json!(["Apply the update now?", "> Cancel", "  Confirm"]),
    );
    fixture.confirm_current_dialog();
    fixture.assert_playing();
    fixture.assert_last_platform_effect(RuntimePlatformEffect::UpdateApply);
    fixture.wait_for_toast("Update health validation scheduled.");
    fixture.assert_last_update_output("Update health validation scheduled.");
    assert_eq!(
        fixture.calls(),
        vec![expected_call("check"), expected_call("apply")]
    );

    fixture.focus_and_press("system.rollback", "Rollback");
    fixture.assert_dialog(
        "Confirm Rollback",
        json!([
            "Rollback to the previous",
            "release?",
            "> Cancel",
            "  Confirm"
        ]),
    );
    fixture.default_current_dialog();
    fixture.wait_for_service_barrier();
    assert_eq!(
        fixture.calls(),
        vec![expected_call("check"), expected_call("apply")]
    );

    fixture.focus_and_press("system.rollback", "Rollback");
    fixture.assert_dialog(
        "Confirm Rollback",
        json!([
            "Rollback to the previous",
            "release?",
            "> Cancel",
            "  Confirm"
        ]),
    );
    fixture.cancel_current_dialog();
    fixture.wait_for_service_barrier();
    assert_eq!(
        fixture.calls(),
        vec![expected_call("check"), expected_call("apply")]
    );

    fixture.focus_and_press("system.rollback", "Rollback");
    fixture.assert_dialog(
        "Confirm Rollback",
        json!([
            "Rollback to the previous",
            "release?",
            "> Cancel",
            "  Confirm"
        ]),
    );
    fixture.confirm_current_dialog();
    fixture.assert_last_platform_effect(RuntimePlatformEffect::Rollback);
    fixture.wait_for_toast("rollback broke");
    assert_eq!(
        fixture.calls(),
        vec![
            expected_call("check"),
            expected_call("apply"),
            expected_call("rollback")
        ]
    );
    fixture.assert_playing();
    fixture.cleanup();
}

fn response(success: bool, stdout: &[u8], stderr: &[u8]) -> UpdateResponse {
    UpdateResponse {
        success,
        stdout: stdout.to_vec(),
        stderr: stderr.to_vec(),
    }
}

fn expected_call(action: &str) -> CommandCall {
    CommandCall {
        program: "sudo".into(),
        args: vec![
            "-n".into(),
            "/usr/local/sbin/octessera-update".into(),
            action.into(),
        ],
    }
}

fn temporary_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "octessera-pi-update-menu-{}-{nanos}",
        std::process::id()
    ))
}

#[cfg(windows)]
fn exit_status(success: bool) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(if success { 0 } else { 1 })
}

#[cfg(unix)]
fn exit_status(success: bool) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(if success { 0 } else { 1 })
}
