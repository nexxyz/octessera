use super::super::fm_render;
use super::super::*;
use super::{source_sample, VoiceSource};
use std::f64::consts::TAU;

const UPSAMPLE: usize = 8;
const TAP_COUNT: usize = 257;
const FRAME_COUNT: usize = 2048;
const FIRST_FRAME: usize = 64;
const LOW_BAND: f64 = 0.35;
const REFERENCE_LIMIT: f64 = 0.0025;
const REVIEW_LIMIT: f64 = 0.01;

#[test]
fn calibrated_low_band_reference_reports_matched_fm_residuals() {
    let taps = windowed_sinc_fir();
    let (uncertainty, passband_error, foldback_error, injection_error) = calibrate_reference(&taps);
    eprintln!(
        "FM lower-band calibration uncertainty={uncertainty:.6} passband={passband_error:.6} foldback={foldback_error:.6} injection={injection_error:.6}"
    );
    assert!(
        uncertainty <= REFERENCE_LIMIT,
        "lower-band reference uncertainty {uncertainty} exceeds {REFERENCE_LIMIT}"
    );
    for sample_rate in [44_100_u32, 48_000] {
        for (label, note, ratio, fine, index, velocity_pct, velocity, shape, mix) in [
            ("neutral", 84, FmRatio::Eight, 0, 50, 0, 127, 0, 0),
            ("Mraw-low", 108, FmRatio::Five, -100, 50, 0, 127, 0, 0),
            ("Mraw-high", 108, FmRatio::Five, 100, 50, 0, 127, 0, 100),
            ("U-low", 108, FmRatio::One, 0, 41, 35, 83, 100, 0),
            ("U-mix", 108, FmRatio::One, 0, 41, 35, 83, 100, 100),
        ] {
            let config = FmConfig {
                ratio,
                ratio_fine_cents: fine,
                index,
                velocity_to_index_pct: velocity_pct,
                mod_shape_pct: shape,
                mod_mix_pct: mix,
                ..FmConfig::default()
            };
            let mut engine = make_engine(config, sample_rate);
            engine.note_on(0, note, velocity, 10_000);
            let render_config = engine.synth_render_configs[0];
            let initial = *engine.synth_voice_pool.lane(0).unwrap();
            let VoiceSource::Fm {
                effective_ratio,
                index,
                velocity_to_index,
                index_env,
                ..
            } = render_config.source
            else {
                unreachable!();
            };
            let candidate_voice = fixed_env(initial, 0.73);
            let baseline_index = (index
                * ((1.0 - velocity_to_index) + velocity_to_index * candidate_voice.velocity_norm))
                .min(candidate_voice.fm_index_limit);
            let mut baseline_voice = fixed_env(initial, 0.73);
            baseline_voice.fm_index_fundamental = baseline_index;
            baseline_voice.fm_index_second = 0.0;
            baseline_voice.fm_direct_mix = 0.0;
            baseline_voice.fm_normalization = 1.0;
            baseline_voice.fm_neutral = true;
            let mut baseline_config = render_config;
            baseline_config.source = VoiceSource::Fm {
                base_ratio: effective_ratio,
                effective_ratio,
                ratio_fine_cents: 0,
                index: baseline_index,
                velocity_to_index: 0.0,
                mod_shape: 0.0,
                mod_mix: 0.0,
                index_env,
            };

            let carrier_hz = f64::from(initial.freq_hz);
            let modulator_hz = carrier_hz * f64::from(effective_ratio);
            let candidate_target =
                render_target(render_config, candidate_voice, velocity, FRAME_COUNT);
            let baseline_target =
                render_target(baseline_config, baseline_voice, velocity, FRAME_COUNT);
            let candidate_high = render_high_rate(
                candidate_voice,
                carrier_hz,
                modulator_hz,
                FRAME_COUNT,
                sample_rate,
            );
            let baseline_high = render_high_rate(
                baseline_voice,
                carrier_hz,
                modulator_hz,
                FRAME_COUNT,
                sample_rate,
            );
            let candidate_reference = filter_decimate(&candidate_high, &taps, sample_rate);
            let baseline_reference = filter_decimate(&baseline_high, &taps, sample_rate);
            let candidate_residual = subtract(&candidate_target, &candidate_reference);
            let baseline_residual = subtract(&baseline_target, &baseline_reference);
            let changed_residual = subtract(&candidate_residual, &baseline_residual);
            let candidate_residual_energy = low_band_energy(&candidate_residual[FIRST_FRAME..]);
            let baseline_residual_energy = low_band_energy(&baseline_residual[FIRST_FRAME..]);
            let changed_residual_energy = low_band_energy(&changed_residual[FIRST_FRAME..]);
            let denominator = rms(&baseline_target[FIRST_FRAME..]);
            let r_base = baseline_residual_energy.sqrt() / denominator;
            let r_case = candidate_residual_energy.sqrt() / denominator;
            let delta_power =
                (candidate_residual_energy - baseline_residual_energy) / denominator.powi(2);
            let r_changed = changed_residual_energy.sqrt() / denominator;
            let voice = engine.synth_voice_pool.lane(0).unwrap();
            let upper = voice.osc1_inc
                + 2.0 * voice.fm_modulator_raw_inc
                + baseline_index * voice.fm_modulator_raw_inc * (1.0 + shape as f32 * 0.01 * 0.5);
            eprintln!(
                "FM {sample_rate}Hz {label} note={note} ratio={ratio:?} fine={fine} Mraw={:.5} U={upper:.5} r_base={r_base:.6} r_case={r_case:.6} deltaP={delta_power:.9e} delta_negative={} r_changed={r_changed:.6} uncertainty={uncertainty:.6} review={}",
                voice.fm_modulator_raw_inc,
                delta_power < 0.0,
                r_case > REVIEW_LIMIT + uncertainty
            );
            for value in [r_base, r_case, delta_power, r_changed, denominator] {
                assert!(value.is_finite());
            }
            assert!(candidate_target
                .iter()
                .all(|sample| sample.is_finite() && sample.abs() <= 1.0));
            assert!(candidate_reference.iter().all(|sample| sample.is_finite()));
        }
    }
    analytic_zero_depth_two_tone_case(&taps);
}

