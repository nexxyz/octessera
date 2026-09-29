pub(super) fn fade(value: f32) -> f32 {
    let t = ((value - 0.45) / 0.04).clamp(0.0, 1.0);
    1.0 - t * t * (3.0 - 2.0 * t)
}

pub(super) fn sample(
    carrier_phase: f32,
    modulator_phase: f32,
    modulator: f32,
    env: f32,
    voice: &super::Voice,
) -> f32 {
    let second = if voice.fm_index_second > 0.0 {
        voice.fm_index_second * env * (2.0 * modulator_phase).sin()
    } else {
        0.0
    };
    let carrier = (carrier_phase + (voice.fm_index_fundamental * env) * modulator + second).sin();
    if voice.fm_direct_mix == 0.0 {
        carrier
    } else {
        (carrier + voice.fm_direct_mix * modulator) * voice.fm_normalization
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fade_has_flat_center_and_smooth_boundaries() {
        assert_eq!(fade(0.45), 1.0);
        assert_eq!(fade(0.49), 0.0);
        assert!(fade(0.47) > 0.0 && fade(0.47) < 1.0);
    }

    #[test]
    fn guarded_fm_samples_remain_bounded_and_continuous_at_fade_edges() {
        for sample_rate in [44_100.0_f32, 48_000.0] {
            let carrier = std::f32::consts::TAU * 440.0 / sample_rate;
            let phase = std::f32::consts::TAU * 0.23;
            let output = |raw_modulator: f32, upper: f32| {
                let shape = 0.5;
                let index = 1.5;
                let second = shape * fade(upper);
                let direct = 0.6 * fade(raw_modulator);
                let mut voice = super::super::Voice::off();
                voice.fm_index_fundamental = index * (1.0 - second);
                voice.fm_index_second = index * second;
                voice.fm_direct_mix = direct;
                voice.fm_normalization = 1.0 / (1.0 + direct);
                sample(carrier, phase, phase.sin(), 1.0, &voice)
            };
            for raw_modulator in [0.45, 0.49, 0.5] {
                let rendered = output(raw_modulator, 0.3);
                assert!(rendered.is_finite() && rendered.abs() <= 1.0);
            }
            for edge in [0.45_f32, 0.49] {
                let at = output(0.1, edge);
                let before_raw = output(edge - 0.0001, 0.3);
                let after_raw = output(edge + 0.0001, 0.3);
                let before_upper = output(0.1, edge - 0.0001);
                let after_upper = output(0.1, edge + 0.0001);
                assert!((before_raw - output(edge, 0.3)).abs() < 0.001);
                assert!((after_raw - output(edge, 0.3)).abs() < 0.001);
                assert!((before_upper - at).abs() < 0.001);
                assert!((after_upper - at).abs() < 0.001);
            }
        }
    }
}
