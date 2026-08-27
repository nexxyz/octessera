use super::*;
use crate::oled_frame_cache::OledFramePublication;
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
use octessera_hal::OledSsd1351;
use playback_runtime::oled_frame::OLED_FRAME_BYTES;
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
use serde_json::json;
#[cfg(all(
    unix,
    not(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))
))]
use std::fs;
use std::time::Instant;

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn native_snapshot() -> Value {
    json!({
        "display": { "off": false },
        "settings": { "buttonBrightness": 100, "displayBrightness": 100 },
        "leds": { "rgb": vec![0; 64 * 3] },
        "transport": { "playing": false },
        "transportIcon": "stop",
        "transportFlash": "none",
        "eventDotOn": false,
        "oledFrameRevision": 1,
        "neoKeyLeds": {
            "back": [0, 0, 0],
            "space": [0, 0, 0],
            "shift": [0, 0, 0],
            "fn": [0, 0, 0]
        }
    })
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn terminal_snapshot(revision: u64) -> Value {
    let mut snapshot = native_snapshot();
    snapshot["oledFrameRevision"] = serde_json::json!(revision);
    snapshot
}

#[test]
fn shutdown_ack_preserves_display_off_failure() {
    let (ack_tx, ack_rx) = mpsc::channel();
    ack_tx
        .send(display_off_ack(Err("SPI write failed".into())))
        .unwrap();
    assert_eq!(
        ack_rx.recv().unwrap(),
        Err("OLED display-off failed: SPI write failed".into())
    );
}

#[test]
fn shutdown_ack_timeout_is_bounded() {
    assert_eq!(SHUTDOWN_ACK_TIMEOUT, Duration::from_millis(750));
    assert_eq!(INITIAL_RENDER_ACK_TIMEOUT, Duration::from_millis(750));
    assert_eq!(OWNERSHIP_ACK_TIMEOUT, Duration::from_secs(2));
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn preserving_terminal_teardown_acknowledges_and_joins_worker() {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });

    worker
        .publish_terminal_preserving(
            native_snapshot(),
            OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]),
        )
        .unwrap();
    assert!(worker.is_terminated());
}

#[test]
fn terminal_command_requires_an_accepted_matching_native_frame() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let worker = RenderWorker {
        state,
        worker: Arc::new(Mutex::new(None)),
    };
    let snapshot = serde_json::json!({"oledFrameRevision": 7});

    assert_eq!(
        worker.publish_terminal_preserving(snapshot.clone(), OledFramePublication::ExplicitBlack),
        Err("terminal OLED snapshot requires an accepted native frame".into())
    );
    assert_eq!(
        worker.publish_terminal_preserving(
            snapshot.clone(),
            OledFramePublication::test_retained_last_good(7, vec![0; OLED_FRAME_BYTES]),
        ),
        Err("terminal OLED snapshot requires an accepted native frame".into())
    );
    assert_eq!(
        worker.publish_terminal_preserving(
            snapshot,
            OledFramePublication::test_native(6, vec![0; OLED_FRAME_BYTES]),
        ),
        Err("terminal OLED publication does not match snapshot frame revision".into())
    );
}

