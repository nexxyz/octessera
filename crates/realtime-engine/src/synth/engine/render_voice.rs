use super::*;

pub(super) fn render_synth_voice_sample(
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
    let osc1 = osc_sample(cfg.osc1, v.freq_hz, &mut v.phase1, sample_rate);
    let osc2 = osc_sample(cfg.osc2, v.freq_hz, &mut v.phase2, sample_rate);
    let dry = (osc1 + osc2) * 0.5;
    let cutoff = synth_voice_cutoff(&cfg, mods.cutoff_cc, filt_env);
    let q = synth_voice_q(&cfg, mods.resonance_cc);
    let filtered = v.filt.process(dry, cfg.filter.kind, cutoff, q, sample_rate);
    filtered * amp_env * vel_gain * gain * 0.35
}

fn synth_voice_cutoff(cfg: &SynthConfig, cutoff_cc: f32, filt_env: f32) -> f32 {
    let cutoff_base = cfg.filter.cutoff_hz;
    let env_amt = (cfg.filter.env_amount_pct / 100.0).clamp(-1.0, 1.0);
    let cutoff_env = cutoff_base * (1.0 + env_amt * filt_env).max(0.0);
    if cutoff_cc > 0.0 {
        120.0 + cutoff_cc * 15_880.0
    } else {
        cutoff_env
    }
}

fn synth_voice_q(cfg: &SynthConfig, resonance_cc: f32) -> f32 {
    let resonance = if resonance_cc > 0.0 {
        resonance_cc * 100.0
    } else {
        cfg.filter.resonance
    };
    0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5
}
