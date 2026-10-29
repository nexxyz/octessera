//! Per-voice sample shaping: amp and filter envelopes, the filter type, its envelope amount and
//! key tracking. A neutral shape leaves the plain block loop in charge; anything else renders
//! through the shaped frame path.

use super::super::runtime_state::{BiquadState, EnvState};
use super::super::types::{EnvConfig, FilterType, SampleShapeConfig};

/// The filter follows its envelope at control rate, like synth voices.
const FILTER_CONTROL_FRAMES: u8 = 16;

#[derive(Clone, Copy, Debug)]
pub(super) struct SampleVoiceShape {
    pub(super) amp_env: EnvState,
    filt_env: EnvState,
    pub(super) filter_kind: FilterType,
    env_amount: f32,
    key_scale: f32,
    filter_countdown: u8,
    pending_release: Option<PendingRelease>,
}

/// A note-off placed inside a block: the frames left until it, counted from the block start.
#[derive(Clone, Copy, Debug)]
struct PendingRelease {
    frames: u32,
    amp_env: EnvConfig,
    filter_env: EnvConfig,
}

impl SampleVoiceShape {
    /// `pitch_semis` is how far the sample is played from its recorded pitch.
    pub(super) fn start(config: &SampleShapeConfig, pitch_semis: f32, sample_rate: u32) -> Self {
        let key_tracking = (config.filter_key_tracking_pct / 100.0).clamp(0.0, 1.0);
        Self {
            amp_env: EnvState::note_on(config.amp_env, sample_rate),
            filt_env: EnvState::note_on(config.filter_env, sample_rate),
            filter_kind: config.filter_type,
            env_amount: (config.filter_env_amount_pct / 100.0).clamp(-1.0, 1.0),
            key_scale: if key_tracking == 0.0 {
                1.0
            } else {
                2.0_f32.powf(pitch_semis * key_tracking / 12.0)
            },
            filter_countdown: 0,
            pending_release: None,
        }
    }

    pub(super) fn off() -> Self {
        Self {
            amp_env: EnvState::off(),
            filt_env: EnvState::off(),
            filter_kind: FilterType::Lowpass,
            env_amount: 0.0,
            key_scale: 1.0,
            filter_countdown: 0,
            pending_release: None,
        }
    }

    /// Releases `frames` frames into the next rendered block; zero releases now.
    pub(super) fn release_after(
        &mut self,
        config: &SampleShapeConfig,
        frames: u32,
        sample_rate: u32,
    ) {
        if frames == 0 {
            self.pending_release = None;
            self.begin_release(config.amp_env, config.filter_env, sample_rate);
            return;
        }
        if self
            .pending_release
            .is_some_and(|pending| pending.frames <= frames)
        {
            return;
        }
        self.pending_release = Some(PendingRelease {
            frames,
            amp_env: config.amp_env,
            filter_env: config.filter_env,
        });
    }

    /// The frame inside a `frames`-long block where a pending release begins; a later release
    /// moves one block closer.
    pub(super) fn release_frame_in_block(&mut self, frames: usize) -> Option<usize> {
        let pending = self.pending_release.as_mut()?;
        if (pending.frames as usize) < frames {
            return Some(pending.frames as usize);
        }
        pending.frames -= frames as u32;
        None
    }

    /// Advances a pending release by one frame, beginning it when its frame arrives.
    pub(super) fn tick_pending_release(&mut self, sample_rate: u32) {
        match self.pending_release.as_mut() {
            Some(pending) if pending.frames > 0 => pending.frames -= 1,
            Some(_) => self.begin_pending_release(sample_rate),
            None => {}
        }
    }

    pub(super) fn begin_pending_release(&mut self, sample_rate: u32) {
        if let Some(pending) = self.pending_release.take() {
            self.begin_release(pending.amp_env, pending.filter_env, sample_rate);
        }
    }

    fn begin_release(&mut self, amp_env: EnvConfig, filter_env: EnvConfig, sample_rate: u32) {
        self.amp_env.begin_release(amp_env, sample_rate);
        self.filt_env.begin_release(filter_env, sample_rate);
    }

    /// True while the voice plays at full level through a fixed filter, so the plain loop applies.
    pub(super) fn is_static(&self) -> bool {
        self.env_amount == 0.0 && self.amp_env.holds_full_level()
    }

    pub(super) fn static_cutoff(&self, cutoff_hz: f32, sample_rate: u32) -> f32 {
        self.cutoff(cutoff_hz, 0.0, sample_rate)
    }

    /// Advances both envelopes one frame: the amp gain, or `None` once the release has ended.
    pub(super) fn next_frame(
        &mut self,
        filt: &mut BiquadState,
        cutoff_hz: f32,
        q: f32,
        sample_rate: u32,
    ) -> Option<f32> {
        let amp = self.amp_env.next();
        let filter_env = self.filt_env.next();
        if self.amp_env.is_off() {
            return None;
        }
        if self.filter_countdown == 0 {
            let cutoff = self.cutoff(cutoff_hz, filter_env, sample_rate);
            filt.prepare(self.filter_kind, cutoff, q, sample_rate);
            self.filter_countdown = FILTER_CONTROL_FRAMES - 1;
        } else {
            self.filter_countdown -= 1;
        }
        Some(amp)
    }

    fn cutoff(&self, cutoff_hz: f32, filter_env: f32, sample_rate: u32) -> f32 {
        let enveloped = cutoff_hz * (1.0 + self.env_amount * filter_env).max(0.0);
        (enveloped * self.key_scale).clamp(20.0, (sample_rate as f32 * 0.49).clamp(20.0, 20_000.0))
    }
}
