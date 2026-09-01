use super::super::runtime_state::{BiquadState, InstrumentMod, Voice};
use super::super::synth_voice_pool::SynthVoicePool;
use super::super::types::{
    FilterType, SampleBankConfig, SynthConfig, INSTRUMENT_SLOT_COUNT, SAMPLE_VOICE_LANE_CAPACITY,
    SYNTH_VOICE_LANE_CAPACITY,
};
use super::render_voice::{
    refresh_synth_voice_render_cache, render_synth_voice_sample_precomputed, SynthVoiceRenderConfig,
};
use super::sample_voice_pool::SampleVoicePool;
use super::support::{mono_frame, SampleVoice};
use super::BLOCK_SLOT_SCRATCH_FRAMES;

pub(super) struct InlineSourceExecutor {
    synth_samples: [Vec<f32>; SYNTH_VOICE_LANE_CAPACITY],
    synth_active: [Vec<bool>; SYNTH_VOICE_LANE_CAPACITY],
    synth_slots: [u8; SYNTH_VOICE_LANE_CAPACITY],
    sample_samples: [Vec<f32>; SAMPLE_VOICE_LANE_CAPACITY],
    sample_active: [Vec<bool>; SAMPLE_VOICE_LANE_CAPACITY],
    sample_slots: [u8; SAMPLE_VOICE_LANE_CAPACITY],
}

impl InlineSourceExecutor {
    pub(super) fn new() -> Self {
        Self {
            synth_samples: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            synth_slots: [0; SYNTH_VOICE_LANE_CAPACITY],
            sample_samples: std::array::from_fn(|_| vec![0.0; BLOCK_SLOT_SCRATCH_FRAMES]),
            sample_active: std::array::from_fn(|_| vec![false; BLOCK_SLOT_SCRATCH_FRAMES]),
            sample_slots: [0; SAMPLE_VOICE_LANE_CAPACITY],
        }
    }

    pub(super) fn prepare(&mut self, frames: usize) -> bool {
        if frames > BLOCK_SLOT_SCRATCH_FRAMES {
            return false;
        }
        for samples in &mut self.synth_samples {
            samples[..frames].fill(0.0);
        }
        for active in &mut self.synth_active {
            active[..frames].fill(false);
        }
        for samples in &mut self.sample_samples {
            samples[..frames].fill(0.0);
        }
        for active in &mut self.sample_active {
            active[..frames].fill(false);
        }
        true
    }

    pub(super) fn render_synth_sources(
        &mut self,
        frames: usize,
        base_sample_clock: u64,
        pool: &mut SynthVoicePool,
        context: SynthSourceContext<'_>,
        output: SourceRenderOutput<'_>,
    ) {
        for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
            let slot = {
                let voice = pool.lane(lane);
                if !voice.active {
                    continue;
                }
                voice.instrument_slot as usize
            };
            if slot >= INSTRUMENT_SLOT_COUNT {
                continue;
            }
            self.synth_slots[lane] = slot as u8;
            let frame_context = SynthVoiceFrameContext {
                sample_rate: context.sample_rate,
                config: context.configs[slot],
                render_config: &context.render_configs[slot],
                revision: context.revisions[slot],
                mods: context.mods[slot],
            };
            let voice = pool.lane_mut(lane);
            for frame in 0..frames {
                if let Some(sample) = render_synth_voice_frame(
                    voice,
                    slot,
                    base_sample_clock.saturating_add(frame as u64),
                    frame_context,
                ) {
                    self.synth_samples[lane][frame] = sample;
                    self.synth_active[lane][frame] = true;
                }
            }
        }
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            pool.compact_slot_lanes(slot);
            *active = false;
        }
        self.reduce_synth_sources(
            frames,
            &mut *output.slot_out,
            &mut *output.slot_active,
            &mut *output.active_slots,
        );
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            *active = pool.active_count_for_slot(slot) > 0;
        }
    }

    pub(super) fn render_sample_sources(
        &mut self,
        frames: usize,
        pool: &mut SampleVoicePool,
        context: SampleSourceContext<'_>,
        output: SourceRenderOutput<'_>,
    ) {
        for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
            let slot = {
                let voice = pool.lane(lane);
                if !voice.active {
                    continue;
                }
                voice.instrument_slot as usize
            };
            if slot >= INSTRUMENT_SLOT_COUNT {
                continue;
            }
            self.sample_slots[lane] = slot as u8;
            let Some(bank) = context.banks.get(slot) else {
                continue;
            };
            let voice = pool.lane_mut(lane);
            for frame in 0..frames {
                if let Some(sample) = render_sample_voice_frame(voice, bank, context.sample_rate) {
                    self.sample_samples[lane][frame] = sample;
                    self.sample_active[lane][frame] = true;
                }
            }
        }
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            pool.compact_slot_lanes(slot);
            *active = false;
        }
        self.reduce_sample_sources(
            frames,
            &mut *output.slot_out,
            &mut *output.slot_active,
            &mut *output.active_slots,
        );
        for (slot, active) in output.active_slots.iter_mut().enumerate() {
            *active = pool.active_count_for_slot(slot) > 0 && context.banks.get(slot).is_some();
        }
    }

    fn reduce_synth_sources(
        &self,
        frames: usize,
        slot_out: &mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
        slot_active: &mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
        active_slots: &mut [bool; INSTRUMENT_SLOT_COUNT],
    ) {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            for lane in 0..SYNTH_VOICE_LANE_CAPACITY {
                if self.synth_slots[lane] as usize != slot {
                    continue;
                }
                for frame in 0..frames {
                    slot_out[slot][frame] += self.synth_samples[lane][frame];
                    slot_active[slot][frame] |= self.synth_active[lane][frame];
                    active_slots[slot] |= self.synth_active[lane][frame];
                }
            }
        }
    }

    fn reduce_sample_sources(
        &self,
        frames: usize,
        slot_out: &mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
        slot_active: &mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
        active_slots: &mut [bool; INSTRUMENT_SLOT_COUNT],
    ) {
        for slot in 0..INSTRUMENT_SLOT_COUNT {
            for lane in 0..SAMPLE_VOICE_LANE_CAPACITY {
                if self.sample_slots[lane] as usize != slot {
                    continue;
                }
                for frame in 0..frames {
                    slot_out[slot][frame] += self.sample_samples[lane][frame];
                    slot_active[slot][frame] |= self.sample_active[lane][frame];
                    active_slots[slot] |= self.sample_active[lane][frame];
                }
            }
        }
    }
}

