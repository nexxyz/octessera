use super::test_scenes::{test_scene, test_scene_command};
use super::*;
#[cfg(all(
    unix,
    not(any(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "hardware-orange-pi-zero-2w"
    ))
))]
use std::fs;
use std::time::Instant;

fn idle_worker(state: Arc<(Mutex<RenderState>, Condvar)>) -> RenderWorker {
    RenderWorker {
        state,
        worker: Arc::new(Mutex::new(None)),
    }
}

fn rendered_state() -> Arc<(Mutex<RenderState>, Condvar)> {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    state.0.lock().unwrap().acknowledged_snapshot_rendered = true;
    state
}

#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn spawn_fake_worker() -> (
    RenderWorker,
    mpsc::Receiver<crate::seesaw_io::SeesawCommand>,
) {
    let (seesaw_tx, seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
        seesaw_tx,
        oled_handoff: None,
        hdmi: crate::render::hdmi::HdmiFramebuffer::new(),
    });
    (worker, seesaw_rx)
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
    let (worker, _seesaw_rx) = spawn_fake_worker();

    worker
        .publish_terminal_preserving(test_scene(), Default::default(), None)
        .unwrap();
    assert!(worker.is_terminated());
}

#[test]
fn native_scene_is_rejected_once_terminal_command_is_queued() {
    let state = rendered_state();
    let (ack, _received) = mpsc::channel();
    let (scene, _completion) = test_scene_command();
    state.0.lock().unwrap().command = Some(RenderCommand::PreserveTerminal { scene, ack });
    let worker = idle_worker(Arc::clone(&state));

    assert!(worker
        .publish_native_scene(test_scene(), Default::default(), None)
        .is_err());
    assert!(state.0.lock().unwrap().snapshot.is_none());
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn atomic_terminal_preempts_a_pending_scene() {
    let state = rendered_state();
    let (stale, stale_completion) = test_scene_command();
    state.0.lock().unwrap().snapshot = Some(stale);
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let worker_state = Arc::clone(&state);
    let worker_gate = Arc::clone(&gate);
    let (seesaw_tx, seesaw_rx) = mpsc::channel();
    let mut targets = HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
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
    let terminal_worker = worker.clone();
    let terminal = thread::spawn(move || {
        terminal_worker.publish_terminal_preserving(test_scene(), Default::default(), None)
    });

    while !matches!(
        state.0.lock().unwrap().command,
        Some(RenderCommand::PreserveTerminal { .. })
    ) {
        thread::yield_now();
    }
    *gate.0.lock().unwrap() = true;
    gate.1.notify_one();

    assert_eq!(terminal.join().unwrap(), Ok(()));
    assert_eq!(
        stale_completion.recv().unwrap().result,
        Err("render worker was preempted by preserving terminal teardown".into())
    );
    assert!(worker.is_terminated());
}

#[test]
fn pending_scene_lane_wins_over_expired_animation_deadline() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    {
        let mut guard = state.0.lock().unwrap();
        guard.snapshot = Some(test_scene_command().0);
        assert!(pending_work_wins_over_expired_animation_deadline(&guard));
    }

    let command = take_next_command(&state, Some(Instant::now() - Duration::from_millis(1)));
    assert!(matches!(command, Some(RenderCommand::NativeSnapshot(_))));
}

#[test]
fn pending_work_decision_covers_command_and_scene_lanes() {
    let state = Arc::new((Mutex::new(RenderState::default()), Condvar::new()));
    let (ack, _received) = mpsc::channel();
    let mut guard = state.0.lock().unwrap();
    assert!(!pending_work_wins_over_expired_animation_deadline(&guard));

    guard.snapshot = Some(test_scene_command().0);
    assert!(pending_work_wins_over_expired_animation_deadline(&guard));

    guard.snapshot = None;
    guard.command = Some(RenderCommand::MarkFailed { ack });
    assert!(pending_work_wins_over_expired_animation_deadline(&guard));
}

#[test]
fn native_scene_publication_reports_a_poisoned_worker() {
    let state = rendered_state();
    let poison_state = Arc::clone(&state);
    thread::spawn(move || {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = poison_state.0.lock().unwrap();
            panic!("poison render state");
        }));
    })
    .join()
    .unwrap();

    assert!(idle_worker(state)
        .publish_native_scene(test_scene(), Default::default(), None)
        .is_err());
}

#[test]
fn native_scene_publication_reports_a_pending_shutdown() {
    let state = rendered_state();
    let (ack, _received) = mpsc::channel();
    state.0.lock().unwrap().command = Some(RenderCommand::Shutdown { ack });

    assert_eq!(
        idle_worker(state)
            .publish_native_scene(test_scene(), Default::default(), None)
            .err(),
        Some("render worker is terminating".into())
    );
}

#[test]
#[cfg(not(any(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "hardware-orange-pi-zero-2w"
)))]
fn mark_failed_ack_succeeds_without_an_attached_handoff() {
    let (worker, _seesaw_rx) = spawn_fake_worker();

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
    let path = crate::test_temp_dir::unique_temp_path("octessera-render-failed");
    let handoff = crate::boot_oled_handoff::native_guard_for_test(&path).unwrap();
    fs::remove_file(path.join("status.json")).unwrap();
    fs::create_dir(path.join("status.json")).unwrap();
    let (seesaw_tx, _seesaw_rx) = mpsc::channel();
    let worker = RenderWorker::spawn(HardwareRenderTargets {
        oled: crate::render::test_oled_output::fake_oled_output(),
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
fn ownership_lane_keeps_latest_scene_pending() {
    let state = rendered_state();
    let (ack, _received) = mpsc::channel();
    state.0.lock().unwrap().command = Some(RenderCommand::Ownership {
        stage: OledOwnershipStage::PrepareRelease,
        cancellation: Arc::new(AtomicBool::new(false)),
        ack,
    });

    assert!(idle_worker(Arc::clone(&state))
        .publish_native_scene(test_scene(), Default::default(), None)
        .is_ok());
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
fn initial_scene_ack_is_current_and_cannot_be_reused() {
    let (worker, _seesaw_rx) = spawn_fake_worker();
    assert_eq!(
        worker.mark_first_menu_rendered(),
        Err("OLED handoff is not acknowledged by a successful native write".into())
    );
    assert!(worker
        .publish_native_scene(test_scene(), Default::default(), None)
        .is_err());

    let (revision, pixels) = worker
        .publish_initial_native_scene(test_scene(), Default::default(), None)
        .unwrap();
    assert!(revision > 0);
    assert_eq!(pixels.len(), crate::render::OLED_FRAME_BYTES);
    assert_eq!(worker.mark_first_menu_rendered(), Ok(()));
    assert_eq!(
        worker.publish_initial_native_scene(test_scene(), Default::default(), None),
        Err("render worker rejected a second initial scene".into())
    );
    assert!(worker
        .publish_native_scene(test_scene(), Default::default(), None)
        .is_ok());
    worker.abort().unwrap();
}