#[test]
fn snapshot_publication_is_rejected_once_terminal_command_is_queued() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let (ack, _received) = mpsc::channel();
    state.0.lock().unwrap().command = Some(RenderCommand::PreserveTerminal {
        snapshot: Value::Null,
        oled: OledFramePublication::ExplicitBlack,
        ack,
    });
    let worker = RenderWorker {
        state: Arc::clone(&state),
        worker: Arc::new(Mutex::new(None)),
    };

    assert!(!worker.publish_snapshot(Value::Null, OledFramePublication::ExplicitBlack));
    assert!(state.0.lock().unwrap().snapshot.is_none());
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn atomic_terminal_rejects_stale_snapshot_and_uses_supplied_command() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let (stale_ack_tx, stale_ack_rx) = mpsc::channel();
    state.0.lock().unwrap().snapshot = Some(SnapshotCommand {
        snapshot: terminal_snapshot(1),
        oled: OledFramePublication::test_native(1, vec![1; OLED_FRAME_BYTES]),
        rendered_acks: vec![stale_ack_tx],
    });
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_state = Arc::clone(&state);
    let worker_gate = Arc::clone(&gate);
    let (seesaw_tx, seesaw_rx) = mpsc::channel();
    let mut targets = HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    };
    let handle = thread::spawn(move || {
        let (lock, ready) = &*worker_gate;
        let mut open = lock.lock().unwrap();
        while !*open {
            open = ready.wait(open).unwrap();
        }
        drop(open);
        let _seesaw_rx = seesaw_rx;
        render_worker_loop(worker_state, &mut targets);
    });
    let worker = RenderWorker {
        state: Arc::clone(&state),
        worker: Arc::new(Mutex::new(Some(handle))),
    };
    let terminal_snapshot = terminal_snapshot(2);
    let terminal_oled = OledFramePublication::test_native(2, vec![2; OLED_FRAME_BYTES]);
    let terminal_worker = worker.clone();
    let terminal_snapshot_for_thread = terminal_snapshot.clone();
    let terminal_oled_for_thread = terminal_oled.clone();
    let terminal = thread::spawn(move || {
        terminal_worker
            .publish_terminal_preserving(terminal_snapshot_for_thread, terminal_oled_for_thread)
    });

    loop {
        let queued = state
            .0
            .lock()
            .unwrap()
            .command
            .as_ref()
            .is_some_and(|command| {
                matches!(
                    command,
                    RenderCommand::PreserveTerminal { snapshot, oled, .. }
                        if snapshot == &terminal_snapshot && oled.revision() == Some(2)
                )
            });
        if queued {
            break;
        }
        thread::yield_now();
    }
    *gate.0.lock().unwrap() = true;
    gate.1.notify_one();

    assert_eq!(terminal.join().unwrap(), Ok(()));
    assert_eq!(
        stale_ack_rx.recv().unwrap(),
        Err("render worker was preempted by preserving terminal teardown".into())
    );
    assert!(worker.is_terminated());
}

#[test]
fn initial_handoff_validation_requires_exact_native_publication() {
    let snapshot = serde_json::json!({"oledFrameRevision": 7});
    assert_eq!(
        validate_oled_publication(&snapshot, &OledFramePublication::ExplicitBlack, true),
        Err("initial OLED snapshot requires an accepted native frame".into())
    );
    assert_eq!(
        validate_oled_publication(
            &snapshot,
            &OledFramePublication::test_retained_last_good(7, vec![0; OLED_FRAME_BYTES]),
            true,
        ),
        Err("initial OLED snapshot requires an accepted native frame".into())
    );
    assert_eq!(
        validate_oled_publication(
            &snapshot,
            &OledFramePublication::test_native(6, vec![0; OLED_FRAME_BYTES]),
            true,
        ),
        Err("OLED publication does not match snapshot frame revision".into())
    );
    assert!(validate_oled_publication(
        &snapshot,
        &OledFramePublication::test_native(7, vec![0; OLED_FRAME_BYTES]),
        true,
    )
    .is_ok());
}

#[test]
fn pending_snapshot_lane_wins_over_expired_animation_deadline() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    {
        let (lock, _) = &*state;
        let mut guard = lock.lock().unwrap();
        guard.snapshot = Some(SnapshotCommand {
            snapshot: Value::Null,
            oled: OledFramePublication::ExplicitBlack,
            rendered_acks: Vec::new(),
        });
        assert!(pending_work_wins_over_expired_animation_deadline(&guard));
    }

    let command = take_next_command(&state, Some(Instant::now() - Duration::from_millis(1)));
    assert!(matches!(command, Some(RenderCommand::Snapshot { .. })));
}

#[test]
fn pending_work_decision_covers_command_and_snapshot_lanes() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let (ack, _received) = mpsc::channel();
    let mut guard = state.0.lock().unwrap();
    assert!(!pending_work_wins_over_expired_animation_deadline(&guard));

    guard.snapshot = Some(SnapshotCommand {
        snapshot: Value::Null,
        oled: OledFramePublication::ExplicitBlack,
        rendered_acks: Vec::new(),
    });
    assert!(pending_work_wins_over_expired_animation_deadline(&guard));

    guard.snapshot = None;
    guard.command = Some(RenderCommand::MarkFailed { ack });
    assert!(pending_work_wins_over_expired_animation_deadline(&guard));
}

#[test]
fn snapshot_publication_reports_a_poisoned_worker() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let poison_state = Arc::clone(&state);
    thread::spawn(move || {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poison_state.0.lock().unwrap();
            panic!("poison render state");
        }));
    })
    .join()
    .unwrap();
    let worker = RenderWorker {
        state,
        worker: Arc::new(Mutex::new(None)),
    };

    assert!(!worker.publish_snapshot(Value::Null, OledFramePublication::ExplicitBlack));
}

