use super::fx_param::{apply_fx_param, FxParamId, FxParamMutation};
use super::fx_params::FxBusParams;

/// How long a live bus or master FX change takes to land, matching the instrument glides.
const FX_PARAM_GLIDE_MS: f32 = 10.0;

pub(super) fn fx_param_glide_frames(sample_rate: u32) -> u32 {
    (sample_rate as f32 * FX_PARAM_GLIDE_MS / 1000.0)
        .round()
        .max(1.0) as u32
}

/// Levels, gains, feedback, filter centers and Q glide. Rates switch at once because the LFO
/// phase carries on smoothly anyway; attack and release times, Glitch chance and slice length,
/// and the Bitcrusher's bits and rate divider are steps or time constants, not levels.
fn glides(id: FxParamId) -> bool {
    !matches!(
        id,
        FxParamId::RateHz
            | FxParamId::ChancePct
            | FxParamId::SliceMs
            | FxParamId::AttackMs
            | FxParamId::ReleaseMs
            | FxParamId::Bits
            | FxParamId::RateDiv
    )
}

/// Where one FX slot's live parameters are heading. The slot's params hold the values it plays;
/// while frames remain they move one frame at a time towards `target`.
#[derive(Clone, Copy, Debug)]
struct FxParamGlide {
    target: FxBusParams,
    remaining: u32,
}

impl FxParamGlide {
    pub(super) const SETTLED: Self = Self {
        target: FxBusParams::None,
        remaining: 0,
    };

    #[inline]
    pub(super) fn is_active(&self) -> bool {
        self.remaining > 0
    }

    /// Sets `id` on where the slot is heading. A gliding parameter starts a glide of `frames`
    /// from the value the slot plays now; a switching one changes the playing params at once.
    pub(super) fn set(
        &mut self,
        current: &mut FxBusParams,
        id: FxParamId,
        value: f32,
        frames: u32,
    ) -> FxParamMutation {
        let mut target = if self.is_active() {
            self.target
        } else {
            *current
        };
        let mutation = apply_fx_param(&mut target, id, value);
        if mutation != FxParamMutation::Changed {
            return mutation;
        }
        if glides(id) {
            *self = Self {
                target,
                remaining: frames.max(1),
            };
        } else {
            apply_fx_param(current, id, value);
            self.target = target;
        }
        FxParamMutation::Changed
    }

    #[cfg(test)]
    pub(super) fn land(&mut self, current: &mut FxBusParams) {
        if self.is_active() {
            *current = self.target;
            self.remaining = 0;
        }
    }

    /// Moves `current` one frame along; the last frame lands on the target exactly.
    #[inline]
    pub(super) fn advance(&mut self, current: &mut FxBusParams) {
        if self.remaining <= 1 {
            *current = self.target;
            self.remaining = 0;
            return;
        }
        glide_fields(current, &self.target, 1.0 / self.remaining as f32);
        self.remaining -= 1;
    }
}

/// The glides of a bus chain's or the master's FX slots, with one flag a settled chain checks
/// per frame.
#[derive(Clone, Copy, Debug)]
pub(super) struct FxSlotGlides<const N: usize> {
    slots: [FxParamGlide; N],
    active: bool,
}

impl<const N: usize> FxSlotGlides<N> {
    pub(super) const SETTLED: Self = Self {
        slots: [FxParamGlide::SETTLED; N],
        active: false,
    };

    #[inline]
    pub(super) fn is_active(&self) -> bool {
        self.active
    }

    /// Sets a live parameter of `slot`, whose playing params are `params`.
    pub(super) fn set(
        &mut self,
        slot: usize,
        params: &mut FxBusParams,
        id: FxParamId,
        value: f32,
        frames: u32,
    ) -> FxParamMutation {
        let Some(glide) = self.slots.get_mut(slot) else {
            return FxParamMutation::Rejected;
        };
        let mutation = glide.set(params, id, value, frames);
        self.active |= glide.is_active();
        mutation
    }

    /// Moves every running slot glide one frame along. Out of line so a settled chain's frame
    /// loop compiles as it did without glides.
    #[inline(never)]
    pub(super) fn advance(&mut self, params: &mut [FxBusParams]) {
        let mut active = false;
        for (glide, params) in self.slots.iter_mut().zip(params) {
            if glide.is_active() {
                glide.advance(params);
                active |= glide.is_active();
            }
        }
        self.active = active;
    }

    /// Drops `slot`'s running glide when a slot config replaces its params.
    pub(super) fn settle(&mut self, slot: usize) {
        if let Some(glide) = self.slots.get_mut(slot) {
            *glide = FxParamGlide::SETTLED;
        }
        self.active = self.slots.iter().any(FxParamGlide::is_active);
    }

