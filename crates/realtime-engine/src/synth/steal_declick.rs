//! Stolen voices never click. A voice the steal leaves in its lane fades to silence over one
//! render quantum and then frees the lane; a voice that has to take over a busy lane in the same
//! slot starts from the old voice's last output and lets that offset decay within a few ms.

use super::types::DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES;

/// A 0.6 ms time constant: a full-scale offset falls below `SILENT` within about 5 ms.
const DECAY_MS: f32 = 0.6;
const SILENT: f32 = 1.0e-4;
/// One render quantum, 2.9 ms at 44.1 kHz: a stolen voice holds its lane for one or two blocks.
pub(super) const STEAL_FADE_FRAMES: u16 = DEFAULT_AUDIO_RENDER_QUANTUM_FRAMES as u16;
const FADE_STEP: f32 = 1.0 / STEAL_FADE_FRAMES as f32;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct StealDeclick {
    residual: f32,
    /// Frames left in a steal fade-out; zero while the voice is not being stolen.
    fade_left: u16,
}

impl StealDeclick {
    pub(super) fn from_output(output: f32) -> Self {
        Self {
            residual: output,
            fade_left: 0,
        }
    }

    pub(super) fn begin_fade(&mut self) {
        if self.fade_left == 0 {
            self.fade_left = STEAL_FADE_FRAMES;
        }
    }

    pub(super) fn is_fading(&self) -> bool {
        self.fade_left != 0
    }

    /// Adds the decaying offset to a freshly rendered lane block, applies a steal fade and
    /// remembers the last sample. Returns the frame count the fade ended the voice at.
    pub(super) fn finish_block(
        &mut self,
        last_output: &mut f32,
        samples: &mut [f32],
        sample_rate: u32,
    ) -> Option<usize> {
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
        let mut ended = None;
        if self.fade_left != 0 {
            for (index, sample) in samples.iter_mut().enumerate() {
                self.fade_left -= 1;
                if self.fade_left == 0 {
                    ended = Some(index);
                    break;
                }
                *sample *= f32::from(self.fade_left) * FADE_STEP;
            }
        }
        if let Some(last) = samples[..ended.unwrap_or(samples.len())].last() {
            *last_output = *last;
        }
        ended
    }

    /// The frame path's `finish_block`: `None` once a steal fade has ended the voice.
    pub(super) fn finish_frame(
        &mut self,
        last_output: &mut f32,
        value: f32,
        sample_rate: u32,
    ) -> Option<f32> {
        let mut value = value;
        if self.residual != 0.0 {
            value += self.residual;
            self.residual *= decay_per_sample(sample_rate);
            if self.residual.abs() < SILENT {
                self.residual = 0.0;
            }
        }
        if self.fade_left != 0 {
            self.fade_left -= 1;
            if self.fade_left == 0 {
                return None;
            }
            value *= f32::from(self.fade_left) * FADE_STEP;
        }
        *last_output = value;
        Some(value)
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
        assert_eq!(declick.finish_block(&mut last, &mut samples, 48_000), None);
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
            assert_eq!(
                frame.finish_frame(&mut frame_last, 0.1, 44_100),
                Some(expected)
            );
        }
        assert_eq!(block_last, frame_last);
    }

    #[test]
    fn a_steal_fade_ramps_to_silence_and_ends_the_voice_after_one_quantum() {
        let frames = usize::from(STEAL_FADE_FRAMES);
        let mut block = StealDeclick::default();
        block.begin_fade();
        let mut frame = block;
        let mut samples = vec![0.8; frames + 16];
        let mut last = 0.0;
        assert_eq!(
            block.finish_block(&mut last, &mut samples, 48_000),
            Some(frames - 1)
        );
        let mut previous = 0.8_f32;
        for sample in &samples[..frames - 1] {
            assert!((previous - sample).abs() <= 0.8 * FADE_STEP + 1.0e-6);
            previous = *sample;
        }
        assert!(previous <= 0.8 * FADE_STEP + 1.0e-6);
        assert_eq!(last, previous);
        assert!(!block.is_fading());
        for expected in &samples[..frames - 1] {
            assert_eq!(frame.finish_frame(&mut last, 0.8, 48_000), Some(*expected));
        }
        assert_eq!(frame.finish_frame(&mut last, 0.8, 48_000), None);
    }

    #[test]
    fn a_steal_fade_carries_across_blocks_and_does_not_restart() {
        let mut declick = StealDeclick::default();
        declick.begin_fade();
        let mut last = 0.0;
        let mut first = [1.0; 64];
        assert_eq!(declick.finish_block(&mut last, &mut first, 44_100), None);
        declick.begin_fade();
        let mut second = [1.0; 64];
        assert_eq!(
            declick.finish_block(&mut last, &mut second, 44_100),
            Some(63)
        );
        assert!((first[63] - second[0] - FADE_STEP).abs() < 1.0e-6);
    }
}
