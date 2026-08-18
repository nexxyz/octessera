use super::OrangeHostAdapter;
use crate::audio::test_service;
use crate::device_update::UpdateExecutor;
use crate::platform_service::PiPlatformService;
use playback_runtime::{
    HostAdapter, HostMessage, RuntimePlatformEffect, RuntimePlatformRequest, RuntimeStoreResult,
};
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Output};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

struct ScriptedExecutor {
    calls: Mutex<VecDeque<String>>,
}

impl ScriptedExecutor {
    fn new() -> Self {
        Self {
            calls: Mutex::new(VecDeque::new()),
        }
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().iter().cloned().collect()
    }
}

impl UpdateExecutor for ScriptedExecutor {
    fn output(&self, command: &mut Command) -> io::Result<Output> {
        let action = command
            .get_args()
            .last()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.calls.lock().unwrap().push_back(action.clone());
        Ok(Output {
            status: success_status(),
            stdout: format!("Orange update {action} scheduled\n").into_bytes(),
            stderr: Vec::new(),
        })
    }
}

#[test]
fn orange_update_effects_use_the_native_updater_worker() {
    let root = temporary_root();
    let (audio, _, _) = test_service();
    let mut adapter = OrangeHostAdapter::with_directories(
        audio,
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
    )
    .unwrap();
    let executor = Arc::new(ScriptedExecutor::new());
    adapter.platform_service = PiPlatformService::new_with_update_executor(
        root.join("store"),
        root.join("samples"),
        executor.clone(),
    );

    for (effect, request_id) in [
        (RuntimePlatformEffect::UpdateCheck, "orange-check"),
        (RuntimePlatformEffect::UpdateApply, "orange-apply"),
        (RuntimePlatformEffect::Rollback, "orange-rollback"),
    ] {
        let response = adapter
            .handle_platform_effect(&RuntimePlatformRequest::new(
                effect,
                request_id.into(),
                Some(7),
            ))
            .unwrap();
        assert!(response.is_empty());
    }
    assert!(!adapter.shutdown_pending());

    let barrier = adapter.platform_service.enqueue_test_barrier().unwrap();
    barrier.recv_timeout(Duration::from_secs(1)).unwrap();
    let results = adapter.drain_results(8);
    assert_eq!(
        executor.calls(),
        vec![
            "check".to_string(),
            "apply".to_string(),
            "rollback".to_string()
        ]
    );
    assert_eq!(results.len(), 3);
    for ((message, request_id), action) in results
        .iter()
        .zip(["orange-check", "orange-apply", "orange-rollback"])
        .zip(["check", "apply", "rollback"])
    {
        let HostMessage::RuntimeResult {
            result:
                RuntimeStoreResult::Identified {
                    result,
                    request_id: actual_request_id,
                    revision,
                },
        } = message
        else {
            panic!("expected identified Orange update result");
        };
        assert_eq!(actual_request_id, request_id);
        assert_eq!(*revision, Some(7));
        assert!(matches!(
            result.as_ref(),
            RuntimeStoreResult::DeviceUpdateStatus { ok: true, message }
                if message == &format!("Orange update {action} scheduled")
        ));
    }

    drop(adapter);
    let _ = std::fs::remove_dir_all(root);
}

fn temporary_root() -> PathBuf {
    std::env::temp_dir().join(format!(
        "octessera-orange-update-host-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

#[cfg(windows)]
fn success_status() -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;

    ExitStatus::from_raw(0)
}

#[cfg(unix)]
fn success_status() -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;

    ExitStatus::from_raw(0)
}