pub(super) struct SynthSourceContext<'a> {
    pub sample_rate: u32,
    pub configs: &'a [SynthConfig; INSTRUMENT_SLOT_COUNT],
    pub render_configs: &'a [SynthVoiceRenderConfig; INSTRUMENT_SLOT_COUNT],
    pub revisions: &'a [u32; INSTRUMENT_SLOT_COUNT],
    pub mods: &'a [InstrumentMod; INSTRUMENT_SLOT_COUNT],
}

pub(super) struct SampleSourceContext<'a> {
    pub sample_rate: u32,
    pub banks: &'a [SampleBankConfig],
}

pub(super) struct SourceRenderOutput<'a> {
    pub slot_out: &'a mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub slot_active: &'a mut [Vec<bool>; INSTRUMENT_SLOT_COUNT],
    pub active_slots: &'a mut [bool; INSTRUMENT_SLOT_COUNT],
}

#[derive(Clone, Copy)]
pub(super) struct SynthVoiceFrameContext<'a> {
    pub sample_rate: u32,
    pub config: SynthConfig,
    pub render_config: &'a SynthVoiceRenderConfig,
    pub revision: u32,
    pub mods: InstrumentMod,
}

pub(super) fn render_synth_voice_frame(
    voice: &mut Voice,
    slot_idx: usize,
    frame_sample_clock: u64,
    context: SynthVoiceFrameContext<'_>,
) -> Option<f32> {
    if !voice.active {
        return None;
    }
    debug_assert_eq!(voice.instrument_slot as usize, slot_idx);
    if frame_sample_clock >= voice.note_off_sample {
        voice
            .amp_env
            .begin_release(context.config.amp_env, context.sample_rate);
        voice
            .filt_env
            .begin_release(context.config.filter_env, context.sample_rate);
    }
    let amp_env = voice.amp_env.next();
    let filt_env = voice.filt_env.next();
    if voice.amp_env.is_off() {
        voice.active = false;
        return None;
    }
    if voice.render_revision != context.revision {
        refresh_synth_voice_render_cache(
            voice,
            context.render_config,
            context.sample_rate,
            context.revision,
        );
    }
    Some(render_synth_voice_sample_precomputed(
        context.sample_rate,
        context.mods,
        context.render_config,
        voice,
        amp_env,
        filt_env,
    ))
}

pub(super) fn render_sample_voice_frame(
    voice: &mut SampleVoice,
    bank: &SampleBankConfig,
    sample_rate: u32,
) -> Option<f32> {
    if !voice.active {
        return None;
    }
    let Some(Some(buffer)) = bank
        .slots
        .get(voice.sample_slot)
        .map(|slot| slot.buffer.as_ref())
    else {
        voice.active = false;
        return None;
    };
    let frames = buffer.samples.len() / buffer.channels as usize;
    if frames == 0 || voice.pos >= frames as f32 {
        voice.active = false;
        return None;
    }
    let frame = voice.pos.floor() as usize;
    let frac = voice.pos - frame as f32;
    let next_frame = (frame + 1).min(frames - 1);
    let sample = mono_frame(buffer, frame) * (1.0 - frac) + mono_frame(buffer, next_frame) * frac;
    let filtered = sample_lowpass(
        sample,
        &mut voice.filt,
        bank.filter_cutoff_hz,
        bank.filter_resonance,
        sample_rate,
    );
    voice.pos += voice.step;
    Some(filtered * voice.gain)
}

pub(super) fn sample_lowpass(
    sample: f32,
    filt: &mut BiquadState,
    cutoff_hz: f32,
    resonance: f32,
    sample_rate: u32,
) -> f32 {
    let q = 0.5 + (resonance.clamp(0.0, 100.0) / 100.0) * 11.5;
    filt.process(sample, FilterType::Lowpass, cutoff_hz, q, sample_rate)
        .clamp(-8.0, 8.0)
}
