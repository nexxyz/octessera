use super::*;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use octessera_hal::OledSsd1351;

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
fn pending_snapshot_lane_wins_over_expired_animation_deadline() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    {
        let (lock, _) = &*state;
        let mut guard = lock.lock().unwrap();
        guard.snapshot = Some(SnapshotCommand {
            snapshot: Value::Null,
            pulses: Vec::new(),
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
        pulses: Vec::new(),
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

    assert!(!worker.publish_snapshot(Value::Null, Vec::new()));
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

    assert!(!worker.publish_snapshot(Value::Null, Vec::new()));
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

    assert!(worker.publish_snapshot(Value::Null, Vec::new()));
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
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
        #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
        hdmi: None,
    });
    assert!(worker
        .publish_acknowledged_snapshot(Value::Null, Vec::new())
        .is_ok());
    assert!(!readiness_path.exists());
    readiness.mark_ready().unwrap();
    assert!(readiness_path.is_file());
    assert_eq!(
        worker.publish_acknowledged_snapshot(Value::Null, Vec::new()),
        Err("render worker rejected a second acknowledged snapshot".into())
    );
    worker.abort().unwrap();
    drop(readiness);
    let _ = std::fs::remove_dir_all(readiness_path);
}
