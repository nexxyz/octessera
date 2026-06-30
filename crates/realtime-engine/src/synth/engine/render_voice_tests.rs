use super::*;

#[test]
fn precomputed_render_matches_reference_for_sine_with_mods() {
    let mut cfg = default_synth_config();
    cfg.osc1.waveform = WaveformId::Sine;
    cfg.osc2.waveform = WaveformId::Sine;
    cfg.osc1.detune_cents = 7.0;
    cfg.osc2.octave = -1;
    cfg.amp.velocity_sensitivity_pct = 80.0;
    cfg.amp.gain_pct = 73.0;
    cfg.filter.env_amount_pct = 45.0;
    assert_precomputed_matches_reference(
        cfg,
        InstrumentMod {
            cutoff_cc: 0.6,
            resonance_cc: 0.3,
        },
    );
}

#[test]
fn precomputed_render_matches_reference_for_pulse_and_square() {
    let mut cfg = default_synth_config();
    cfg.osc1.waveform = WaveformId::Pulse;
    cfg.osc1.pulse_width_pct = 27.0;
    cfg.osc1.octave = 1;
    cfg.osc2.waveform = WaveformId::Square;
    cfg.osc2.detune_cents = -13.0;
    cfg.amp.velocity_sensitivity_pct = 35.0;
    cfg.filter.env_amount_pct = -30.0;
    assert_precomputed_matches_reference(cfg, InstrumentMod::new());
}

fn assert_precomputed_matches_reference(cfg: SynthConfig, mods: InstrumentMod) {
    let render_cfg = SynthVoiceRenderConfig::from_config(cfg);
    let mut actual = test_voice();
    let mut expected = test_voice();
    refresh_synth_voice_render_cache(&mut actual, &render_cfg, 44_100, 1);
    for frame in 0..1024 {
        let amp_env = 0.2 + (frame as f32 * 0.0003);
        let filt_env = ((frame as f32) * 0.011).sin() * 0.5 + 0.5;
        let actual_sample = render_synth_voice_sample_precomputed(
            44_100,
            mods,
            &render_cfg,
            &mut actual,
            amp_env,
            filt_env,
        );
        let expected_sample = reference_render_synth_voice_sample(
            44_100,
            mods,
            cfg,
            &mut expected,
            amp_env,
            filt_env,
        );
        assert_eq!(
            actual_sample.to_bits(),
            expected_sample.to_bits(),
            "sample {frame}"
        );
    }
}

#[test]
fn render_cache_refresh_matches_reference_after_config_change() {
    let mut first = default_synth_config();
    first.osc1.waveform = WaveformId::Sine;
    first.osc2.waveform = WaveformId::Triangle;
    first.osc2.detune_cents = -5.0;
    let mut second = first;
    second.osc1.octave = 1;
    second.osc2.detune_cents = 19.0;
    second.osc2.waveform = WaveformId::Pulse;
    second.osc2.pulse_width_pct = 41.0;

    let first_render = SynthVoiceRenderConfig::from_config(first);
    let second_render = SynthVoiceRenderConfig::from_config(second);
    let mut actual = test_voice();
    let mut expected = test_voice();
    refresh_synth_voice_render_cache(&mut actual, &first_render, 44_100, 1);

    for frame in 0..1024 {
        let (cfg, render_cfg, revision) = if frame < 512 {
            (first, &first_render, 1)
        } else {
            (second, &second_render, 2)
        };
        if actual.render_revision != revision {
            refresh_synth_voice_render_cache(&mut actual, render_cfg, 44_100, revision);
        }
        let amp_env = 0.3 + (frame as f32 * 0.0002);
        let filt_env = ((frame as f32) * 0.017).sin() * 0.5 + 0.5;
        let actual_sample = render_synth_voice_sample_precomputed(
            44_100,
            InstrumentMod::new(),
            render_cfg,
            &mut actual,
            amp_env,
            filt_env,
        );
        let expected_sample = reference_render_synth_voice_sample(
            44_100,
            InstrumentMod::new(),
            cfg,
            &mut expected,
            amp_env,
            filt_env,
        );
        assert_eq!(
            actual_sample.to_bits(),
            expected_sample.to_bits(),
            "sample {frame}"
        );
    }
}

fn test_voice() -> Voice {
    let mut voice = Voice::off();
    voice.active = true;
    voice.velocity = 83;
    voice.freq_hz = 261.62558;
    voice.filt = BiquadState::new();
    voice
}

fn reference_render_synth_voice_sample(
    sample_rate: u32,
    mods: InstrumentMod,
    cfg: SynthConfig,
    v: &mut Voice,
    amp_env: f32,
    filt_env: f32,
) -> f32 {
    let vel = (v.velocity as f32 / 127.0).clamp(0.0, 1.0);
    let vel_sens = (cfg.amp.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
    let vel_gain = (1.0 - vel_sens) + vel_sens * vel;
    let gain = (cfg.amp.gain_pct / 100.0).clamp(0.0, 1.0);
    let osc1 = reference_osc_sample(cfg.osc1, v.freq_hz, &mut v.phase1, sample_rate);
    let osc2 = reference_osc_sample(cfg.osc2, v.freq_hz, &mut v.phase2, sample_rate);
    let dry = (osc1 + osc2) * 0.5;
    let cutoff = reference_synth_voice_cutoff(&cfg, mods.cutoff_cc, filt_env);
    let q = reference_synth_voice_q(&cfg, mods.resonance_cc);
    let filtered = v.filt.process(dry, cfg.filter.kind, cutoff, q, sample_rate);
    filtered * amp_env * vel_gain * gain * 0.35
}

fn reference_synth_voice_cutoff(cfg: &SynthConfig, cutoff_cc: f32, filt_env: f32) -> f32 {
    let cutoff_base = cfg.filter.cutoff_hz;
    let env_amt = (cfg.filter.env_amount_pct / 100.0).clamp(-1.0, 1.0);
    let cutoff_env = cutoff_base * (1.0 + env_amt * filt_env).max(0.0);
    if cutoff_cc > 0.0 {
        120.0 + cutoff_cc * 15_880.0
    } else {
        cutoff_env
    }
}

fn reference_synth_voice_q(cfg: &SynthConfig, resonance_cc: f32) -> f32 {
    let resonance = if resonance_cc > 0.0 {
        resonance_cc * 100.0
    } else {
        cfg.filter.resonance
    };
    0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5
}

fn reference_osc_sample(cfg: OscConfig, base_freq: f32, phase: &mut f32, sample_rate: u32) -> f32 {
    let octave_mul = 2.0_f32.powi(cfg.octave.clamp(-2, 2));
    let detune_mul = 2.0_f32.powf(cfg.detune_cents.clamp(-1200.0, 1200.0) / 1200.0);
    let freq = base_freq * octave_mul * detune_mul;
    let inc = (freq / (sample_rate as f32)).clamp(0.0, 0.5);
    *phase = (*phase + inc).fract();

    let raw = match cfg.waveform {
        WaveformId::Sine => (2.0 * PI * *phase).sin(),
        WaveformId::Triangle => 4.0 * (*phase - 0.5).abs() - 1.0,
        WaveformId::Saw => 2.0 * *phase - 1.0,
        WaveformId::Square => {
            if *phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
        WaveformId::Pulse => {
            let duty = (cfg.pulse_width_pct / 100.0).clamp(0.05, 0.95);
            if *phase < duty {
                1.0
            } else {
                -1.0
            }
        }
    };

    let level = (cfg.level_pct / 100.0).clamp(0.0, 1.0);
    raw * level
}