fn calibrate_reference(taps: &[f64]) -> (f64, f64, f64, f64) {
    let mut passband_error: f64 = 0.0;
    let mut one_k_error: f64 = 0.0;
    let mut foldback_error: f64 = 0.0;
    let mut six_tenths_ratio: f64 = 0.0;
    let mut excluded_leakage: f64 = 0.0;
    let mut dc_error: f64 = 0.0;
    let mut raw_phase_error: f64 = 0.0;
    let mut injection_error: f64 = 0.0;
    for sample_rate in [44_100_u32, 48_000] {
        let dc_high = vec![1.0; high_length()];
        let dc_reference = filter_decimate(&dc_high, taps, sample_rate);
        dc_error = dc_error.max(
            dc_reference[FIRST_FRAME..]
                .iter()
                .map(|value| (value - 1.0).abs())
                .fold(0.0, f64::max),
        );
        for frequency in [0.01, 0.10, 0.22, 0.34, 0.35]
            .into_iter()
            .chain([1_000.0 / f64::from(sample_rate)])
        {
            for phase in [0.0, 0.37, 1.21] {
                let target = target_tone(frequency, phase);
                let high = high_tone(frequency, phase);
                for frame in 0..512 {
                    let aligned = high[(frame + 1) * UPSAMPLE - 1];
                    raw_phase_error = raw_phase_error.max((aligned - target[frame]).abs());
                }
                let reference = filter_decimate(&high, taps, sample_rate);
                let residual = subtract(&target, &reference);
                let baseline = rms(&target[FIRST_FRAME..]);
                let tone_error = low_band_energy(&residual[FIRST_FRAME..]).sqrt() / baseline;
                passband_error = passband_error.max(tone_error);
                if frequency == 1_000.0 / f64::from(sample_rate) {
                    one_k_error = one_k_error.max(tone_error);
                }
            }
        }
        for frequency in [0.40, 0.45, 0.46197, 0.49] {
            let target = target_tone(frequency, 0.43);
            let reference = filter_decimate(&high_tone(frequency, 0.43), taps, sample_rate);
            let residual = subtract(&target, &reference);
            excluded_leakage = excluded_leakage.max(
                low_band_energy(&residual[FIRST_FRAME..]).sqrt() / rms(&target[FIRST_FRAME..]),
            );
        }
        for frequency in [0.60, 0.65, 0.80, 0.95] {
            let high = high_tone(frequency, 0.73);
            let filtered = filter_decimate(&high, taps, sample_rate);
            if frequency == 0.60 {
                six_tenths_ratio = six_tenths_ratio.max(
                    rms(&filtered[FIRST_FRAME..])
                        / rms(&target_tone(frequency, 0.73)[FIRST_FRAME..]),
                );
            }
            foldback_error = foldback_error.max(
                low_band_energy(&filtered[FIRST_FRAME..]).sqrt()
                    / rms(&target_tone(frequency, 0.73)[FIRST_FRAME..]),
            );
        }
        let (recovered, target_rms) = injected_alias_calibration(taps, sample_rate);
        injection_error = injection_error.max((recovered - 0.02).abs());
        assert!(target_rms > 0.0);
        for frequency in [0.0, 0.20, 0.34, 0.35, 0.36, 0.40, 0.45, 0.46197, 0.49] {
            eprintln!(
                "FIR response {sample_rate}Hz at {frequency:.5}Fs: {:.8}",
                frequency_response(taps, frequency / UPSAMPLE as f64)
            );
        }
        for frequency in [0.0, 0.20, 0.34, 0.35] {
            assert!(
                (frequency_response(taps, frequency / UPSAMPLE as f64) - 1.0).abs()
                    <= REFERENCE_LIMIT,
                "passband tap response at {frequency}Fs"
            );
        }
    }
    let total = dc_error
        + passband_error
        + foldback_error
        + excluded_leakage
        + injection_error
        + raw_phase_error / std::f64::consts::SQRT_2;
    eprintln!(
        "FIR components DC={dc_error:.7} 1k={one_k_error:.7} 0.6Fs-foldback={six_tenths_ratio:.7} pass={passband_error:.7} fold-in-band={foldback_error:.7} excluded-leak={excluded_leakage:.7} injection={injection_error:.7} phase={raw_phase_error:.3e} total={total:.7}"
    );
    assert!(injection_error <= REFERENCE_LIMIT);
    (total, passband_error, foldback_error, injection_error)
}

