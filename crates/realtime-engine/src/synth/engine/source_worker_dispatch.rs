use super::super::source_lane_renderer::SampleSourceContext;
use super::super::source_worker_health::SourceWorkerHealth;
use super::super::source_worker_lease::OwnerLease;
use super::super::source_worker_protocol::{WorkStamp, WorkerCommand};
use super::super::SynthEngine;
use super::SourceWorkerRuntime;
use crossbeam_channel::TrySendError;
#[cfg(feature = "source-worker-benchmark-timing")]
use std::time::Instant;

impl SourceWorkerRuntime {
    pub(super) fn dispatch(&mut self, engine: &mut SynthEngine) -> bool {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        let mut stamp = self.expected_stamp.unwrap_or(WorkStamp {
            runtime_generation: self.runtime_generation,
            render_plan_generation: engine.render_plan.generation,
            quantum_sequence: sequence,
            frames: 0,
            base_sample_clock: engine.sample_clock,
        });
        stamp.quantum_sequence = sequence;
        self.expected_stamp = Some(stamp);
        #[cfg(feature = "source-worker-benchmark-timing")]
        let dispatch_started_at = self.timing_probe.as_ref().map(|_| Instant::now());
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let Some(probe) = self.timing_probe.as_ref() {
            probe.begin_sequence(sequence, self.rendezvous_deadline(stamp.frames));
        }
        let Some(mut first) = self.lease_home(0) else {
            self.latch_dispatch_failure(0b11);
            return false;
        };
        let Some(second) = self.lease_home(1) else {
            first.return_fault();
            self.latch_dispatch_failure(0b11);
            return false;
        };
        #[cfg(feature = "source-worker-benchmark-timing")]
        {
            self.dispatch_started_at = dispatch_started_at;
        }
        let first_sent = self.send_work(
            engine,
            stamp,
            first,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
        );
        let second_sent = self.send_work(
            engine,
            stamp,
            second,
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
        );
        #[cfg(feature = "source-worker-benchmark-timing")]
        if let Some(probe) = self.timing_probe.as_ref() {
            probe.record_dispatch(sequence, self.in_flight_mask);
        }
        first_sent && second_sent && self.health.status() == SourceWorkerHealth::Healthy
    }

    fn send_work(
        &mut self,
        engine: &SynthEngine,
        stamp: WorkStamp,
        mut lease: OwnerLease,
        #[cfg(feature = "source-worker-benchmark-timing")] dispatch_started_at: Option<Instant>,
    ) -> bool {
        let Some(owner) = lease.take_owner() else {
            self.latch_dispatch_failure(1 << lease.parity);
            return false;
        };
        let work = WorkerCommand::RenderSources {
            owner,
            stamp,
            synth_context: engine.synth_source_context(),
            sample_context: SampleSourceContext {
                sample_rate: engine.sample_rate,
            },
            #[cfg(feature = "source-worker-benchmark-timing")]
            dispatch_started_at,
            #[cfg(feature = "source-worker-benchmark-timing")]
            timing_probe: self.timing_probe.clone(),
        };
        let parity = lease.parity;
        let Some(work_tx) = self.work_txs.as_ref().map(|work_txs| &work_txs[parity]) else {
            let WorkerCommand::RenderSources { owner, .. } = work else {
                unreachable!("source dispatch only creates source commands");
            };
            lease.restore_owner(owner);
            lease.return_home();
            self.latch_dispatch_failure(1 << parity);
            return false;
        };
        match work_tx.try_send(work) {
            Ok(()) => {
                self.in_flight_mask |= 1 << parity;
                true
            }
            Err(
                TrySendError::Full(WorkerCommand::RenderSources { owner, .. })
                | TrySendError::Disconnected(WorkerCommand::RenderSources { owner, .. }),
            ) => {
                lease.restore_owner(owner);
                lease.return_home();
                self.latch_dispatch_failure(1 << parity);
                false
            }
            Err(
                TrySendError::Full(WorkerCommand::RenderBuses { .. })
                | TrySendError::Disconnected(WorkerCommand::RenderBuses { .. }),
            ) => {
                self.latch_dispatch_failure(1 << parity);
                false
            }
        }
    }
}
