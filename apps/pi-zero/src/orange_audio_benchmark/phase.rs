use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::time::{Duration, Instant};

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
    requested_generation: AtomicU64,
    requested_phase: AtomicU8,
    acknowledged_generation: AtomicU64,
    acknowledged_phase: AtomicU8,
    acknowledged_entry_ns: AtomicU64,
}

impl MeasurementControl {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
            requested_generation: AtomicU64::new(0),
            requested_phase: AtomicU8::new(MeasurementPhase::Disabled as u8),
            acknowledged_generation: AtomicU64::new(0),
            acknowledged_phase: AtomicU8::new(MeasurementPhase::Disabled as u8),
            acknowledged_entry_ns: AtomicU64::new(0),
        }
    }

    pub fn request(&self, phase: MeasurementPhase) -> u64 {
        self.requested_phase.store(phase as u8, Ordering::Relaxed);
        self.requested_generation.fetch_add(1, Ordering::Release) + 1
    }

    pub fn capture_at_callback_entry(&self) -> PhaseCapture {
        let entry_ns = self.now_ns();
        let generation = self.requested_generation.load(Ordering::Acquire);
        let phase = phase_from_u8(self.requested_phase.load(Ordering::Relaxed));
        if self.acknowledged_generation.load(Ordering::Acquire) != generation {
            self.acknowledged_phase
                .store(phase as u8, Ordering::Relaxed);
            self.acknowledged_entry_ns
                .store(entry_ns, Ordering::Relaxed);
            self.acknowledged_generation
                .store(generation, Ordering::Release);
        }
        PhaseCapture {
            generation,
            phase,
            entry_ns,
        }
    }

    pub fn acknowledgement(
        &self,
        generation: u64,
        phase: MeasurementPhase,
    ) -> Option<PhaseCapture> {
        if self.acknowledged_generation.load(Ordering::Acquire) != generation {
            return None;
        }
        let acknowledged_phase = phase_from_u8(self.acknowledged_phase.load(Ordering::Relaxed));
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

fn phase_from_u8(value: u8) -> MeasurementPhase {
    if value == MeasurementPhase::Measuring as u8 {
        MeasurementPhase::Measuring
    } else {
        MeasurementPhase::Disabled
    }
}

#[cfg(test)]
mod tests {
    use super::{same_measuring_generation, MeasurementControl, MeasurementPhase, PhaseCapture};

    #[test]
    fn phase_acknowledgement_is_generation_bound() {
        let control = MeasurementControl::new();
        let measuring = control.request(MeasurementPhase::Measuring);
        assert!(control
            .acknowledgement(measuring, MeasurementPhase::Measuring)
            .is_none());
        let capture = control.capture_at_callback_entry();
        assert_eq!(capture.phase, MeasurementPhase::Measuring);
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
}