#[test]
fn snapshot_publication_reports_a_pending_shutdown() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let (ack, _received) = mpsc::channel();
    state.0.lock().unwrap().command = Some(RenderCommand::Shutdown { ack });
    let worker = RenderWorker {
        state,
        worker: Arc::new(Mutex::new(None)),
    };

    assert!(!worker.publish_snapshot(Value::Null, OledFramePublication::ExplicitBlack));
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn mark_failed_ack_succeeds_without_an_attached_handoff() {
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });

    assert_eq!(worker.mark_oled_failed(), Ok(()));
    worker.abort().unwrap();
}

#[test]
#[cfg(all(
    unix,
    not(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))
))]
fn mark_failed_ack_reports_failed_status_persistence() {
    let path = std::env::temp_dir().join(format!(
        "octessera-render-failed-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let handoff = crate::boot_oled_handoff::native_guard_for_test(&path).unwrap();
    fs::remove_file(path.join("status.json")).unwrap();
    fs::create_dir(path.join("status.json")).unwrap();
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: Some(handoff),
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });

    assert!(worker
        .mark_oled_failed()
        .unwrap_err()
        .contains("cannot publish OLED handoff status.json"));
    worker.abort().unwrap();
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn ownership_lane_keeps_latest_snapshot_pending() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let (ack, _received) = mpsc::channel();
    state.0.lock().unwrap().command = Some(RenderCommand::Ownership {
        stage: OledOwnershipStage::PrepareRelease,
        cancellation: Arc::new(AtomicBool::new(false)),
        ack,
    });
    let worker = RenderWorker {
        state: Arc::clone(&state),
        worker: Arc::new(Mutex::new(None)),
    };

    assert!(worker.publish_snapshot(Value::Null, OledFramePublication::ExplicitBlack));
    let guard = state.0.lock().unwrap();
    assert!(matches!(
        guard.command,
        Some(RenderCommand::Ownership { .. })
    ));
    assert!(guard.snapshot.is_some());
}

#[test]
fn ownership_timeout_cancels_late_command_execution() {
    let cancellation = Arc::new(AtomicBool::new(false));
    assert!(!ownership_command_cancelled(&cancellation));
    cancel_ownership(&cancellation);
    assert!(ownership_command_cancelled(&cancellation));
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn initial_snapshot_ack_is_current_and_cannot_be_reused() {
    let readiness_path = std::env::temp_dir().join(format!(
        "octessera-render-readiness-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut readiness = crate::candidate_readiness::CandidateReadiness::new(
        Some(readiness_path.clone()),
        "render-test".into(),
    );
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: OledSsd1351::new().unwrap(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    assert_eq!(
        worker.mark_first_menu_rendered(),
        Err("OLED handoff is not acknowledged by a successful native write".into())
    );
    assert!(worker
        .publish_acknowledged_snapshot(
            native_snapshot(),
            OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]),
        )
        .is_ok());
    assert!(!readiness_path.exists());
    readiness.mark_ready().unwrap();
    assert!(readiness_path.is_file());
    assert_eq!(
        worker.publish_acknowledged_snapshot(
            native_snapshot(),
            OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]),
        ),
        Err("render worker rejected a second acknowledged snapshot".into())
    );
    worker.abort().unwrap();
    drop(readiness);
    let _ = std::fs::remove_dir_all(readiness_path);
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn initial_snapshot_rejects_missing_or_mismatched_native_frame() {
    let make_worker = || {
        let (seesaw_tx, _seesaw_rx) = mpsc::channel();
        RenderWorker::spawn(HardwareRenderTargets {
            oled: OledSsd1351::new().unwrap(),
            seesaw_tx,
            oled_handoff: None,
            hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
        })
    };

    let worker = make_worker();
    assert_eq!(
        worker
            .publish_acknowledged_snapshot(native_snapshot(), OledFramePublication::ExplicitBlack,),
        Err("initial OLED snapshot requires an accepted native frame".into())
    );
    worker.abort().unwrap();

    let worker = make_worker();
    let mut snapshot = native_snapshot();
    snapshot["oledFrameRevision"] = serde_json::json!(2);
    assert_eq!(
        worker.publish_acknowledged_snapshot(
            snapshot,
            OledFramePublication::test_native(1, vec![0; OLED_FRAME_BYTES]),
        ),
        Err("OLED publication does not match snapshot frame revision".into())
    );
    worker.abort().unwrap();
}
