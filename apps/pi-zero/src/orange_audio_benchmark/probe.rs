use realtime_engine::synth::SynthProfileSnapshot;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

pub struct ProfileProbe {
    requested_generation: AtomicU64,
    published_generation: AtomicU64,
    active_synth_voices: AtomicU64,
    active_sample_voices: AtomicU64,
    active_preview_sample_voices: AtomicU64,
    active_momentary_fx: AtomicU64,
    cumulative_voice_steals: AtomicU64,
    synth_parallel_dispatches: AtomicU64,
    synth_parallel_light_skips: AtomicU64,
    synth_parallel_backoff_skips: AtomicU64,
    synth_parallel_timing_backoffs: AtomicU64,
    synth_parallel_failures: AtomicU64,
    synth_parallel_unhealthy: AtomicBool,
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
            cumulative_voice_steals: AtomicU64::new(0),
            synth_parallel_dispatches: AtomicU64::new(0),
            synth_parallel_light_skips: AtomicU64::new(0),
            synth_parallel_backoff_skips: AtomicU64::new(0),
            synth_parallel_timing_backoffs: AtomicU64::new(0),
            synth_parallel_failures: AtomicU64::new(0),
            synth_parallel_unhealthy: AtomicBool::new(false),
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
        self.cumulative_voice_steals
            .store(snapshot.cumulative_voice_steals, Ordering::Relaxed);
        self.synth_parallel_dispatches
            .store(snapshot.synth_parallel_dispatches, Ordering::Relaxed);
        self.synth_parallel_light_skips
            .store(snapshot.synth_parallel_light_skips, Ordering::Relaxed);
        self.synth_parallel_backoff_skips
            .store(snapshot.synth_parallel_backoff_skips, Ordering::Relaxed);
        self.synth_parallel_timing_backoffs
            .store(snapshot.synth_parallel_timing_backoffs, Ordering::Relaxed);
        self.synth_parallel_failures
            .store(snapshot.synth_parallel_failures, Ordering::Relaxed);
        self.synth_parallel_unhealthy
            .store(snapshot.synth_parallel_unhealthy, Ordering::Relaxed);
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
            synth_parallel_dispatches: self.synth_parallel_dispatches.load(Ordering::Relaxed),
            synth_parallel_light_skips: self.synth_parallel_light_skips.load(Ordering::Relaxed),
            synth_parallel_backoff_skips: self.synth_parallel_backoff_skips.load(Ordering::Relaxed),
            synth_parallel_timing_backoffs: self
                .synth_parallel_timing_backoffs
                .load(Ordering::Relaxed),
            synth_parallel_failures: self.synth_parallel_failures.load(Ordering::Relaxed),
            synth_parallel_unhealthy: self.synth_parallel_unhealthy.load(Ordering::Relaxed),
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
            ..SynthProfileSnapshot::default()
        });
        assert_eq!(probe.poll(generation).unwrap().active_synth_voices, 16);
        assert!(!probe.request_pending());
    }
}