    #[cfg(test)]
    pub(super) fn land(&mut self, params: &mut [FxBusParams]) {
        for (glide, params) in self.slots.iter_mut().zip(params) {
            glide.land(params);
        }
        self.active = false;
    }
}

fn line(current: &mut f32, target: f32, fraction: f32) {
    *current += (target - *current) * fraction;
}

fn pitch(current: &mut f32, target: f32, fraction: f32) {
    *current *= (target / *current).powf(fraction);
}

fn glide_fields(current: &mut FxBusParams, target: &FxBusParams, f: f32) {
    use FxBusParams as P;
    match (current, *target) {
        (P::Tremolo { depth, .. }, P::Tremolo { depth: to, .. })
        | (P::AutoPan { depth, .. }, P::AutoPan { depth: to, .. }) => line(depth, to, f),
        (
            P::Delay {
                feedback,
                mix,
                spread,
                ..
            },
            P::Delay {
                feedback: to_feedback,
                mix: to_mix,
                spread: to_spread,
                ..
            },
        ) => {
            line(feedback, to_feedback, f);
            line(mix, to_mix, f);
            line(spread, to_spread, f);
        }
        (
            P::ModDelay { feedback, mix, .. },
            P::ModDelay {
                feedback: to_feedback,
                mix: to_mix,
                ..
            },
        ) => {
            line(feedback, to_feedback, f);
            line(mix, to_mix, f);
        }
        (
            P::FilterLfo {
                depth,
                center_hz,
                q,
                ..
            },
            P::FilterLfo {
                depth: to_depth,
                center_hz: to_center,
                q: to_q,
                ..
            },
        ) => {
            line(depth, to_depth, f);
            pitch(center_hz, to_center, f);
            line(q, to_q, f);
        }
        (
            P::Reverb { mix, decay, damp },
            P::Reverb {
                mix: to_mix,
                decay: to_decay,
                damp: to_damp,
            },
        ) => {
            line(mix, to_mix, f);
            line(decay, to_decay, f);
            line(damp, to_damp, f);
        }
        (P::Glitch { mix, .. }, P::Glitch { mix: to, .. })
        | (P::Bitcrusher { mix, .. }, P::Bitcrusher { mix: to, .. }) => line(mix, to, f),
        (
            P::Duck {
                threshold, amount, ..
            },
            P::Duck {
                threshold: to_threshold,
                amount: to_amount,
                ..
            },
        ) => {
            line(threshold, to_threshold, f);
            line(amount, to_amount, f);
        }
        (
            P::Saturator { drive, mix },
            P::Saturator {
                drive: to_drive,
                mix: to_mix,
            },
        ) => {
            line(drive, to_drive, f);
            line(mix, to_mix, f);
        }
        (
            P::Distortion { drive, clip, mix },
            P::Distortion {
                drive: to_drive,
                clip: to_clip,
                mix: to_mix,
            },
        ) => {
            line(drive, to_drive, f);
            line(clip, to_clip, f);
            line(mix, to_mix, f);
        }
        (
            P::Compressor {
                threshold_db,
                ratio,
                makeup_db,
                mix,
                ..
            },
            P::Compressor {
                threshold_db: to_threshold,
                ratio: to_ratio,
                makeup_db: to_makeup,
                mix: to_mix,
                ..
            },
        ) => {
            line(threshold_db, to_threshold, f);
            line(ratio, to_ratio, f);
            line(makeup_db, to_makeup, f);
            line(mix, to_mix, f);
        }
        (
            P::Eq {
                low_gain_db,
                mid_gain_db,
                mid_freq_hz,
                mid_q,
                high_gain_db,
                mix,
            },
            P::Eq {
                low_gain_db: to_low,
                mid_gain_db: to_mid,
                mid_freq_hz: to_freq,
                mid_q: to_q,
                high_gain_db: to_high,
                mix: to_mix,
            },
        ) => {
            line(low_gain_db, to_low, f);
            line(mid_gain_db, to_mid, f);
            pitch(mid_freq_hz, to_freq, f);
            line(mid_q, to_q, f);
            line(high_gain_db, to_high, f);
            line(mix, to_mix, f);
        }
        (
            P::Vinyl {
                saturation,
                crackle,
                warp_depth,
                mix,
            },
            P::Vinyl {
                saturation: to_saturation,
                crackle: to_crackle,
                warp_depth: to_warp,
                mix: to_mix,
            },
        ) => {
            line(saturation, to_saturation, f);
            line(crackle, to_crackle, f);
            line(warp_depth, to_warp, f);
            line(mix, to_mix, f);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