fn injected_alias_calibration(taps: &[f64], sample_rate: u32) -> (f64, f64) {
    let carrier = target_tone(0.11, 0.29);
    let carrier_high = high_tone(0.11, 0.29);
    let alias_high = high_tone(0.80, -0.41);
    let alias = raw_decimate(&alias_high);
    let injected_high: Vec<_> = carrier_high
        .iter()
        .zip(alias_high)
        .map(|(carrier, alias)| carrier + 0.02 * alias)
        .collect();
    let target_with_alias: Vec<_> = carrier
        .iter()
        .zip(alias)
        .map(|(carrier, alias)| carrier + 0.02 * alias)
        .collect();
    let reference_carrier = filter_decimate(&carrier_high, taps, sample_rate);
    let reference_combined = filter_decimate(&injected_high, taps, sample_rate);
    let combined_residual = subtract(&target_with_alias, &reference_combined);
    let carrier_residual = subtract(&carrier, &reference_carrier);
    let injected_residual = subtract(&combined_residual, &carrier_residual);
    let denominator = rms(&carrier[FIRST_FRAME..]);
    (
        low_band_energy(&injected_residual[FIRST_FRAME..]).sqrt() / denominator,
        denominator,
    )
}

fn analytic_zero_depth_two_tone_case(taps: &[f64]) {
    let sample_rate = 48_000;
    let config = FmConfig {
        ratio: FmRatio::Five,
        ratio_fine_cents: 100,
        index: 50,
        mod_mix_pct: 100,
        ..FmConfig::default()
    };
    let mut engine = make_engine(config, sample_rate);
    engine.note_on(0, 108, 127, 10_000);
    let render_config = engine.synth_render_configs[0];
    let initial = *engine.synth_voice_pool.lane(0).unwrap();
    let mut voice = initial;
    voice.index_env.stage = EnvStage::Sustain;
    voice.index_env.level = 0.73;
    assert_eq!(voice.fm_index_limit, 0.0);
    assert_eq!(voice.fm_index_fundamental, 0.0);
    assert_eq!(voice.fm_index_second, 0.0);
    let mut ring = [0.0; crate::synth::pluck_string::RING_LEN];
    for _ in 0..512 {
        let carrier_phase = (voice.phase1 + voice.osc1_inc).fract();
        let modulator_phase = (voice.phase2 + voice.osc2_inc).fract();
        let expected = ((TAU as f32 * carrier_phase).sin()
            + voice.fm_direct_mix * (TAU as f32 * modulator_phase).sin())
            * voice.fm_normalization;
        let actual = source_sample(&render_config, &mut voice, &mut ring);
        assert_eq!(actual.to_bits(), expected.to_bits());
    }
    let VoiceSource::Fm {
        effective_ratio, ..
    } = render_config.source
    else {
        unreachable!();
    };
    let frozen = fixed_env(initial, 0.73);
    let high = render_high_rate(
        frozen,
        f64::from(initial.freq_hz),
        f64::from(initial.freq_hz) * f64::from(effective_ratio),
        FRAME_COUNT,
        sample_rate,
    );
    let reference = filter_decimate(&high, taps, sample_rate);
    let target = render_target(render_config, frozen, 127, FRAME_COUNT);
    let residual = subtract(&target, &reference);
    let lower_band = low_band_energy(&residual[FIRST_FRAME..]).sqrt() / rms(&target[FIRST_FRAME..]);
    eprintln!(
        "Analytic note108 two-tone check: Mraw={:.6} Fs, lower-band residual={lower_band:.6}; FIR gain on excluded direct tone={:.6}",
        voice.fm_modulator_raw_inc,
        frequency_response(taps, f64::from(voice.fm_modulator_raw_inc) / UPSAMPLE as f64)
    );
    assert!(lower_band.is_finite() && lower_band <= REFERENCE_LIMIT);
}

