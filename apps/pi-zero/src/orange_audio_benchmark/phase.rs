use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const PHASE_MASK: u64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MeasurementPhase {
    Disabled = 0,
    Measuring = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PhaseCapture {
    pub generation: u64,
    pub phase: MeasurementPhase,
    pub entry_ns: u64,
}

pub fn same_measuring_generation(previous: PhaseCapture, current: PhaseCapture) -> bool {
    previous.phase == MeasurementPhase::Measuring
        && current.phase == MeasurementPhase::Measuring
        && previous.generation == current.generation
}

pub struct MeasurementControl {
    origin: Instant,
    requested_state: AtomicU64,
    acknowledged_state: AtomicU64,
    acknowledged_entry_ns: AtomicU64,
}

impl MeasurementControl {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            requested_state: AtomicU64::new(pack_state(0, MeasurementPhase::Disabled)),
            acknowledged_state: AtomicU64::new(pack_state(0, MeasurementPhase::Disabled)),
            acknowledged_entry_ns: AtomicU64::new(0),
        }
    }

    pub fn request(&self, phase: MeasurementPhase) -> u64 {
        let mut current = self.requested_state.load(Ordering::Acquire);
        loop {
            let (generation, _) = unpack_state(current);
            let next_generation = generation
                .checked_add(1)
                .filter(|generation| *generation <= u64::MAX >> 1)
                .expect("measurement phase generation exhausted");
            let next = pack_state(next_generation, phase);
            match self.requested_state.compare_exchange_weak(
                current,
                next,
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => return next_generation,
                Err(observed) => current = observed,
            }
        }
    }

    pub fn capture_at_callback_entry(&self) -> PhaseCapture {
        let entry_ns = self.now_ns();
        let (generation, phase) = unpack_state(self.requested_state.load(Ordering::Acquire));
        PhaseCapture {
            generation,
            phase,
            entry_ns,
        }
    }

    pub fn boundary_pending(&self, generation: u64) -> bool {
        let (acknowledged_generation, _) =
            unpack_state(self.acknowledged_state.load(Ordering::Acquire));
        acknowledged_generation != generation
    }

    pub fn acknowledge(&self, capture: PhaseCapture) {
        let capture_state = pack_state(capture.generation, capture.phase);
        if self.requested_state.load(Ordering::Acquire) != capture_state {
            return;
        }
        self.acknowledged_entry_ns
            .store(capture.entry_ns, Ordering::Relaxed);
        self.acknowledged_state
            .store(capture_state, Ordering::Release);
    }

    pub fn acknowledgement(
        &self,
        generation: u64,
        phase: MeasurementPhase,
    ) -> Option<PhaseCapture> {
        let (acknowledged_generation, acknowledged_phase) =
            unpack_state(self.acknowledged_state.load(Ordering::Acquire));
        if acknowledged_generation != generation {
            return None;
        }
        if acknowledged_phase != phase {
            return None;
        }
        Some(PhaseCapture {
            generation,
            phase: acknowledged_phase,
            entry_ns: self.acknowledged_entry_ns.load(Ordering::Relaxed),
        })
    }

    pub fn wait_for_ack(
        &self,
        generation: u64,
        phase: MeasurementPhase,
        timeout: Duration,
    ) -> Result<PhaseCapture, String> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(capture) = self.acknowledgement(generation, phase) {
                return Ok(capture);
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(format!("phase acknowledgement timed out: {phase:?}"));
            };
            std::thread::sleep(remaining.min(Duration::from_millis(5)));
        }
    }

    pub fn now_ns(&self) -> u64 {
        self.origin.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
}

fn pack_state(generation: u64, phase: MeasurementPhase) -> u64 {
    debug_assert!(generation <= u64::MAX >> 1);
    (generation << 1) | u64::from(phase as u8)
}

fn unpack_state(state: u64) -> (u64, MeasurementPhase) {
    let phase = if state & PHASE_MASK == MeasurementPhase::Measuring as u64 {
        MeasurementPhase::Measuring
    } else {
        MeasurementPhase::Disabled
    };
    (state >> 1, phase)
}

#[cfg(test)]
mod tests {
    use super::{same_measuring_generation, MeasurementControl, MeasurementPhase, PhaseCapture};
    use std::sync::{Arc, Barrier};
    use std::thread;

