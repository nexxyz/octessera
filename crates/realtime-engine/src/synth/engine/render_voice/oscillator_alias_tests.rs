use super::*;

const FRAMES: usize = 4096;
const CYCLES: usize = 223;

fn osc(waveform: WaveformId, pulse_duty: f32) -> OscRenderConfig {
    OscRenderConfig {
        waveform,
        octave_mul: 1.0,
        detune_mul: 1.0,
        level: 1.0,
        pulse_duty,
    }
}

/// Share of the signal power that is not on the harmonics of a tone with exactly `CYCLES`
/// periods in `FRAMES` samples: everything folded back from above Nyquist.
fn alias_power_db(cfg: OscRenderConfig) -> f64 {
    let inc = CYCLES as f32 / FRAMES as f32;
    let mut phase = 0.0;
    let samples: Vec<f64> = (0..FRAMES)
        .map(|_| f64::from(osc_sample_precomputed(cfg, inc, &mut phase)))
        .collect();
    let bin_power = |bin: usize| {
        let (mut re, mut im) = (0.0, 0.0);
        for (n, sample) in samples.iter().enumerate() {
            let angle = std::f64::consts::TAU * ((bin * n) % FRAMES) as f64 / FRAMES as f64;
            re += sample * angle.cos();
            im -= sample * angle.sin();
        }
        re * re + im * im
    };
    let total: f64 = samples.iter().map(|sample| sample * sample).sum();
    let harmonics: f64 = bin_power(0) / FRAMES as f64
        + (1..)
            .map(|harmonic| harmonic * CYCLES)
            .take_while(|bin| *bin < FRAMES / 2)
            .map(|bin| 2.0 * bin_power(bin) / FRAMES as f64)
            .sum::<f64>();
    10.0 * ((total - harmonics).max(1.0e-12) / total).log10()
}

#[test]
fn edge_waveforms_keep_folded_harmonics_low_on_a_high_note() {
    for (name, cfg) in [
        ("saw", osc(WaveformId::Saw, 0.5)),
        ("square", osc(WaveformId::Square, 0.5)),
        ("pulse", osc(WaveformId::Pulse, 0.25)),
    ] {
        let alias = alias_power_db(cfg);
        assert!(alias < -25.0, "{name}: aliasing at {alias:.1} dB");
    }
}
