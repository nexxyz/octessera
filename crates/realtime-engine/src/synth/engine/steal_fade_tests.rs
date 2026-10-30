use super::*;
use crate::synth::steal_declick::STEAL_FADE_FRAMES;

const RATE: u32 = 48_000;

fn steady_slot(kind: &str) -> InstrumentSlotConfig {
    let mut synth = default_synth_config();
    synth.amp_env.attack_ms = 0.0;
    synth.amp_env.decay_ms = 0.0;
    synth.amp_env.sustain_pct = 100.0;
    synth.osc1.waveform = WaveformId::Sine;
    synth.osc2.level_pct = 0.0;
    InstrumentSlotConfig {
        fm: None,
        pluck: None,
        drum: None,
        kind: kind.into(),
        synth,
        mixer: None,
    }
}

/// Slot 0 holds a full set of steady voices: a low sine for synth, a flat level for sampler.
fn steady_engine(kind: &str, slots: usize) -> SynthEngine {
    let mut engine = SynthEngine::new(RATE);
    for slot in 0..slots {
        engine.set_instrument_slot(slot, steady_slot(kind));
    }
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: vec![0.1; RATE as usize].into(),
            channels: 1,
            sample_rate: RATE,
        }),
    };
    engine.set_sample_banks(vec![bank; INSTRUMENT_SLOT_COUNT]);
    for slot in 0..slots {
        for _ in 0..MAX_SYNTH_VOICES_PER_SLOT {
            engine.note_on(slot as u8, 0, 100, 50_000);
        }
    }
    engine
}

fn render(engine: &mut SynthEngine, frames: usize, block: bool) -> Vec<f32> {
    if block {
        let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
        engine.render_interleaved_block(frames, &mut left, &mut right, &mut out);
        left
    } else {
        (0..frames).map(|_| engine.next_stereo_sample().0).collect()
    }
}

/// The last steady sample and the largest step the steady voices make on their own.
fn warm_up(engine: &mut SynthEngine, block: bool) -> (f32, f32) {
    let warm = render(engine, 4_800, block);
    let tail = &warm[warm.len() - 256..];
    (tail[255], max_step(tail[0], &tail[1..]))
}

/// The largest sample-to-sample step from the last pre-steal frame through `after`.
fn max_step(before: f32, after: &[f32]) -> f32 {
    let mut previous = before;
    after.iter().fold(0.0_f32, |max, sample| {
        let step = (sample - previous).abs();
        previous = *sample;
        max.max(step)
    })
}

fn sounding_synth_voices(engine: &SynthEngine, slot: usize) -> usize {
    engine.synth_voice_pool.sounding_count_for_slot(slot)
}

#[test]
fn a_stolen_synth_voice_fades_in_its_lane_without_a_step_then_frees_it() {
    for block in [false, true] {
        let mut engine = steady_engine("synth", 1);
        let (before, natural) = warm_up(&mut engine, block);
        let one_voice = before.abs() / MAX_SYNTH_VOICES_PER_SLOT as f32;
        assert!(one_voice > 0.005, "block={block}: steady level {before}");

        engine.note_on(0, 0, 100, 50_000);
        #[cfg(feature = "profiling")]
        assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
        assert_eq!(
            engine.active_voice_count_for_slot(0),
            MAX_SYNTH_VOICES_PER_SLOT + 1
        );
        assert_eq!(sounding_synth_voices(&engine, 0), MAX_SYNTH_VOICES_PER_SLOT);
        engine.assert_voice_pool_invariants();

        let after = render(&mut engine, usize::from(STEAL_FADE_FRAMES) * 2, block);
        let step = max_step(before, &after);
        // A hard cut steps by the stolen voice's whole share in one sample.
        assert!(
            step < natural + one_voice * 0.05,
            "block={block}: step {step} against a voice of {one_voice}"
        );
        assert_eq!(
            engine.active_voice_count_for_slot(0),
            MAX_SYNTH_VOICES_PER_SLOT
        );
        engine.assert_voice_pool_invariants();
    }
}

