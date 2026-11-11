use super::{request_worker_dispatch, DISPATCH_REPLY_TIMEOUT};
use crate::runtime_worker::WorkerCommand;
use playback_runtime::{
    HostMessage, RunnerMessage, RuntimeErrorDomain, RuntimeOperation, RuntimeStatusState,
};
use std::sync::mpsc;

fn assert_worker_stopped_status(messages: &[RunnerMessage]) {
    let [RunnerMessage::RuntimeStatus { status }] = messages else {
        panic!("expected one runtime status, got {messages:?}");
    };
    assert_eq!(status.state, RuntimeStatusState::Error);
    let error = status.error.as_ref().expect("status error");
    assert_eq!(error.domain, RuntimeErrorDomain::Runtime);
    assert_eq!(error.operation, RuntimeOperation::RuntimeDispatch);
    assert!(error
        .message
        .as_deref()
        .is_some_and(|message| message.contains("restart")));
}

#[test]
fn a_dispatch_to_a_stopped_worker_returns_a_visible_runtime_error() {
    let (worker_tx, worker_rx) = mpsc::channel::<WorkerCommand>();
    drop(worker_rx);

    let messages = request_worker_dispatch(
        &worker_tx,
        HostMessage::MidiRealtimeStop,
        DISPATCH_REPLY_TIMEOUT,
    )
    .expect("a stopped worker is reported as a runtime status");

    assert_worker_stopped_status(&messages);
}

#[test]
fn a_worker_that_dies_before_replying_returns_a_visible_runtime_error() {
    let (worker_tx, worker_rx) = mpsc::channel::<WorkerCommand>();
    let worker = std::thread::spawn(move || {
        let command = worker_rx.recv().expect("dispatch");
        drop(command);
    });

    let messages = request_worker_dispatch(
        &worker_tx,
        HostMessage::MidiRealtimeStop,
        DISPATCH_REPLY_TIMEOUT,
    )
    .expect("a dead worker is reported as a runtime status");

    worker.join().unwrap();
    assert_worker_stopped_status(&messages);
}

#[test]
fn a_worker_reply_passes_through() {
    let (worker_tx, worker_rx) = mpsc::channel::<WorkerCommand>();
    let worker = std::thread::spawn(move || {
        let Ok(WorkerCommand::Dispatch(_, reply)) = worker_rx.recv() else {
            panic!("expected a dispatch");
        };
        reply
            .send(Ok(vec![RunnerMessage::PresentedRuntimeErrorDismissed]))
            .unwrap();
    });

    let messages = request_worker_dispatch(
        &worker_tx,
        HostMessage::MidiRealtimeStop,
        DISPATCH_REPLY_TIMEOUT,
    )
    .unwrap();

    worker.join().unwrap();
    assert_eq!(
        messages,
        vec![RunnerMessage::PresentedRuntimeErrorDismissed]
    );
}
