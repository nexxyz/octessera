//! Table sine for oscillators, LFOs, panning and crossfades: 4096 steps with linear
//! interpolation, at most about 6e-7 (-124 dB) from `f32::sin` and roughly twice as fast.
//! Filter and EQ coefficient design keeps `f32::sin`/`cos`: near DC it needs more relative
//! precision than the table gives.

use std::f32::consts::TAU;

include!(concat!(env!("OUT_DIR"), "/sine_table.generated.rs"));

/// Sine of an angle given in turns (1.0 = one full cycle); any finite value works.
#[inline]
pub fn sin_turns(turns: f32) -> f32 {
    let position = (turns - turns.floor()) * SINE_TABLE_STEPS as f32;
    let index = (position as usize).min(SINE_TABLE_STEPS - 1);
    let fraction = position - index as f32;
    let start = SINE_TABLE[index];
    start + (SINE_TABLE[index + 1] - start) * fraction
}

#[inline(always)]
pub fn cos_turns(turns: f32) -> f32 {
    sin_turns(turns + 0.25)
}

#[inline(always)]
pub fn sin_radians(radians: f32) -> f32 {
    sin_turns(radians * (1.0 / TAU))
}

#[inline(always)]
pub fn cos_radians(radians: f32) -> f32 {
    cos_turns(radians * (1.0 / TAU))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_sine_tracks_the_library_sine_for_any_angle() {
        let mut worst = 0.0_f32;
        for step in -200_000..200_000 {
            let turns = step as f32 / 37_123.0;
            let angle = f64::from(turns) * std::f64::consts::TAU;
            worst = worst.max((f64::from(sin_turns(turns)) - angle.sin()).abs() as f32);
            worst = worst.max((f64::from(cos_turns(turns)) - angle.cos()).abs() as f32);
        }
        assert!(worst < 2.0e-6, "{worst}");
        assert_eq!(sin_turns(-1.0e-9), sin_turns(1.0 - 1.0e-9));
        assert!((sin_radians(std::f32::consts::FRAC_PI_2) - 1.0).abs() < 1.0e-6);
    }
}