#[test]
fn a_stolen_sample_voice_fades_while_the_new_note_sounds_on_its_first_frame() {
    for block in [false, true] {
        let mut engine = steady_engine("sampler", 1);
        let (before, natural) = warm_up(&mut engine, block);
        let one_voice = before / MAX_SAMPLE_VOICES_PER_SLOT as f32;
        assert!(one_voice > 0.005, "block={block}: steady level {before}");

        engine.note_on(0, 0, 100, 50_000);
        #[cfg(feature = "profiling")]
        assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
        assert_eq!(
            engine.active_sample_voice_count_for_slot(0),
            MAX_SAMPLE_VOICES_PER_SLOT + 1
        );
        let after = render(&mut engine, usize::from(STEAL_FADE_FRAMES) * 2, block);
        assert!(
            after[0] > before + one_voice * 0.9,
            "block={block}: the new note is late: {before} -> {}",
            after[0]
        );
        let step = max_step(after[0], &after[1..]);
        assert!(
            step < natural + one_voice * 0.05,
            "block={block}: step {step} against a voice of {one_voice}"
        );
        assert!((after[after.len() - 1] - before).abs() < one_voice * 0.05);
        assert_eq!(natural, 0.0);
        assert_eq!(
            engine.active_sample_voice_count_for_slot(0),
            MAX_SAMPLE_VOICES_PER_SLOT
        );
        engine.assert_voice_pool_invariants();
    }
}

#[test]
fn a_global_budget_cut_fades_instead_of_cutting_and_counts_until_silent() {
    for block in [false, true] {
        let mut engine = steady_engine("synth", 1);
        engine.set_instrument_slot(1, steady_slot("synth"));
        engine.set_voice_stealing_mode(VoiceStealingMode::Fixed12);
        for _ in 0..4 {
            engine.note_on(1, 0, 100, 50_000);
        }
        engine.render_through_steal_fades();
        assert_eq!(engine.active_voice_count_for_slot(0), 8);
        assert_eq!(engine.active_voice_count_for_slot(1), 4);
        #[cfg(feature = "profiling")]
        let steals = engine.profile_snapshot().cumulative_voice_steals;
        let (before, natural) = warm_up(&mut engine, block);
        let one_voice = before.abs() / 12.0;
        assert!(one_voice > 0.005, "block={block}: steady level {before}");

        engine.note_on(1, 0, 100, 50_000);
        #[cfg(feature = "profiling")]
        assert_eq!(
            engine.profile_snapshot().cumulative_voice_steals,
            steals + 1
        );
        assert_eq!(engine.active_voice_count_for_slot(0), 8);
        assert_eq!(sounding_synth_voices(&engine, 0), 7);
        let after = render(&mut engine, usize::from(STEAL_FADE_FRAMES) * 2, block);
        let step = max_step(before, &after);
        assert!(
            step < natural + one_voice * 0.05,
            "block={block}: step {step} against a voice of {one_voice}"
        );
        assert_eq!(engine.active_voice_count_for_slot(0), 7);
        assert_eq!(engine.active_voice_count_for_slot(1), 5);
        engine.assert_voice_pool_invariants();
    }
}

#[test]
fn a_full_pool_steal_takes_the_lane_and_ramps_from_the_old_output() {
    for block in [false, true] {
        let mut engine = steady_engine("synth", INSTRUMENT_SLOT_COUNT);
        assert_eq!(
            engine.profile_snapshot().active_synth_voices,
            SYNTH_VOICE_LANE_CAPACITY
        );
        let (before, natural) = warm_up(&mut engine, block);
        let one_voice = before.abs() / SYNTH_VOICE_LANE_CAPACITY as f32;
        assert!(one_voice > 0.005, "block={block}: steady level {before}");

        engine.note_on(0, 0, 100, 50_000);
        #[cfg(feature = "profiling")]
        assert_eq!(engine.profile_snapshot().cumulative_voice_steals, 1);
        assert_eq!(
            engine.profile_snapshot().active_synth_voices,
            SYNTH_VOICE_LANE_CAPACITY
        );
        let after = render(&mut engine, usize::from(STEAL_FADE_FRAMES) * 2, block);
        let step = max_step(before, &after);
        assert!(
            step < natural + one_voice * 0.05,
            "block={block}: step {step} against a voice of {one_voice}"
        );
        engine.assert_voice_pool_invariants();
    }
}
