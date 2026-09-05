use super::*;
use crossbeam_channel::bounded;

fn completion(parity: usize) -> CompletedEnvelope {
    CompletedEnvelope {
        owner: super::super::owner_for_test(parity),
        phase: super::super::super::source_worker_protocol::WorkerPhase::Sources,
        stamp: super::super::super::source_worker_protocol::WorkStamp {
            runtime_generation: 1,
            render_plan_generation: 0,
            quantum_sequence: 1,
            frames: 128,
            base_sample_clock: 0,
        },
        render_ok: true,
        worker_exited: false,
        transport_failed: false,
        dsp_duration_ns: 0,
        active_cost_units: 0,
    }
}

#[test]
fn full_completion_is_preserved_in_worker_exit() {
    let (done_tx, done_rx) = bounded(1);
    done_tx.try_send(completion(0)).expect("queued completion");
    let exit = send_completion(&done_tx, completion(0)).expect("worker exit");
    assert!(exit.unsent_completion.is_some());
    assert!(done_rx.try_recv().is_ok());
}

#[test]
fn disconnected_completion_is_preserved_in_worker_exit() {
    let (done_tx, done_rx) = bounded(1);
    drop(done_rx);
    let exit = send_completion(&done_tx, completion(1)).expect("worker exit");
    assert!(exit.unsent_completion.is_some());
}
