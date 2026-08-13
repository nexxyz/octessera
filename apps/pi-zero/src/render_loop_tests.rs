use super::*;
use crate::oled_frame_cache::OledFramePublication;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use octessera_hal::OledSsd1351;
use playback_runtime::oled_frame::OLED_FRAME_BYTES;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use serde_json::json;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
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
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn initial_snapshot_rejects_missing_or_mismatched_native_frame() {
    let make_worker = || {
        let (seesaw_tx, _seesaw_rx) = mpsc::channel();
        RenderWorker::spawn(HardwareRenderTargets {
            oled: OledSsd1351::new().unwrap(),
            seesaw_tx,
            oled_handoff: None,
            hdmi: None,
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
