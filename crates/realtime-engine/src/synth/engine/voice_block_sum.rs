use super::super::pluck_string::PluckRing;
use super::super::runtime_state::Voice;
use super::super::synth_voice_pool::SynthVoicePartition;
use super::super::types::INSTRUMENT_SLOT_COUNT;
use super::render_voice::synth_voice_render::{render_synth_voice_block, SynthVoiceFrameContext};
use super::sample_voice_pool::SampleVoicePartition;
use super::source_lane_renderer::{render_sample_voice_block, SynthSourceContext};
use super::support::SampleVoice;
#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static RENDERED_VOICES: Cell<[usize; 2]> = const { Cell::new([0; 2]) };
}

#[cfg(test)]
fn record_rendered_voice(index: usize) {
    RENDERED_VOICES.with(|rendered| {
        let mut value = rendered.get();
        value[index] += 1;
        rendered.set(value);
    });
}

/// Synth and sample voices rendered on this thread since the last reset.
#[cfg(test)]
pub(super) fn rendered_voices_for_test() -> [usize; 2] {
    RENDERED_VOICES.with(Cell::get)
}

#[cfg(test)]
pub(super) fn reset_rendered_voices_for_test() {
    RENDERED_VOICES.with(|rendered| rendered.set([0; 2]));
}

/// One block's per-instrument sums. Each voice renders into a reused block and is added to its
/// instrument at once; `rendered_prefixes` keeps each instrument's longest rendered prefix, which
/// marks the frames where that instrument sounded.
pub(super) struct SlotBlockSum<'a> {
    pub(super) slot_out: &'a mut [Vec<f32>; INSTRUMENT_SLOT_COUNT],
    pub(super) rendered_prefixes: [usize; INSTRUMENT_SLOT_COUNT],
}

impl<'a> SlotBlockSum<'a> {
    pub(super) fn new(slot_out: &'a mut [Vec<f32>; INSTRUMENT_SLOT_COUNT]) -> Self {
        Self {
            slot_out,
            rendered_prefixes: [0; INSTRUMENT_SLOT_COUNT],
        }
    }