fn make_engine(config: FmConfig, sample_rate: u32) -> SynthEngine {
    let mut engine = SynthEngine::new(sample_rate);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "fm".into(),
            synth: default_synth_config(),
            fm: Some(config),
            pluck: None,
            drum: None,
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}

fn fixed_env(mut voice: Voice, level: f32) -> Voice {
    voice.index_env.stage = EnvStage::Sustain;
    voice.index_env.level = level;
    voice.index_env.stage_pos = 0;
    voice
}

fn render_target(
    config: SynthVoiceRenderConfig,
    mut voice: Voice,
    velocity: u8,
    frames: usize,
) -> Vec<f64> {
    voice.velocity = velocity;
    let mut ring = [0.0; crate::synth::pluck_string::RING_LEN];
    (0..frames)
        .map(|_| f64::from(source_sample(&config, &mut voice, &mut ring)))
        .collect()
}

fn render_high_rate(
    voice: Voice,
    carrier_hz: f64,
    modulator_hz: f64,
    frames: usize,
    sample_rate: u32,
) -> Vec<f64> {
    let count = (frames + 1) * UPSAMPLE + TAP_COUNT;
    let high_rate = f64::from(sample_rate) * UPSAMPLE as f64;
    let mut carrier_phase = 0.0;
    let mut modulator_phase = 0.0;
    (0..count)
        .map(|_| {
            carrier_phase = (carrier_phase + carrier_hz / high_rate).fract();
            modulator_phase = (modulator_phase + modulator_hz / high_rate).fract();
            let modulator = (TAU * modulator_phase) as f32;
            let value = fm_render::sample(
                (TAU * carrier_phase) as f32,
                (TAU * modulator_phase) as f32,
                modulator.sin(),
                voice.index_env.level,
                &voice,
            );
            assert!(value.is_finite() && value.abs() <= 1.0);
            f64::from(value)
        })
        .collect()
}

