use playback_runtime::{CoreRunner, HostMessage, RunnerMessage};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(3);

enum ReaderEvent {
    Batch(Result<Vec<RunnerMessage>, String>),
    Exited,
}

pub struct NodeRunnerProcess {
    child: Child,
    stdin: ChildStdin,
    reader_handle: Option<JoinHandle<()>>,
    response_rx: mpsc::Receiver<ReaderEvent>,
}

impl NodeRunnerProcess {
    pub fn spawn_default(workspace_root: impl AsRef<Path>) -> Result<Self, String> {
        let workspace_root = workspace_root.as_ref();
        let runner_pkg_dir = workspace_root.join("packages").join("platform-core-runner");

        let tsx_cli = runner_pkg_dir
            .join("node_modules")
            .join("tsx")
            .join("dist")
            .join("cli.mjs");

        if !tsx_cli.is_file() {
            return Err(format!(
                "tsx CLI not found at `{}` (is tsx installed in packages/platform-core-runner?)",
                tsx_cli.display()
            ));
        }

        let runner_entry = runner_pkg_dir.join("src").join("main.ts");

        let mut command = Command::new("node");
        command
            .arg(&tsx_cli)
            .arg(&runner_entry)
            .current_dir(workspace_root);

        Self::spawn(command)
    }

    pub fn spawn(mut command: Command) -> Result<Self, String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|err| format!("failed to spawn node runner: {err}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "node runner stdin unavailable".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "node runner stdout unavailable".to_string())?;

        let (response_tx, response_rx) = mpsc::channel::<ReaderEvent>();
        let reader_handle = std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let batch = Self::read_batch(&mut reader);
                let is_fatal = batch.is_err();
                if response_tx.send(ReaderEvent::Batch(batch)).is_err() {
                    break;
                }
                if is_fatal {
                    break;
                }
            }
            let _ = response_tx.send(ReaderEvent::Exited);
        });

        Ok(Self {
            child,
            stdin,
            reader_handle: Some(reader_handle),
            response_rx,
        })
    }

    fn read_batch(reader: &mut BufReader<ChildStdout>) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = Vec::new();
        loop {
            let mut line = String::new();
            let read = reader
                .read_line(&mut line)
                .map_err(|err| format!("failed reading node runner stdout: {err}"))?;
            if read == 0 {
                return if messages.is_empty() {
                    Err("node runner stdout closed".to_string())
                } else {
                    Err("node runner stdout closed mid-batch".to_string())
                };
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let message = serde_json::from_str::<RunnerMessage>(trimmed)
                .map_err(|err| format!("invalid node runner message `{trimmed}`: {err}"))?;
            let is_terminal = matches!(message, RunnerMessage::RuntimeStatus { .. });
            messages.push(message);
            if is_terminal {
                return Ok(messages);
            }
        }
    }
}

impl CoreRunner for NodeRunnerProcess {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let line = serde_json::to_string(&message)
            .map_err(|err| format!("failed to encode host message: {err}"))?;
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|err| format!("failed writing to node runner stdin: {err}"))?;

        match self.response_rx.recv_timeout(READ_TIMEOUT) {
            Ok(ReaderEvent::Batch(result)) => result,
            Ok(ReaderEvent::Exited) => Err("node runner reader thread exited".to_string()),
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let _ = self.child.kill();
                match self.response_rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(ReaderEvent::Batch(result)) => result,
                    Ok(ReaderEvent::Exited) | Err(_) => Err(format!(
                        "node runner did not respond within {}s",
                        READ_TIMEOUT.as_secs()
                    )),
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("node runner reader thread disconnected".to_string())
            }
        }
    }
}

impl Drop for NodeRunnerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(handle) = self.reader_handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn workspace_root_from(crate_dir: impl AsRef<Path>) -> PathBuf {
    let start = crate_dir.as_ref();
    for ancestor in start.ancestors() {
        if ancestor.join("pnpm-workspace.yaml").is_file()
            && ancestor.join("packages").is_dir()
            && ancestor.join("Cargo.toml").is_file()
        {
            return ancestor.to_path_buf();
        }
        if ancestor
            .file_name()
            .is_some_and(|name| name == "crates" || name == "apps")
        {
            if let Some(parent) = ancestor.parent() {
                return parent.to_path_buf();
            }
        }
    }
    start.to_path_buf()
}