    fn add(&mut self, slot: usize, voice_block: &[f32]) {
        for (out, sample) in self.slot_out[slot].iter_mut().zip(voice_block) {
            *out += *sample;
        }
        self.rendered_prefixes[slot] = self.rendered_prefixes[slot].max(voice_block.len());
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn longest_prefix(&self) -> usize {
        self.rendered_prefixes.iter().copied().max().unwrap_or(0)
    }
}

pub(super) fn sum_synth_voice(
    voice: &mut Voice,
    ring: &mut PluckRing,
    frames: usize,
    base_sample_clock: u64,
    context: &SynthSourceContext,
    voice_block: &mut [f32],
    sum: &mut SlotBlockSum<'_>,
) {
    let slot = voice.instrument_slot as usize;
    let rendered = render_synth_voice_block(
        voice,
        ring,
        slot,
        frames,
        base_sample_clock,
        SynthVoiceFrameContext {
            sample_rate: context.sample_rate,
            config: context.configs[slot],
            render_config: context.render_configs[slot],
            revision: context.revisions[slot],
            mods: context.mods[slot],
        },
        voice_block,
    );
    #[cfg(test)]
    record_rendered_voice(0);
    sum.add(slot, &voice_block[..rendered]);
}

pub(super) fn sum_sample_voice(
    voice: &mut SampleVoice,
    frames: usize,
    sample_rate: u32,
    voice_block: &mut [f32],
    sum: &mut SlotBlockSum<'_>,
) -> usize {
    let rendered = render_sample_voice_block(voice, frames, sample_rate, voice_block);
    #[cfg(test)]
    record_rendered_voice(1);
    let slot = voice.instrument_slot as usize;
    if slot < INSTRUMENT_SLOT_COUNT {
        sum.add(slot, &voice_block[..rendered]);
    }
    rendered
}

/// Renders a partition's owned synth voices into `sum`, dropping lanes that went silent.
pub(super) fn sum_synth_partition(
    partition: &mut SynthVoicePartition,
    frames: usize,
    base_sample_clock: u64,
    context: &SynthSourceContext,
    voice_block: &mut [f32],
    sum: &mut SlotBlockSum<'_>,
) {
    let mut survivor_count = 0;
    for read in 0..partition.render_lane_count {
        let lane = partition.render_lanes[read];
        let (voice, ring) = partition.lane_and_ring_mut(lane).expect("source lane ring");
        let remains_active = if !voice.active {
            false
        } else if voice.instrument_slot as usize >= INSTRUMENT_SLOT_COUNT {
            true
        } else {
            sum_synth_voice(
                voice,
                ring,
                frames,
                base_sample_clock,
                context,
                voice_block,
                sum,
            );
            voice.active
        };
        if remains_active {
            partition.render_lanes[survivor_count] = lane;
            survivor_count += 1;
        }
    }
    partition.render_lane_count = survivor_count;
}

/// Renders a partition's owned sample voices into `sum`, dropping lanes that went silent.
pub(super) fn sum_sample_partition(
    partition: &mut SampleVoicePartition,
    frames: usize,
    sample_rate: u32,
    voice_block: &mut [f32],
    sum: &mut SlotBlockSum<'_>,
) {
    let mut survivor_count = 0;
    for read in 0..partition.render_lane_count {
        let lane = partition.render_lanes[read];
        let voice = &mut partition.lanes_mut()[lane];
        let remains_active = if !voice.active {
            false
        } else if voice.instrument_slot as usize >= INSTRUMENT_SLOT_COUNT {
            true
        } else {
            sum_sample_voice(voice, frames, sample_rate, voice_block, sum);
            voice.active
        };
        if remains_active {
            partition.render_lanes[survivor_count] = lane;
            survivor_count += 1;
        }
    }
    partition.render_lane_count = survivor_count;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voices_add_into_their_instrument_and_mark_the_longest_rendered_prefix() {
        let mut slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT] = std::array::from_fn(|_| vec![0.0; 4]);
        let mut sum = SlotBlockSum::new(&mut slot_out);
        sum.add(2, &[1.0, 1.0, 1.0, 1.0]);
        sum.add(2, &[0.5, 0.5]);
        sum.add(5, &[]);

        assert_eq!(sum.rendered_prefixes[2], 4);
        assert_eq!(sum.rendered_prefixes[5], 0);
        assert_eq!(slot_out[2], vec![1.5, 1.5, 1.0, 1.0]);
        assert!(slot_out[5].iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn voices_with_an_invalid_instrument_stay_listed_but_add_nothing() {
        let invalid = INSTRUMENT_SLOT_COUNT as u8;
        let mut engine = super::super::SynthEngine::new(48_000);
        assert!(engine.synth_voice_pool.assign_lane(4, 0));
        let synth = engine.synth_voice_pool.lane_mut(4).expect("synth lane");
        synth.instrument_slot = invalid;
        synth.active = true;
        assert!(engine.sample_voice_pool.assign_lane(4, 0));
        let sample = engine.sample_voice_pool.lane_mut(4).expect("sample lane");
        sample.instrument_slot = invalid;
        sample.active = true;
        let mut synth_partition = engine
            .synth_voice_pool
            .take_partition(0)
            .expect("synth home");
        let mut sample_partition = engine
            .sample_voice_pool
            .take_partition(0)
            .expect("sample home");
        let context = engine.synth_source_context();
        let mut slot_out: [Vec<f32>; INSTRUMENT_SLOT_COUNT] =
            std::array::from_fn(|_| vec![0.0; 32]);
        let mut voice_block = vec![0.0; 32];
        let mut sum = SlotBlockSum::new(&mut slot_out);

        sum_synth_partition(
            &mut synth_partition,
            32,
            0,
            &context,
            &mut voice_block,
            &mut sum,
        );
        sum_sample_partition(
            &mut sample_partition,
            32,
            48_000,
            &mut voice_block,
            &mut sum,
        );

        assert_eq!(sum.rendered_prefixes, [0; INSTRUMENT_SLOT_COUNT]);
        assert_eq!(
            &synth_partition.render_lanes[..synth_partition.render_lane_count],
            &[2]
        );
        assert_eq!(
            &sample_partition.render_lanes[..sample_partition.render_lane_count],
            &[2]
        );
    }
}