fn windowed_sinc_fir() -> Vec<f64> {
    let middle = (TAP_COUNT - 1) as f64 * 0.5;
    let cutoff = 0.45 / UPSAMPLE as f64;
    let mut taps: Vec<_> = (0..TAP_COUNT)
        .map(|index| {
            let offset = index as f64 - middle;
            let ideal = if offset == 0.0 {
                2.0 * cutoff
            } else {
                (2.0 * cutoff * std::f64::consts::PI * offset).sin()
                    / (std::f64::consts::PI * offset)
            };
            let position = index as f64 / (TAP_COUNT - 1) as f64;
            let window = 0.42 - 0.5 * (TAU * position).cos() + 0.08 * (2.0 * TAU * position).cos();
            ideal * window
        })
        .collect();
    let sum = taps.iter().sum::<f64>();
    taps.iter_mut().for_each(|tap| *tap /= sum);
    taps
}

fn high_length() -> usize {
    (FRAME_COUNT + 1) * UPSAMPLE + TAP_COUNT
}

fn target_tone(frequency: f64, phase: f64) -> Vec<f64> {
    (0..FRAME_COUNT)
        .map(|frame| (TAU * frequency * (frame + 1) as f64 + phase).sin())
        .collect()
}

fn high_tone(frequency: f64, phase: f64) -> Vec<f64> {
    (0..high_length())
        .map(|frame| (TAU * frequency * (frame + 1) as f64 / UPSAMPLE as f64 + phase).sin())
        .collect()
}

fn filter_decimate(high: &[f64], taps: &[f64], _sample_rate: u32) -> Vec<f64> {
    let delay = taps.len() / 2;
    (0..FIRST_FRAME)
        .map(|_| 0.0)
        .chain((FIRST_FRAME..FRAME_COUNT).map(|frame| {
            let index = (frame + 1) * UPSAMPLE - 1 + delay;
            taps.iter()
                .enumerate()
                .map(|(tap, coefficient)| coefficient * high[index - tap])
                .sum()
        }))
        .collect()
}

fn raw_decimate(high: &[f64]) -> Vec<f64> {
    (0..FRAME_COUNT)
        .map(|frame| high[(frame + 1) * UPSAMPLE - 1])
        .collect()
}

fn low_band_energy(signal: &[f64]) -> f64 {
    let hann: Vec<_> = (0..signal.len())
        .map(|index| 0.5 - 0.5 * (TAU * index as f64 / (signal.len() - 1) as f64).cos())
        .collect();
    let window_power = hann.iter().map(|value| value * value).sum::<f64>();
    let last_bin = (LOW_BAND * signal.len() as f64).floor() as usize;
    (0..=last_bin)
        .map(|bin| {
            let angle = TAU * bin as f64 / signal.len() as f64;
            let (sin_step, cos_step) = angle.sin_cos();
            let (mut sin_phase, mut cos_phase) = (0.0, 1.0);
            let (mut real, mut imaginary) = (0.0, 0.0);
            for (sample, window) in signal.iter().zip(&hann) {
                let value = sample * window;
                real += value * cos_phase;
                imaginary -= value * sin_phase;
                let next_sin = sin_phase * cos_step + cos_phase * sin_step;
                cos_phase = cos_phase * cos_step - sin_phase * sin_step;
                sin_phase = next_sin;
            }
            let one_sided = if bin == 0 { 1.0 } else { 2.0 };
            one_sided * (real * real + imaginary * imaginary)
        })
        .sum::<f64>()
        / (signal.len() as f64 * window_power)
}

fn frequency_response(taps: &[f64], high_rate_cycles: f64) -> f64 {
    let middle = (taps.len() - 1) as f64 * 0.5;
    let (real, imaginary) = taps
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |sum, (index, tap)| {
            let angle = TAU * high_rate_cycles * (index as f64 - middle);
            (sum.0 + tap * angle.cos(), sum.1 - tap * angle.sin())
        });
    real.hypot(imaginary)
}

fn subtract(left: &[f64], right: &[f64]) -> Vec<f64> {
    left.iter()
        .zip(right)
        .map(|(left, right)| left - right)
        .collect()
}

fn rms(samples: &[f64]) -> f64 {
    (samples.iter().map(|sample| sample * sample).sum::<f64>() / samples.len() as f64).sqrt()
}
