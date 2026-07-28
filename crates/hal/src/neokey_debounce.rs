use std::time::{Duration, Instant};

pub(super) const NEOKEY_DEBOUNCE: Duration = Duration::from_millis(24);

#[derive(Clone, Default)]
pub(super) struct NeoKeyDebouncer {
    stable: [bool; 4],
    candidate: [bool; 4],
    candidate_since: [Option<Instant>; 4],
}

impl NeoKeyDebouncer {
    pub(super) fn update(&mut self, sampled: [bool; 4], now: Instant) -> [bool; 4] {
        for (index, pressed) in sampled.into_iter().enumerate() {
            if pressed == self.stable[index] {
                self.candidate[index] = pressed;
                self.candidate_since[index] = None;
                continue;
            }
            if self.candidate[index] != pressed {
                self.candidate[index] = pressed;
                self.candidate_since[index] = Some(now);
                continue;
            }
            let Some(started) = self.candidate_since[index] else {
                self.candidate_since[index] = Some(now);
                continue;
            };
            if now.duration_since(started) >= NEOKEY_DEBOUNCE {
                self.stable[index] = pressed;
                self.candidate_since[index] = None;
            }
        }
        self.stable
    }
}
