use realtime_engine::synth::SynthProfileSnapshot;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct ProfileProbe {
    requested_generation: AtomicU64,
    published_generation: AtomicU64,
    active_synth_voices: AtomicU64,
    active_sample_voices: AtomicU64,
    active_preview_sample_voices: AtomicU64,
    active_momentary_fx: AtomicU64,
    active_bus_fx_slots: AtomicU64,
    active_global_fx_slots: AtomicU64,
    cumulative_voice_steals: AtomicU64,
    cumulative_voice_admission_drops: AtomicU64,
}

impl ProfileProbe {
    pub fn new() -> Self {
        Self {
            requested_generation: AtomicU64::new(0),
            published_generation: AtomicU64::new(0),
            active_synth_voices: AtomicU64::new(0),
            active_sample_voices: AtomicU64::new(0),
            active_preview_sample_voices: AtomicU64::new(0),
            active_momentary_fx: AtomicU64::new(0),
            active_bus_fx_slots: AtomicU64::new(0),
            active_global_fx_slots: AtomicU64::new(0),
            cumulative_voice_steals: AtomicU64::new(0),
            cumulative_voice_admission_drops: AtomicU64::new(0),
        }
    }

    pub fn request(&self) -> u64 {
        self.requested_generation.fetch_add(1, Ordering::AcqRel) + 1
    }

    pub fn request_pending(&self) -> bool {
        self.requested_generation.load(Ordering::Acquire)
            != self.published_generation.load(Ordering::Acquire)
    }

    pub fn publish(&self, snapshot: SynthProfileSnapshot) {
        let generation = self.requested_generation.load(Ordering::Acquire);
        if generation == 0 || generation == self.published_generation.load(Ordering::Acquire) {
            return;
        }
        self.active_synth_voices
            .store(snapshot.active_synth_voices as u64, Ordering::Relaxed);
        self.active_sample_voices
            .store(snapshot.active_sample_voices as u64, Ordering::Relaxed);
        self.active_preview_sample_voices.store(
            snapshot.active_preview_sample_voices as u64,
            Ordering::Relaxed,
        );
        self.active_momentary_fx
            .store(snapshot.active_momentary_fx as u64, Ordering::Relaxed);
        self.active_bus_fx_slots
            .store(snapshot.active_bus_fx_slots as u64, Ordering::Relaxed);
        self.active_global_fx_slots
            .store(snapshot.active_global_fx_slots as u64, Ordering::Relaxed);
        self.cumulative_voice_steals
            .store(snapshot.cumulative_voice_steals, Ordering::Relaxed);
        self.cumulative_voice_admission_drops
            .store(snapshot.cumulative_voice_admission_drops, Ordering::Relaxed);
        self.published_generation
            .store(generation, Ordering::Release);
    }

    pub fn poll(&self, generation: u64) -> Option<SynthProfileSnapshot> {
        if self.published_generation.load(Ordering::Acquire) != generation {
            return None;
        }
        Some(SynthProfileSnapshot {
            active_synth_voices: self.active_synth_voices.load(Ordering::Relaxed) as usize,
            active_sample_voices: self.active_sample_voices.load(Ordering::Relaxed) as usize,
            active_preview_sample_voices: self.active_preview_sample_voices.load(Ordering::Relaxed)
                as usize,
            active_momentary_fx: self.active_momentary_fx.load(Ordering::Relaxed) as usize,
            cumulative_voice_steals: self.cumulative_voice_steals.load(Ordering::Relaxed),
            cumulative_voice_admission_drops: self
                .cumulative_voice_admission_drops
                .load(Ordering::Relaxed),
            active_bus_fx_slots: self.active_bus_fx_slots.load(Ordering::Relaxed) as usize,
            active_global_fx_slots: self.active_global_fx_slots.load(Ordering::Relaxed) as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileProbe;
    use realtime_engine::synth::SynthProfileSnapshot;

    #[test]
    fn profile_probe_publishes_only_requested_generations() {
        let probe = ProfileProbe::new();
        assert!(!probe.request_pending());
        let generation = probe.request();
        assert!(probe.request_pending());
        assert!(probe.poll(generation).is_none());
        probe.publish(SynthProfileSnapshot {
            active_synth_voices: 16,
            cumulative_voice_admission_drops: 3,
            ..SynthProfileSnapshot::default()
        });
        let snapshot = probe.poll(generation).unwrap();
        assert_eq!(snapshot.active_synth_voices, 16);
        assert_eq!(snapshot.cumulative_voice_admission_drops, 3);
        assert!(!probe.request_pending());
    }
}
