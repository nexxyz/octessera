pub(super) const RING_LEN: usize = 8192;
pub(super) type PluckRing = [f32; RING_LEN];
#[cfg(feature = "routing-tree-benchmark")]
pub(super) type PluckRings = [Option<Box<PluckRing>>; super::types::SYNTH_VOICE_LANE_CAPACITY];

#[derive(Clone, Copy, Debug)]
pub(super) struct PluckState {
    pub(super) write: usize,
    pub(super) emitted: u32,
    pub(super) delay: usize,
    pub(super) fraction: f32,
    pub(super) pick_offset: usize,
    pub(super) loss: f32,
    pub(super) brightness: f32,
    pub(super) previous: f32,
    pub(super) seed: u32,
}

impl PluckState {
    pub(super) fn off() -> Self {
        Self {
            write: 0,
            emitted: 0,
            delay: 3,
            fraction: 0.0,
            pick_offset: 1,
            loss: 0.0,
            brightness: 0.0,
            previous: 0.0,
            seed: 1,
        }
    }

    pub(super) fn note_on(
        freq_hz: f32,
        sample_rate: u32,
        note: u8,
        velocity: u8,
        decay_ms: f32,
        brightness_pct: f32,
        pick_position_pct: f32,
    ) -> Self {
        let delay = (sample_rate as f32 / freq_hz - 0.5).clamp(3.0, (RING_LEN - 2) as f32);
        let integer = delay.floor() as usize;
        let mut state = Self {
            delay: integer,
            fraction: delay - integer as f32,
            pick_offset: ((delay * pick_position_pct.clamp(5.0, 50.0) / 100.0).round() as usize)
                .clamp(1, integer.saturating_sub(1)),
            seed: (note as u32).wrapping_mul(0x9e37_79b9)
                ^ (velocity as u32).wrapping_mul(0x85eb_ca6b)
                ^ 0x6d2b_79f5,
            ..Self::off()
        };
        state.update_coefficients(freq_hz, decay_ms, brightness_pct);
        state
    }

    pub(super) fn update_coefficients(&mut self, freq_hz: f32, decay_ms: f32, brightness_pct: f32) {
        self.loss = (-6.907_755 / (decay_ms.clamp(100.0, 5_000.0) * 0.001 * freq_hz))
            .exp()
            .clamp(0.0, 0.999_95);
        self.brightness = (0.2 + brightness_pct.clamp(0.0, 100.0) * 0.0078).clamp(0.2, 0.98);
    }

    pub(super) fn next(&mut self, ring: &mut PluckRing) -> f32 {
        let output = if (self.emitted as usize) <= self.delay {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 17;
            self.seed ^= self.seed << 5;
            let noise = (self.seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let previous_pick = if (self.emitted as usize) >= self.pick_offset {
                ring[(self.write + RING_LEN - self.pick_offset) % RING_LEN]
            } else {
                0.0
            };
            (noise - previous_pick * 0.65) * 0.5
        } else {
            let newer = ring[(self.write + RING_LEN - self.delay) % RING_LEN];
            let older = ring[(self.write + RING_LEN - self.delay - 1) % RING_LEN];
            let delayed = newer * (1.0 - self.fraction) + older * self.fraction;
            (delayed * self.brightness + self.previous * (1.0 - self.brightness)) * self.loss
        };
        ring[self.write] = output;
        self.write = (self.write + 1) % RING_LEN;
        self.emitted = self.emitted.saturating_add(1);
        self.previous = output;
        output
    }
}
