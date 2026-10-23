//! A voice that takes over a stolen lane starts from the old voice's last output and lets that
//! offset decay away within a few milliseconds, so the steal has no step to click on.

/// A 0.6 ms time constant: a full-scale offset falls below `SILENT` within about 5 ms.
const DECAY_MS: f32 = 0.6;
const SILENT: f32 = 1.0e-4;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StealDeclick {
    residual: f32,
}

impl StealDeclick {
    pub(super) fn from_output(output: f32) -> Self {
        Self { residual: output }
    }

    /// Adds the decaying offset to a freshly rendered lane block and remembers its last sample.
    pub(super) fn finish_block(
        &mut self,
        last_output: &mut f32,
        samples: &mut [f32],
        sample_rate: u32,
    ) {
        if self.residual != 0.0 {
            let decay = decay_per_sample(sample_rate);
            for sample in samples.iter_mut() {
                *sample += self.residual;
                self.residual *= decay;
                if self.residual.abs() < SILENT {
                    self.residual = 0.0;
                    break;
                }
            }
        }
        if let Some(last) = samples.last() {
            *last_output = *last;
        }
    }

    pub(super) fn finish_frame(
        &mut self,
        last_output: &mut f32,
        value: f32,
        sample_rate: u32,
    ) -> f32 {
        let mut value = value;
        if self.residual != 0.0 {
            value += self.residual;
            self.residual *= decay_per_sample(sample_rate);
            if self.residual.abs() < SILENT {
                self.residual = 0.0;
            }
        }
        *last_output = value;
        value
    }
}

fn decay_per_sample(sample_rate: u32) -> f32 {
    (-1000.0 / (DECAY_MS * sample_rate.max(1) as f32)).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decays_the_old_output_away_within_a_few_milliseconds() {
        let mut declick = StealDeclick::from_output(0.5);
        let mut last = 0.0;
        let mut samples = [0.0; 480];
        declick.finish_block(&mut last, &mut samples, 48_000);
        assert_eq!(samples[0], 0.5);
        assert!(samples[1] < samples[0] && samples[1] > 0.48);
        assert!(samples[300..].iter().all(|sample| *sample == 0.0));
        assert_eq!(last, 0.0);
        assert_eq!(declick.residual, 0.0);
    }

    #[test]
    fn frame_and_block_paths_agree() {
        let mut block = StealDeclick::from_output(-0.3);
        let mut frame = block;
        let mut samples = [0.1; 64];
        let (mut block_last, mut frame_last) = (0.0, 0.0);
        block.finish_block(&mut block_last, &mut samples, 44_100);
        for expected in samples {
            assert_eq!(frame.finish_frame(&mut frame_last, 0.1, 44_100), expected);
        }
        assert_eq!(block_last, frame_last);
    }
}
