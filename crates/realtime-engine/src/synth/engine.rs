use super::fx::{
    fx_bus_state_from_params, fx_bus_state_matches_params, master_fx_state_from_params,
    master_fx_state_matches_params, process_fx_bus_slot, process_master_fx_slot, FxBusState,
    MasterFxState,
};
use super::fx_params::{compile_fx_bus_params, FxBusParams};
use super::types::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::f32::consts::PI;

pub(super) const FREEZE_INJECT_MS: u32 = 120;
pub(super) const DRY_HISTORY_FRAMES: usize = 2048;

pub struct SynthEngine {
    sample_rate: u32,
    sample_clock: u64,
    slot_kind: [InstrumentKind; INSTRUMENT_SLOT_COUNT],
    instruments: [SynthConfig; INSTRUMENT_SLOT_COUNT],
    sample_banks: Vec<SampleBankConfig>,
    mods: [InstrumentMod; INSTRUMENT_SLOT_COUNT],
    voices: [[Voice; VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
    sample_voices: [[SampleVoice; VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
    preview_sample_voices: Vec<PreviewSampleVoice>,
    slot_route: [usize; INSTRUMENT_SLOT_COUNT],
    slot_pan_pos: [usize; INSTRUMENT_SLOT_COUNT],
    slot_volume: [f32; INSTRUMENT_SLOT_COUNT],
    bus_pan_pos: Vec<usize>,
    bus_mono_scratch: Vec<f32>,
    bus_mono_snapshot: Vec<f32>,
    bus_slot_params: Vec<[FxBusParams; BUS_SLOTS_PER_BUS]>,
    bus_slot_state: Vec<[FxBusState; BUS_SLOTS_PER_BUS]>,
    master_slot_params: Vec<FxBusParams>,
    master_slot_state: Vec<MasterFxState>,
    pan_positions: usize,
    master_volume: f32,
    voice_stealing_mode: VoiceStealingMode,
    smoothed_load_ratio: f32,
    voice_steal_since_status: bool,
    momentary_fx: Vec<MomentaryFxState>,
    dry_history: Vec<f32>,
    dry_history_pos: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InstrumentKind {
    Synth,
    Sample,
    Midi,
    None,
}

#[derive(Clone, Copy, Debug)]
struct SampleVoice {
    active: bool,
    sample_slot: usize,
    pos: f32,
    step: f32,
    gain: f32,
}

#[derive(Clone, Debug)]
struct PreviewSampleVoice {
    slot: usize,
    buffer: SampleBuffer,
    pos: f32,
    step: f32,
    gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MomentaryFxKind {
    Stutter,
    Freeze,
    FilterSweep,
    PitchShift,
}

struct MomentaryFxState {
    id: String,
    kind: MomentaryFxKind,
    params: BTreeMap<String, Value>,
    target: MomentaryFxTarget,
    releasing: bool,
    release_pos: u32,
    release_len: u32,
    sweep_pos: f32,
    filt_l: BiquadState,
    filt_r: BiquadState,
    pitch_shifter: LivePitchShift,
    pitch_ramp_pos: u32,
    pitch_ramp_len: u32,
    stutter_l: Vec<f32>,
    stutter_r: Vec<f32>,
    stutter_write: usize,
    stutter_ready: bool,
    stutter_ramp_len: usize,
    stutter_ramp_pos: usize,
    freeze_bufs: [Vec<f32>; 4],
    freeze_idxs: [usize; 4],
    freeze_lp: [f32; 4],
    freeze_inject_pos: u32,
    freeze_inject_len: u32,
}

impl MomentaryFxState {
    fn new(
        id: String,
        kind: MomentaryFxKind,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTarget,
        sample_rate: u32,
    ) -> Self {
        let ramp_samples = ((sample_rate as f32 * 0.002) as usize).max(1);
        let pitch_ramp_len = ((sample_rate as f32 * 0.002) as u32).max(1);
        const DELAY_LENS: [usize; 4] = [1557, 1617, 1491, 1422];
        let freeze_bufs: [Vec<f32>; 4] =
            DELAY_LENS.map(|n| vec![0.0; (n * sample_rate as usize / 44_100).max(1)]);
        let freeze_inject_len = (sample_rate * FREEZE_INJECT_MS / 1000).max(1);
        Self {
            id,
            kind,
            params,
            target,
            releasing: false,
            release_pos: 0,
            release_len: 0,
            sweep_pos: 0.0,
            filt_l: BiquadState::new(),
            filt_r: BiquadState::new(),
            pitch_shifter: LivePitchShift::new(sample_rate),
            pitch_ramp_pos: 0,
            pitch_ramp_len,
            stutter_l: Vec::new(),
            stutter_r: Vec::new(),
            stutter_write: 0,
            stutter_ready: false,
            stutter_ramp_len: ramp_samples,
            stutter_ramp_pos: 0,
            freeze_bufs,
            freeze_idxs: [0; 4],
            freeze_lp: [0.0; 4],
            freeze_inject_pos: 0,
            freeze_inject_len,
        }
    }
}

impl SampleVoice {
    const fn off() -> Self {
        Self {
            active: false,
            sample_slot: 0,
            pos: 0.0,
            step: 1.0,
            gain: 0.0,
        }
    }
}

pub(super) const PITCH_BUF_FRAMES: usize = 2048;
const PITCH_MIN_DELAY: f32 = 64.0;
const PITCH_RANGE: f32 = 1024.0;

struct LivePitchShift {
    buf: Vec<f32>,
    buf_len: usize,
    pub(super) write_pos: usize,
    pos: f32,
    min_delay: f32,
    range: f32,
}

impl LivePitchShift {
    fn new(sample_rate: u32) -> Self {
        let _ = sample_rate;
        Self {
            buf: vec![0.0; PITCH_BUF_FRAMES * 2],
            buf_len: PITCH_BUF_FRAMES,
            write_pos: 0,
            pos: PITCH_RANGE * 0.25,
            min_delay: PITCH_MIN_DELAY,
            range: PITCH_RANGE,
        }
    }

    fn prefill(&mut self, data: &[f32]) {
        let frames = data.len().min(self.buf.len()) / 2;
        let offset = self.buf_len.saturating_sub(frames);
        for i in 0..frames {
            let src = i * 2;
            let dst = (offset + i) * 2;
            if src + 1 < data.len() && dst + 1 < self.buf.len() {
                self.buf[dst] = data[src];
                self.buf[dst + 1] = data[src + 1];
            }
        }
        self.write_pos = self.buf_len - 1;
        self.pos = PITCH_RANGE * 0.25;
    }

    fn process_frame(&mut self, l: f32, r: f32, ratio: f32) -> (f32, f32) {
        let buf_len_f = self.buf_len as f32;
        let min_delay = self.min_delay;
        let range = self.range;

        self.buf[self.write_pos * 2] = l;
        self.buf[self.write_pos * 2 + 1] = r;
        self.write_pos = (self.write_pos + 1) % self.buf_len;

        self.pos += 1.0 - ratio;
        let pos_norm = ((self.pos % range) + range) % range;

        let delay_a = min_delay + pos_norm;
        let delay_b = min_delay + ((pos_norm + range * 0.5) % range);

        let read_a = (self.write_pos as f32 - delay_a + buf_len_f) % buf_len_f;
        let read_b = (self.write_pos as f32 - delay_b + buf_len_f) % buf_len_f;

        let phase = ((pos_norm / range) + 0.5) % 1.0;
        let angle = phase * PI;
        let gain_a = angle.cos().powi(2);
        let gain_b = angle.sin().powi(2);

        let out_l = gain_a * Self::interp(&self.buf, read_a, 0)
            + gain_b * Self::interp(&self.buf, read_b, 0);
        let out_r = gain_a * Self::interp(&self.buf, read_a, 1)
            + gain_b * Self::interp(&self.buf, read_b, 1);

        (out_l, out_r)
    }

    fn interp(buf: &[f32], pos: f32, ch: usize) -> f32 {
        let i = pos as usize;
        let frac = pos - i as f32;
        let idx = i * 2 + ch;
        let a = buf.get(idx).copied().unwrap_or(0.0);
        let b = buf.get(idx + 2).copied().unwrap_or(0.0);
        a + frac * (b - a)
    }
}

impl SynthEngine {
    pub fn new(sample_rate: u32) -> Self {
        let default = default_synth_config();
        Self {
            sample_rate,
            sample_clock: 0,
            slot_kind: [InstrumentKind::Synth; INSTRUMENT_SLOT_COUNT],
            instruments: [default; INSTRUMENT_SLOT_COUNT],
            sample_banks: vec![SampleBankConfig::default(); INSTRUMENT_SLOT_COUNT],
            mods: [InstrumentMod::new(); INSTRUMENT_SLOT_COUNT],
            voices: [[Voice::off(); VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
            sample_voices: [[SampleVoice::off(); VOICES_PER_SLOT]; INSTRUMENT_SLOT_COUNT],
            preview_sample_voices: Vec::new(),
            slot_route: [0; INSTRUMENT_SLOT_COUNT],
            slot_pan_pos: [DEFAULT_PAN_POSITIONS / 2; INSTRUMENT_SLOT_COUNT],
            slot_volume: [1.0; INSTRUMENT_SLOT_COUNT],
            bus_pan_pos: Vec::new(),
            bus_mono_scratch: Vec::new(),
            bus_mono_snapshot: Vec::new(),
            bus_slot_params: Vec::new(),
            bus_slot_state: Vec::new(),
            master_slot_params: Vec::new(),
            master_slot_state: Vec::new(),
            pan_positions: DEFAULT_PAN_POSITIONS,
            master_volume: 1.0,
            voice_stealing_mode: VoiceStealingMode::Balanced,
            smoothed_load_ratio: 0.0,
            voice_steal_since_status: false,
            momentary_fx: Vec::new(),
            dry_history: vec![0.0; DRY_HISTORY_FRAMES * 2],
            dry_history_pos: 0,
        }
    }

    pub fn momentary_fx_start(
        &mut self,
        id: String,
        fx_type: String,
        params: BTreeMap<String, Value>,
        target: MomentaryFxTarget,
    ) {
        let Some(kind) = parse_momentary_fx_kind(&fx_type) else {
            return;
        };
        self.momentary_fx.retain(|fx| fx.id != id);
        self.momentary_fx.push(MomentaryFxState::new(
            id,
            kind,
            params,
            target,
            self.sample_rate,
        ));

        if kind == MomentaryFxKind::PitchShift {
            if let Some(fx) = self.momentary_fx.last_mut() {
                let pos = self.dry_history_pos;
                let len = self.dry_history.len();
                let mut contiguous = Vec::with_capacity(len);
                if pos < len {
                    contiguous.extend_from_slice(&self.dry_history[pos..]);
                }
                if pos > 0 {
                    contiguous.extend_from_slice(&self.dry_history[..pos]);
                }
                fx.pitch_shifter.prefill(&contiguous);
            }
        }
    }

    pub fn momentary_fx_stop(&mut self, id: &str) {
        let should_remove = self
            .momentary_fx
            .iter()
            .find(|fx| fx.id == id)
            .map(|fx| {
                matches!(
                    fx.kind,
                    MomentaryFxKind::Stutter | MomentaryFxKind::PitchShift
                )
            })
            .unwrap_or(true);
        if should_remove {
            self.momentary_fx.retain(|fx| fx.id != id);
        } else if let Some(fx) = self.momentary_fx.iter_mut().find(|fx| fx.id == id) {
            fx.releasing = true;
            fx.release_pos = 0;
            if fx.kind == MomentaryFxKind::Freeze {
                let ms = param_f32(&fx.params, "releaseMs", 500.0);
                fx.release_len = ms_to_samples(ms, self.sample_rate).max(1);
            }
        }
    }

    pub fn momentary_fx_update(&mut self, id: &str, params: BTreeMap<String, Value>) {
        if let Some(fx) = self.momentary_fx.iter_mut().find(|fx| fx.id == id) {
            fx.params = params;
        }
    }

    pub fn set_voice_stealing_mode(&mut self, mode: VoiceStealingMode) {
        self.voice_stealing_mode = mode;
    }

    pub fn set_runtime_load_ratio(&mut self, ratio: f32) {
        let r = ratio.clamp(0.0, 2.0);
        self.smoothed_load_ratio = 0.9 * self.smoothed_load_ratio + 0.1 * r;
    }

    pub fn audio_load_status(&mut self) -> AudioLoadStatus {
        let status = AudioLoadStatus {
            ratio: self.smoothed_load_ratio,
            voice_steal: self.voice_steal_since_status,
        };
        self.voice_steal_since_status = false;
        status
    }

    pub fn set_instruments(&mut self, cfg: InstrumentsConfig) {
        self.pan_positions = cfg.pan_positions.max(1);
        self.master_volume = (cfg.master_volume / 100.0).clamp(0.0, 1.0);
        for (idx, slot) in cfg.instruments.into_iter().enumerate() {
            if idx >= INSTRUMENT_SLOT_COUNT {
                break;
            }
            self.slot_kind[idx] = parse_instrument_kind(&slot.kind);
            if self.slot_kind[idx] != InstrumentKind::Synth {
                if let Some(m) = slot.mixer {
                    self.slot_route[idx] = parse_route(&m.route);
                    self.slot_pan_pos[idx] = m.pan_pos.min(self.pan_positions - 1);
                    self.slot_volume[idx] = (m.volume / 100.0).clamp(0.0, 1.0);
                }
                continue;
            }
            self.instruments[idx] = slot.synth;
            if let Some(m) = slot.mixer {
                self.slot_route[idx] = parse_route(&m.route);
                self.slot_pan_pos[idx] = m.pan_pos.min(self.pan_positions - 1);
                self.slot_volume[idx] = (m.volume / 100.0).clamp(0.0, 1.0);
            }
        }
        let mut next_bus_pan_pos = Vec::new();
        let mut next_bus_slot_params = Vec::new();
        let mut next_bus_slot_state = Vec::new();
        let mut next_master_slot_params = Vec::new();
        let mut next_master_slot_state = Vec::new();
        if let Some(mixer) = cfg.mixer {
            for (bus_idx, bus) in mixer.buses.into_iter().enumerate() {
                next_bus_pan_pos.push(bus.pan_pos.min(self.pan_positions - 1));
                let mut cfgs: [FxBusSlotConfig; BUS_SLOTS_PER_BUS] =
                    std::array::from_fn(|_| FxBusSlotConfig::Kind("none".to_string()));
                for (j, slot) in bus.slots.into_iter().enumerate().take(BUS_SLOTS_PER_BUS) {
                    cfgs[j] = slot;
                }
                let params: [FxBusParams; BUS_SLOTS_PER_BUS] =
                    std::array::from_fn(|j| compile_fx_bus_params(&cfgs[j]));
                let states: [FxBusState; BUS_SLOTS_PER_BUS] = std::array::from_fn(|j| {
                    self.bus_slot_state
                        .get(bus_idx)
                        .and_then(|states| states.get(j))
                        .filter(|state| fx_bus_state_matches_params(state, &params[j]))
                        .cloned()
                        .unwrap_or_else(|| fx_bus_state_from_params(&params[j], self.sample_rate))
                });
                next_bus_slot_params.push(params);
                next_bus_slot_state.push(states);
            }
            if let Some(master) = mixer.master {
                for (slot_idx, slot) in master.slots.into_iter().enumerate() {
                    let params = compile_fx_bus_params(&slot);
                    let state = self
                        .master_slot_state
                        .get(slot_idx)
                        .filter(|state| master_fx_state_matches_params(state, &params))
                        .cloned()
                        .unwrap_or_else(|| master_fx_state_from_params(&params));
                    next_master_slot_params.push(params);
                    next_master_slot_state.push(state);
                }
            }
        }
        self.bus_pan_pos = next_bus_pan_pos;
        self.bus_slot_params = next_bus_slot_params;
        self.bus_slot_state = next_bus_slot_state;
        self.master_slot_params = next_master_slot_params;
        self.master_slot_state = next_master_slot_state;
        self.bus_mono_scratch.resize(self.bus_pan_pos.len(), 0.0);
    }

    pub fn set_sample_banks(&mut self, banks: Vec<SampleBankConfig>) {
        self.sample_banks = banks;
        self.sample_banks
            .resize(INSTRUMENT_SLOT_COUNT, SampleBankConfig::default());
        for pool in self.sample_voices.iter_mut() {
            for voice in pool.iter_mut() {
                voice.active = false;
            }
        }
    }

    pub fn preview_sample(&mut self, instrument_slot: u8, buffer: SampleBuffer, velocity: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if buffer.samples.is_empty() || buffer.channels == 0 || buffer.sample_rate == 0 {
            return;
        }
        let bank = self.sample_banks.get(slot).cloned().unwrap_or_default();
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (bank.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (bank.gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(bank.tune_semis / 12.0);
        let step = pitch * buffer.sample_rate as f32 / self.sample_rate as f32;
        self.preview_sample_voices.push(PreviewSampleVoice {
            slot,
            buffer,
            pos: 0.0,
            step,
            gain,
        });
    }

    pub fn note_on(&mut self, instrument_slot: u8, midi_note: u8, velocity: u8, duration_ms: u32) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::Sample {
            self.sample_note_on(slot, midi_note, velocity);
            return;
        }
        if self.slot_kind[slot] != InstrumentKind::Synth {
            return;
        }
        let v = velocity.max(1);
        let duration_samples = ms_to_samples(duration_ms as f32, self.sample_rate).max(1) as u64;
        let note_off_sample = self.sample_clock.saturating_add(duration_samples);
        let freq = midi_note_to_hz(midi_note);

        let pool = &mut self.voices[slot];
        let mut voice_index: Option<usize> = None;
        for (i, voice) in pool.iter().enumerate() {
            if !voice.active {
                voice_index = Some(i);
                break;
            }
        }
        let i = match voice_index {
            Some(i) => i,
            None => {
                self.voice_steal_since_status = true;
                Self::steal_voice_index(pool)
            }
        };

        let cfg = self.instruments[slot];
        let amp_env = EnvState::note_on(cfg.amp_env, self.sample_rate);
        let filt_env = EnvState::note_on(cfg.filter_env, self.sample_rate);
        pool[i] = Voice {
            active: true,
            instrument_slot: slot as u8,
            midi_note,
            velocity: v,
            note_off_sample,
            started_sample: self.sample_clock,
            freq_hz: freq,
            phase1: 0.0,
            phase2: 0.0,
            amp_env,
            filt_env,
            filt: BiquadState::new(),
        };

        self.enforce_global_voice_budget();
    }

    pub fn cc(&mut self, instrument_slot: u8, controller: u8, value: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::None {
            return;
        }
        if controller == 74 {
            self.mods[slot].cutoff_cc = (value as f32 / 127.0).clamp(0.0, 1.0);
        } else if controller == 71 {
            self.mods[slot].resonance_cc = (value as f32 / 127.0).clamp(0.0, 1.0);
        } else if controller == 120 || controller == 123 {
            self.mods[slot] = InstrumentMod::new();
        }
    }

    pub fn note_off(&mut self, instrument_slot: u8, midi_note: u8) {
        let slot = (instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
        if self.slot_kind[slot] == InstrumentKind::None {
            return;
        }
        if self.slot_kind[slot] == InstrumentKind::Sample {
            let sample_slot = sample_slot_for_note(midi_note);
            for voice in self.sample_voices[slot].iter_mut() {
                if voice.active && voice.sample_slot == sample_slot {
                    voice.active = false;
                }
            }
            return;
        }
        let cfg = self.instruments[slot];
        for voice in self.voices[slot].iter_mut() {
            if !voice.active {
                continue;
            }
            if voice.midi_note != midi_note {
                continue;
            }
            voice.amp_env.begin_release(cfg.amp_env, self.sample_rate);
            voice
                .filt_env
                .begin_release(cfg.filter_env, self.sample_rate);
            voice.note_off_sample = self.sample_clock;
        }
    }

    pub fn all_notes_off(&mut self) {
        self.preview_sample_voices.clear();
        for pool in self.sample_voices.iter_mut() {
            for voice in pool.iter_mut() {
                voice.active = false;
            }
        }
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            let cfg = self.instruments[slot];
            for voice in self.voices[slot].iter_mut() {
                if !voice.active {
                    continue;
                }
                voice.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                voice
                    .filt_env
                    .begin_release(cfg.filter_env, self.sample_rate);
                voice.note_off_sample = self.sample_clock;
            }
        }
    }

    pub fn next_sample(&mut self) -> f32 {
        let (l, r) = self.next_stereo_sample();
        (l + r) * 0.5
    }

    pub fn next_stereo_sample(&mut self) -> (f32, f32) {
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        self.render_sample_voices(&mut slot_out);
        self.render_preview_sample_voices(&mut slot_out);
        for pool in self.voices.iter_mut() {
            for v in pool.iter_mut() {
                if !v.active {
                    continue;
                }
                let slot = (v.instrument_slot as usize).min(INSTRUMENT_SLOT_COUNT - 1);
                let cfg = self.instruments[slot];

                if self.sample_clock >= v.note_off_sample {
                    v.amp_env.begin_release(cfg.amp_env, self.sample_rate);
                    v.filt_env.begin_release(cfg.filter_env, self.sample_rate);
                }

                let amp_env = v.amp_env.next();
                let filt_env = v.filt_env.next();
                if v.amp_env.is_off() {
                    v.active = false;
                    continue;
                }

                let vel = (v.velocity as f32 / 127.0).clamp(0.0, 1.0);
                let vel_sens = (cfg.amp.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
                let vel_gain = (1.0 - vel_sens) + vel_sens * vel;
                let gain = (cfg.amp.gain_pct / 100.0).clamp(0.0, 1.0);

                let osc1 = osc_sample(cfg.osc1, v.freq_hz, &mut v.phase1, self.sample_rate);
                let osc2 = osc_sample(cfg.osc2, v.freq_hz, &mut v.phase2, self.sample_rate);
                let dry = (osc1 + osc2) * 0.5;

                let cutoff_base = cfg.filter.cutoff_hz;
                let env_amt = (cfg.filter.env_amount_pct / 100.0).clamp(-1.0, 1.0);
                let cutoff_env = cutoff_base * (1.0 + env_amt * filt_env).max(0.0);
                let cutoff_cc = self.mods[slot].cutoff_cc;
                let cutoff = if cutoff_cc > 0.0 {
                    120.0 + cutoff_cc * 15_880.0
                } else {
                    cutoff_env
                };
                let res_mod = self.mods[slot].resonance_cc;
                let resonance = if res_mod > 0.0 {
                    res_mod * 100.0
                } else {
                    cfg.filter.resonance
                };
                let q = 0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5;
                let filtered = v
                    .filt
                    .process(dry, cfg.filter.kind, cutoff, q, self.sample_rate);

                let sample = filtered * amp_env * vel_gain * gain * 0.35;
                slot_out[slot] += sample;
            }
        }

        if self.bus_mono_scratch.len() != self.bus_pan_pos.len() {
            self.bus_mono_scratch.resize(self.bus_pan_pos.len(), 0.0);
        } else {
            self.bus_mono_scratch.fill(0.0);
        }
        if self.bus_mono_snapshot.len() != self.bus_mono_scratch.len() {
            self.bus_mono_snapshot
                .resize(self.bus_mono_scratch.len(), 0.0);
        }
        let mut left = 0.0_f32;
        let mut right = 0.0_f32;
        for (slot, sample) in slot_out.iter().enumerate() {
            let mut sample = *sample * self.slot_volume[slot];
            let (fx_l, fx_r) = self.process_momentary_fx_target(
                MomentaryFxTarget::Instrument { index: slot },
                sample,
                sample,
            );
            sample = (fx_l + fx_r) * 0.5;
            let route = self.slot_route[slot];
            if route == 0 {
                let (gl, gr) = pan_gains(self.slot_pan_pos[slot], self.pan_positions);
                left += sample * gl;
                right += sample * gr;
            } else {
                let bus = route - 1;
                if bus < self.bus_mono_scratch.len() {
                    self.bus_mono_scratch[bus] += sample;
                } else {
                    let (gl, gr) = pan_gains(self.slot_pan_pos[slot], self.pan_positions);
                    left += sample * gl;
                    right += sample * gr;
                }
            }
        }
        // Snapshot is used for ducking sources without holding a borrow of `self`.
        self.bus_mono_snapshot
            .copy_from_slice(&self.bus_mono_scratch);
        for bus_idx in 0..self.bus_mono_scratch.len() {
            let mut processed = self.bus_mono_scratch[bus_idx];
            let mut pan_override: Option<f32> = None;
            if let (Some(params), Some(states)) = (
                self.bus_slot_params.get(bus_idx),
                self.bus_slot_state.get_mut(bus_idx),
            ) {
                for j in 0..BUS_SLOTS_PER_BUS {
                    processed = process_fx_bus_slot(
                        &params[j],
                        &mut states[j],
                        processed,
                        &slot_out,
                        &self.bus_mono_snapshot,
                        self.sample_rate,
                    );
                    if let FxBusState::AutoPan { pos, .. } = states[j] {
                        pan_override = Some(pos.clamp(0.0, 1.0));
                    }
                }
            }

            let (fx_l, fx_r) = self.process_momentary_fx_target(
                MomentaryFxTarget::FxBus { index: bus_idx },
                processed,
                processed,
            );
            processed = (fx_l + fx_r) * 0.5;
            let (gl, gr) = if let Some(pos) = pan_override {
                pan_gains_float(pos)
            } else {
                let pan = self.bus_pan_pos.get(bus_idx).copied().unwrap_or(0);
                pan_gains(pan, self.pan_positions)
            };
            left += processed * gl;
            right += processed * gr;
        }

        self.dry_history[self.dry_history_pos] = left;
        self.dry_history[self.dry_history_pos + 1] = right;
        self.dry_history_pos += 2;
        if self.dry_history_pos >= self.dry_history.len() {
            self.dry_history_pos = 0;
        }

        for slot_idx in 0..self.master_slot_params.len() {
            let params = self.master_slot_params[slot_idx];
            if let Some(state) = self.master_slot_state.get_mut(slot_idx) {
                (left, right) =
                    process_master_fx_slot(&params, state, left, right, self.sample_rate);
            }
        }

        let (left, right) =
            self.process_momentary_fx_target(MomentaryFxTarget::Global, left, right);
        self.sample_clock = self.sample_clock.saturating_add(1);
        (
            (left * self.master_volume).clamp(-1.0, 1.0),
            (right * self.master_volume).clamp(-1.0, 1.0),
        )
    }

    fn process_momentary_fx_target(
        &mut self,
        target: MomentaryFxTarget,
        left: f32,
        right: f32,
    ) -> (f32, f32) {
        let sample_rate = self.sample_rate;
        let mut l = left;
        let mut r = right;
        for fx in self.momentary_fx.iter_mut() {
            if fx.target != target {
                continue;
            }
            match fx.kind {
                MomentaryFxKind::Stutter => {
                    let rate = param_f32(&fx.params, "rateHz", 8.0).clamp(1.0, 32.0);
                    let depth = (param_f32(&fx.params, "depthPct", 100.0) / 100.0).clamp(0.0, 1.0);
                    let segment_len =
                        ((sample_rate as f32 / rate) as usize).clamp(48, sample_rate as usize);
                    let ramp_len = fx.stutter_ramp_len.min(segment_len / 4).max(1);

                    if fx.stutter_l.len() != segment_len {
                        fx.stutter_l = vec![0.0; segment_len];
                        fx.stutter_r = vec![0.0; segment_len];
                        fx.stutter_write = 0;
                        fx.stutter_ready = false;
                        fx.stutter_ramp_pos = 0;
                    }

                    if !fx.stutter_ready {
                        fx.stutter_l[fx.stutter_write] = l;
                        fx.stutter_r[fx.stutter_write] = r;
                        fx.stutter_write += 1;
                        if fx.stutter_write >= segment_len {
                            fx.stutter_ready = true;
                            fx.stutter_write = 0;
                            fx.stutter_ramp_pos = 0;
                        }
                    } else {
                        let read = fx.stutter_write;
                        let mut wet_l = fx.stutter_l[read];
                        let mut wet_r = fx.stutter_r[read];

                        let eff_wet = if fx.stutter_ramp_pos < ramp_len {
                            let ramp = fx.stutter_ramp_pos as f32 / ramp_len as f32;
                            fx.stutter_ramp_pos += 1;
                            depth * ramp
                        } else {
                            depth
                        };

                        if read < ramp_len {
                            let fade_in = read as f32 / ramp_len as f32;
                            let end_read = segment_len - ramp_len + read;
                            wet_l = wet_l * fade_in + fx.stutter_l[end_read] * (1.0 - fade_in);
                            wet_r = wet_r * fade_in + fx.stutter_r[end_read] * (1.0 - fade_in);
                        }

                        l = l * (1.0 - eff_wet) + wet_l * eff_wet;
                        r = r * (1.0 - eff_wet) + wet_r * eff_wet;

                        fx.stutter_write += 1;
                        if fx.stutter_write >= segment_len {
                            fx.stutter_write = 0;
                        }
                    }
                }
                MomentaryFxKind::Freeze => {
                    let mix = (param_f32(&fx.params, "mixPct", 100.0) / 100.0).clamp(0.0, 1.0);
                    let feedback = 0.997_f32;
                    let damp = 0.35_f32;

                    if fx.releasing {
                        let total = fx.release_len.max(1) as f32;
                        let fade = 1.0 - (fx.release_pos as f32 / total);
                        fx.release_pos += 1;

                        let mut wet_l = 0.0_f32;
                        let mut wet_r = 0.0_f32;
                        for i in 0..4 {
                            let delayed = fx.freeze_bufs[i][fx.freeze_idxs[i]];
                            fx.freeze_lp[i] = delayed * (1.0 - damp) + fx.freeze_lp[i] * damp;
                            fx.freeze_bufs[i][fx.freeze_idxs[i]] = fx.freeze_lp[i] * feedback;
                            fx.freeze_idxs[i] = (fx.freeze_idxs[i] + 1) % fx.freeze_bufs[i].len();
                            if i < 2 {
                                wet_l += delayed;
                            } else {
                                wet_r += delayed;
                            }
                        }
                        wet_l *= 0.5;
                        wet_r *= 0.5;
                        l = l * (1.0 - mix * fade) + wet_l * mix;
                        r = r * (1.0 - mix * fade) + wet_r * mix;
                    } else {
                        let injecting = fx.freeze_inject_pos < fx.freeze_inject_len;
                        let inject_gain = if injecting { 1.0 } else { 0.0 };
                        if injecting {
                            fx.freeze_inject_pos += 1;
                        }

                        let mut wet_l = 0.0_f32;
                        let mut wet_r = 0.0_f32;
                        for i in 0..4 {
                            let delayed = fx.freeze_bufs[i][fx.freeze_idxs[i]];
                            fx.freeze_lp[i] = delayed * (1.0 - damp) + fx.freeze_lp[i] * damp;
                            let channel_in = if i < 2 { l } else { r };
                            fx.freeze_bufs[i][fx.freeze_idxs[i]] =
                                channel_in * inject_gain + fx.freeze_lp[i] * feedback;
                            fx.freeze_idxs[i] = (fx.freeze_idxs[i] + 1) % fx.freeze_bufs[i].len();
                            if i < 2 {
                                wet_l += delayed;
                            } else {
                                wet_r += delayed;
                            }
                        }
                        wet_l *= 0.5;
                        wet_r *= 0.5;
                        l = l * (1.0 - mix) + wet_l * mix;
                        r = r * (1.0 - mix) + wet_r * mix;
                    }
                }
                MomentaryFxKind::FilterSweep => {
                    let cutoff_pct =
                        (param_f32(&fx.params, "cutoffPct", 35.0) / 100.0).clamp(0.0, 1.0);
                    let resonance_pct =
                        (param_f32(&fx.params, "resonancePct", 70.0) / 100.0).clamp(0.0, 1.0);
                    let q = 0.5 + resonance_pct * 11.5;
                    let target_cutoff = 120.0 + cutoff_pct * 8_000.0;

                    if fx.releasing {
                        let out_len =
                            ms_to_samples(param_f32(&fx.params, "sweepOutMs", 500.0), sample_rate)
                                .max(1) as f32;
                        fx.sweep_pos -= 1.0 / out_len;
                        if fx.sweep_pos < 0.0 {
                            fx.sweep_pos = 0.0;
                        }
                    } else {
                        let in_len =
                            ms_to_samples(param_f32(&fx.params, "sweepInMs", 200.0), sample_rate)
                                .max(1) as f32;
                        fx.sweep_pos += 1.0 / in_len;
                        if fx.sweep_pos > 1.0 {
                            fx.sweep_pos = 1.0;
                        }
                    }

                    let cutoff = 20_000.0 + (target_cutoff - 20_000.0) * fx.sweep_pos;
                    l = fx
                        .filt_l
                        .process(l, FilterType::Lowpass, cutoff, q, sample_rate);
                    r = fx
                        .filt_r
                        .process(r, FilterType::Lowpass, cutoff, q, sample_rate);
                }
                MomentaryFxKind::PitchShift => {
                    let semitones = param_f32(&fx.params, "semitones", 7.0).clamp(-24.0, 24.0);
                    let cents = param_f32(&fx.params, "cents", 0.0).clamp(-100.0, 100.0);
                    let mix = (param_f32(&fx.params, "mixPct", 100.0) / 100.0).clamp(0.0, 1.0);
                    let total_semitones = semitones + cents / 100.0;
                    let ratio = 2.0_f32.powf(total_semitones / 12.0);

                    let (wet_l, wet_r) = fx.pitch_shifter.process_frame(l, r, ratio);
                    let ramp = if fx.pitch_ramp_pos < fx.pitch_ramp_len {
                        let r = fx.pitch_ramp_pos as f32 / fx.pitch_ramp_len as f32;
                        fx.pitch_ramp_pos += 1;
                        r
                    } else {
                        1.0
                    };
                    let wet_mix = mix * ramp;
                    l = l * (1.0 - wet_mix) + wet_l * wet_mix;
                    r = r * (1.0 - wet_mix) + wet_r * wet_mix;
                }
            }
        }

        self.momentary_fx.retain(|fx| {
            if !fx.releasing {
                return true;
            }
            match fx.kind {
                MomentaryFxKind::FilterSweep => fx.sweep_pos > 0.0,
                MomentaryFxKind::Freeze => {
                    let total =
                        ms_to_samples(param_f32(&fx.params, "releaseMs", 500.0), sample_rate);
                    fx.release_pos < total
                }
                _ => false,
            }
        });

        (l, r)
    }

    fn sample_note_on(&mut self, slot: usize, midi_note: u8, velocity: u8) {
        let sample_slot = sample_slot_for_note(midi_note);
        let Some(bank) = self.sample_banks.get(slot) else {
            return;
        };
        let Some(Some(buffer)) = bank.slots.get(sample_slot).map(|s| s.buffer.as_ref()) else {
            return;
        };
        if buffer.samples.is_empty() || buffer.channels == 0 || buffer.sample_rate == 0 {
            return;
        }
        let vel = (velocity.max(1) as f32 / 127.0).clamp(0.0, 1.0);
        let vel_sens = (bank.velocity_sensitivity_pct / 100.0).clamp(0.0, 1.0);
        let gain = (bank.gain_pct / 100.0).clamp(0.0, 2.0) * ((1.0 - vel_sens) + vel_sens * vel);
        let pitch = 2.0_f32.powf(bank.tune_semis / 12.0);
        let step = pitch * buffer.sample_rate as f32 / self.sample_rate as f32;
        let pool = &mut self.sample_voices[slot];
        let voice_index = match pool.iter().position(|voice| !voice.active) {
            Some(i) => i,
            None => {
                self.voice_steal_since_status = true;
                0
            }
        };
        pool[voice_index] = SampleVoice {
            active: true,
            sample_slot,
            pos: 0.0,
            step,
            gain,
        };
    }

    fn render_sample_voices(&mut self, slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT]) {
        for (slot, out) in slot_out.iter_mut().enumerate().take(INSTRUMENT_SLOT_COUNT) {
            let Some(bank) = self.sample_banks.get(slot) else {
                continue;
            };
            for voice in self.sample_voices[slot].iter_mut() {
                if !voice.active {
                    continue;
                }
                let Some(Some(buffer)) =
                    bank.slots.get(voice.sample_slot).map(|s| s.buffer.as_ref())
                else {
                    voice.active = false;
                    continue;
                };
                let frames = buffer.samples.len() / buffer.channels as usize;
                if frames == 0 || voice.pos >= frames as f32 {
                    voice.active = false;
                    continue;
                }
                let frame = voice.pos.floor() as usize;
                let frac = voice.pos - frame as f32;
                let next_frame = (frame + 1).min(frames - 1);
                let sample = mono_frame(buffer, frame) * (1.0 - frac)
                    + mono_frame(buffer, next_frame) * frac;
                *out += sample * voice.gain;
                voice.pos += voice.step;
            }
        }
    }

    fn render_preview_sample_voices(&mut self, slot_out: &mut [f32; INSTRUMENT_SLOT_COUNT]) {
        for voice in self.preview_sample_voices.iter_mut() {
            let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
            if frames == 0 || voice.pos >= frames as f32 {
                voice.pos = frames as f32;
                continue;
            }
            let frame = voice.pos.floor() as usize;
            let frac = voice.pos - frame as f32;
            let next_frame = (frame + 1).min(frames - 1);
            let sample = mono_frame(&voice.buffer, frame) * (1.0 - frac)
                + mono_frame(&voice.buffer, next_frame) * frac;
            slot_out[voice.slot] += sample * voice.gain;
            voice.pos += voice.step;
        }
        self.preview_sample_voices.retain(|voice| {
            let frames = voice.buffer.samples.len() / voice.buffer.channels as usize;
            frames > 0 && voice.pos < frames as f32
        });
    }

    fn steal_voice_index(pool: &[Voice; VOICES_PER_SLOT]) -> usize {
        let mut best_i = 0;
        let mut best_score = f32::MAX;
        for (i, v) in pool.iter().enumerate() {
            if !v.active {
                return i;
            }
            let score = v.amp_env.level;
            if score < best_score {
                best_score = score;
                best_i = i;
            }
        }
        best_i
    }

    fn active_voice_total(&self) -> usize {
        self.voices
            .iter()
            .map(|pool| pool.iter().filter(|v| v.active).count())
            .sum()
    }

    fn global_voice_budget(&self) -> usize {
        let max_voices = INSTRUMENT_SLOT_COUNT * VOICES_PER_SLOT;
        let (target_load, min_budget_pct) = match self.voice_stealing_mode {
            VoiceStealingMode::Off => return max_voices,
            VoiceStealingMode::Lenient => (0.88_f32, 0.75_f32),
            VoiceStealingMode::Balanced => (0.78_f32, 0.60_f32),
            VoiceStealingMode::Aggressive => (0.68_f32, 0.45_f32),
        };
        if self.smoothed_load_ratio <= target_load {
            return max_voices;
        }
        let severity =
            ((self.smoothed_load_ratio - target_load) / (1.20_f32 - target_load)).clamp(0.0, 1.0);
        let min_budget = ((max_voices as f32) * min_budget_pct).round() as usize;
        let budget =
            (max_voices as f32 - severity * ((max_voices - min_budget) as f32)).round() as usize;
        budget.clamp(min_budget.max(1), max_voices)
    }

    fn enforce_global_voice_budget(&mut self) {
        if self.voice_stealing_mode == VoiceStealingMode::Off {
            return;
        }
        let budget = self.global_voice_budget();
        while self.active_voice_total() > budget {
            let Some((slot, idx)) = self.find_global_steal_candidate() else {
                break;
            };
            self.voices[slot][idx].active = false;
            self.voice_steal_since_status = true;
        }
    }

    fn find_global_steal_candidate(&self) -> Option<(usize, usize)> {
        let mut best: Option<(usize, usize, f32)> = None;
        for (slot_idx, pool) in self.voices.iter().enumerate() {
            for (voice_idx, voice) in pool.iter().enumerate() {
                if !voice.active {
                    continue;
                }
                let age_samples = self.sample_clock.saturating_sub(voice.started_sample);
                let age_ms = (age_samples as f32) * 1000.0 / (self.sample_rate as f32);
                let mut score = voice.amp_env.level;
                if voice.amp_env.is_releasing() {
                    score -= 0.5;
                }
                score += (voice.velocity as f32 / 127.0) * 0.2;
                if age_ms < 30.0 {
                    score += 1.0;
                }
                match best {
                    Some((_, _, best_score)) if score >= best_score => {}
                    _ => best = Some((slot_idx, voice_idx, score)),
                }
            }
        }
        best.map(|(s, i, _)| (s, i))
    }

    #[cfg(test)]
    pub(super) fn active_voice_count_for_slot(&self, slot: usize) -> usize {
        self.voices[slot].iter().filter(|v| v.active).count()
    }

    #[cfg(test)]
    pub(super) fn mod_values_for_slot(&self, slot: usize) -> (f32, f32) {
        let s = slot.min(INSTRUMENT_SLOT_COUNT - 1);
        (self.mods[s].cutoff_cc, self.mods[s].resonance_cc)
    }

    #[cfg(test)]
    pub(super) fn delay_state_probe(&self, bus: usize, slot: usize) -> Option<(usize, f32)> {
        match self.bus_slot_state.get(bus)?.get(slot)? {
            FxBusState::Delay { buf, idx } => Some((*idx, buf.iter().map(|v| v.abs()).sum())),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn master_compressor_env_probe(&self, slot: usize) -> Option<f32> {
        match self.master_slot_state.get(slot)? {
            MasterFxState::Compressor { env } => Some(*env),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn pitch_buf_probe(&self, id: &str) -> Option<usize> {
        for fx in &self.momentary_fx {
            if fx.id == id && matches!(fx.kind, MomentaryFxKind::PitchShift) {
                return Some(fx.pitch_shifter.write_pos);
            }
        }
        None
    }

    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub(super) fn stutter_buf_for_id(
        &self,
        id: &str,
    ) -> Option<(Vec<f32>, Vec<f32>, usize, bool, usize)> {
        for fx in &self.momentary_fx {
            if fx.id == id && matches!(fx.kind, MomentaryFxKind::Stutter) {
                return Some((
                    fx.stutter_l.clone(),
                    fx.stutter_r.clone(),
                    fx.stutter_write,
                    fx.stutter_ready,
                    fx.stutter_ramp_pos,
                ));
            }
        }
        None
    }
}

fn parse_route(route: &str) -> usize {
    if route == "direct" {
        return 0;
    }
    if let Some(rest) = route
        .strip_prefix("fx_bus_")
        .or_else(|| route.strip_prefix("bus_"))
    {
        if let Ok(n) = rest.parse::<usize>() {
            if n >= 1 {
                return n;
            }
        }
    }
    0
}

fn parse_instrument_kind(kind: &str) -> InstrumentKind {
    match kind {
        "sampler" => InstrumentKind::Sample,
        "midi" => InstrumentKind::Midi,
        "none" => InstrumentKind::None,
        _ => InstrumentKind::Synth,
    }
}

fn parse_momentary_fx_kind(kind: &str) -> Option<MomentaryFxKind> {
    match kind {
        "stutter" => Some(MomentaryFxKind::Stutter),
        "freeze" => Some(MomentaryFxKind::Freeze),
        "filter_sweep" => Some(MomentaryFxKind::FilterSweep),
        "pitch_shift" => Some(MomentaryFxKind::PitchShift),
        _ => None,
    }
}

fn param_f32(params: &BTreeMap<String, Value>, key: &str, fallback: f32) -> f32 {
    params
        .get(key)
        .and_then(Value::as_f64)
        .map(|value| value as f32)
        .filter(|value| value.is_finite())
        .unwrap_or(fallback)
}

fn sample_slot_for_note(note: u8) -> usize {
    note.saturating_sub(36)
        .min((SAMPLE_SLOTS_PER_INSTRUMENT - 1) as u8) as usize
}

fn mono_frame(buffer: &SampleBuffer, frame: usize) -> f32 {
    let channels = buffer.channels.max(1) as usize;
    let base = frame.saturating_mul(channels);
    if channels == 1 {
        return buffer.samples.get(base).copied().unwrap_or(0.0);
    }
    let left = buffer.samples.get(base).copied().unwrap_or(0.0);
    let right = buffer.samples.get(base + 1).copied().unwrap_or(left);
    (left + right) * 0.5
}

fn pan_gains(pan_pos: usize, positions: usize) -> (f32, f32) {
    if positions <= 1 {
        return (0.70710677, 0.70710677);
    }
    let t = (pan_pos.min(positions - 1) as f32) / ((positions - 1) as f32);
    let theta = t * (std::f32::consts::FRAC_PI_2);
    (theta.cos(), theta.sin())
}

fn pan_gains_float(pos: f32) -> (f32, f32) {
    let theta = pos.clamp(0.0, 1.0) * std::f32::consts::FRAC_PI_2;
    (theta.cos(), theta.sin())
}

fn midi_note_to_hz(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn osc_sample(cfg: OscConfig, base_freq: f32, phase: &mut f32, sample_rate: u32) -> f32 {
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