    #[test]
    fn phase_acknowledgement_is_generation_bound() {
        let control = MeasurementControl::new();
        let measuring = control.request(MeasurementPhase::Measuring);
        assert!(control
            .acknowledgement(measuring, MeasurementPhase::Measuring)
            .is_none());
        let capture = control.capture_at_callback_entry();
        assert_eq!(capture.phase, MeasurementPhase::Measuring);
        assert!(control.boundary_pending(capture.generation));
        control.acknowledge(capture);
        assert_eq!(
            control.acknowledgement(measuring, MeasurementPhase::Measuring),
            Some(capture)
        );
        let disabled = control.request(MeasurementPhase::Disabled);
        assert!(control
            .acknowledgement(disabled, MeasurementPhase::Disabled)
            .is_none());
        let disabled_capture = control.capture_at_callback_entry();
        assert_eq!(disabled_capture.phase, MeasurementPhase::Disabled);
        control.acknowledge(disabled_capture);
        assert!(control
            .acknowledgement(measuring, MeasurementPhase::Measuring)
            .is_none());
    }

    #[test]
    fn transition_spacing_requires_one_measuring_generation() {
        let previous = PhaseCapture {
            generation: 1,
            phase: MeasurementPhase::Disabled,
            entry_ns: 1,
        };
        let current = PhaseCapture {
            generation: 2,
            phase: MeasurementPhase::Measuring,
            entry_ns: 2,
        };
        assert!(!same_measuring_generation(previous, current));
        assert!(same_measuring_generation(
            PhaseCapture {
                generation: 2,
                phase: MeasurementPhase::Measuring,
                entry_ns: 3,
            },
            current
        ));
    }

    #[test]
    fn concurrent_transitions_never_capture_a_mismatched_phase_and_generation() {
        const TRANSITIONS: u64 = 100_000;
        let control = Arc::new(MeasurementControl::new());
        let barrier = Arc::new(Barrier::new(2));
        let writer_control = Arc::clone(&control);
        let writer_barrier = Arc::clone(&barrier);
        let writer = thread::spawn(move || {
            writer_barrier.wait();
            for index in 0..TRANSITIONS {
                let phase = if index.is_multiple_of(2) {
                    MeasurementPhase::Measuring
                } else {
                    MeasurementPhase::Disabled
                };
                let generation = writer_control.request(phase);
                assert_eq!(
                    generation % 2,
                    (phase == MeasurementPhase::Measuring) as u64
                );
            }
        });
        let reader_barrier = Arc::clone(&barrier);
        let reader = thread::spawn(move || {
            reader_barrier.wait();
            for _ in 0..TRANSITIONS {
                let capture = control.capture_at_callback_entry();
                assert_eq!(
                    capture.generation % 2,
                    (capture.phase == MeasurementPhase::Measuring) as u64
                );
            }
        });
        writer.join().unwrap();
        reader.join().unwrap();
    }

    #[test]
    fn concurrent_acknowledgements_keep_phase_and_generation_coherent() {
        const TRANSITIONS: u64 = 100_000;
        let control = Arc::new(MeasurementControl::new());
        let barrier = Arc::new(Barrier::new(2));
        let writer_control = Arc::clone(&control);
        let writer_barrier = Arc::clone(&barrier);
        let writer = thread::spawn(move || {
            writer_barrier.wait();
            for index in 0..TRANSITIONS {
                let phase = if index.is_multiple_of(2) {
                    MeasurementPhase::Measuring
                } else {
                    MeasurementPhase::Disabled
                };
                let generation = writer_control.request(phase);
                let capture = writer_control.capture_at_callback_entry();
                assert_eq!(capture.generation, generation);
                writer_control.acknowledge(capture);
            }
        });
        let reader_barrier = Arc::clone(&barrier);
        let reader = thread::spawn(move || {
            reader_barrier.wait();
            for _ in 0..TRANSITIONS {
                if let Some(capture) = control.acknowledgement(
                    control.capture_at_callback_entry().generation,
                    MeasurementPhase::Measuring,
                ) {
                    assert_eq!(
                        capture.generation % 2,
                        (capture.phase == MeasurementPhase::Measuring) as u64
                    );
                }
            }
        });
        writer.join().unwrap();
        reader.join().unwrap();
    }
}
