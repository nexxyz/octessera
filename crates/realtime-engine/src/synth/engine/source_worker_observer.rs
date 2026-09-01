use super::source_worker_owner::OwnerEnvelope;
use super::source_worker_protocol::SourceWorkerShutdown;
use crossbeam_channel::Sender;
use std::cell::RefCell;
use std::thread;

pub type SourceWorkerOwnerIdentity = (usize, usize, usize, usize, usize, Option<usize>);

thread_local! {
    static SHUTDOWN_PROBE: RefCell<Option<Sender<(SourceWorkerShutdown, thread::ThreadId)>>> =
        const { RefCell::new(None) };
}

pub struct SourceWorkerShutdownProbeGuard;

pub fn install_source_worker_shutdown_probe_for_test(
    sender: Sender<(SourceWorkerShutdown, thread::ThreadId)>,
) -> SourceWorkerShutdownProbeGuard {
    SHUTDOWN_PROBE.with(|probe| probe.replace(Some(sender)));
    SourceWorkerShutdownProbeGuard
}

impl Drop for SourceWorkerShutdownProbeGuard {
    fn drop(&mut self) {
        SHUTDOWN_PROBE.with(|probe| probe.replace(None));
    }
}

pub(super) fn notify_shutdown_probe(shutdown: &SourceWorkerShutdown) {
    SHUTDOWN_PROBE.with(|probe| {
        if let Some(sender) = probe.borrow().as_ref() {
            let _ = sender.send((*shutdown, thread::current().id()));
        }
    });
}

pub(super) fn owner_identity(owner: &OwnerEnvelope) -> SourceWorkerOwnerIdentity {
    (
        owner.parity,
        (&*owner.partitions.synth) as *const _ as usize,
        (&*owner.partitions.sample) as *const _ as usize,
        owner.scratch.synth.samples[0].as_ptr() as usize,
        owner.scratch.sample.samples[0].as_ptr() as usize,
        owner
            .partitions
            .sample
            .active_sample_buffer_address_for_test(),
    )
}
