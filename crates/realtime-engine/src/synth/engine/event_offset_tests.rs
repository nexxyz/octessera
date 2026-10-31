use super::*;
use crate::synth::{
    InstrumentSlotConfig, InstrumentsConfig, SampleBankConfig, SampleBuffer, SampleSlotConfig,
};

const BLOCK: usize = 128;
const OFFSET: usize = 37;

fn render_blocks(engine: &mut SynthEngine, blocks: usize) -> Vec<f32> {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut rendered = Vec::new();
    for _ in 0..blocks {
        engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
        rendered.extend_from_slice(&out);
    }
    rendered
}

fn render_serial(engine: &mut SynthEngine, frames: usize, rendered: &mut Vec<f32>) {
    for _ in 0..frames {
        let (left, right) = engine.next_stereo_sample();
        rendered.extend([left, right]);
    }
}

fn assert_bits_eq(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
    }
}

fn first_sounding_frame(rendered: &[f32]) -> Option<usize> {
    rendered
        .iter()
        .position(|sample| *sample != 0.0)
        .map(|index| index / 2)
}

fn sampler_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "sampler".to_string(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: (0..1_000)
                .map(|index| 1.0 - index as f32 / 1_000.0)
                .collect(),
            channels: 1,
            sample_rate: 48_000,
        }),
    };
    engine.set_sample_banks(vec![bank]);
    engine
}

#[test]
fn a_synth_note_starts_and_releases_on_its_frame_inside_the_block() {
    let mut offset = SynthEngine::new(48_000);
    offset.with_event_frame_offset(OFFSET as u32, |engine| engine.note_on(0, 60, 100, 60_000));
    let mut rendered = render_blocks(&mut offset, 2);
    offset.with_event_frame_offset(OFFSET as u32, |engine| engine.note_off(0, 60));
    rendered.extend(render_blocks(&mut offset, 4));

    let mut reference = SynthEngine::new(48_000);
    let mut expected = Vec::new();
    render_serial(&mut reference, OFFSET, &mut expected);
    reference.note_on(0, 60, 100, 60_000);
    render_serial(&mut reference, 2 * BLOCK, &mut expected);
    reference.note_off(0, 60);
    render_serial(&mut reference, 4 * BLOCK - OFFSET, &mut expected);

    assert!(first_sounding_frame(&rendered).is_some_and(|frame| frame >= OFFSET));
    assert_bits_eq(&rendered, &expected);
}

#[test]
fn the_frame_path_honours_the_same_synth_onset() {
    let mut offset = SynthEngine::new(48_000);
    offset.with_event_frame_offset(OFFSET as u32, |engine| engine.note_on(0, 64, 90, 60_000));
    let mut rendered = Vec::new();
    render_serial(&mut offset, 2 * BLOCK, &mut rendered);

    let mut reference = SynthEngine::new(48_000);
    let mut expected = Vec::new();
    render_serial(&mut reference, OFFSET, &mut expected);
    reference.note_on(0, 64, 90, 60_000);
    render_serial(&mut reference, 2 * BLOCK - OFFSET, &mut expected);

    assert_bits_eq(&rendered, &expected);
}

#[test]
fn a_sample_voice_waits_for_its_frame_on_the_block_and_frame_paths() {
    let mut block = sampler_engine();
    block.with_event_frame_offset(OFFSET as u32, |engine| engine.note_on(0, 36, 127, 1_000));
    let rendered = render_blocks(&mut block, 5);
    let mut frame = sampler_engine();
    frame.with_event_frame_offset(OFFSET as u32, |engine| engine.note_on(0, 36, 127, 1_000));
    let mut serial = Vec::new();
    render_serial(&mut frame, 5 * BLOCK, &mut serial);

    let mut reference = sampler_engine();
    let mut expected = Vec::new();
    render_serial(&mut reference, OFFSET, &mut expected);
    reference.note_on(0, 36, 127, 1_000);
    render_serial(&mut reference, 5 * BLOCK - OFFSET, &mut expected);

    assert_eq!(first_sounding_frame(&rendered), Some(OFFSET));
    assert_bits_eq(&rendered, &expected);
    assert_bits_eq(&serial, &expected);
}

fn render_sample_release(offset: u32, serial: bool) -> Vec<f32> {
    let mut engine = sampler_engine();
    engine.note_on(0, 36, 127, 60_000);
    let mut rendered = Vec::new();
    let mut render = |engine: &mut SynthEngine, blocks: usize| {
        if serial {
            render_serial(engine, blocks * BLOCK, &mut rendered);
        } else {
            rendered.extend(render_blocks(engine, blocks));
        }
    };
    render(&mut engine, 1);
    engine.with_event_frame_offset(offset, |engine| engine.note_off(0, 36));
    render(&mut engine, 4);
    rendered
}

#[test]
fn a_sample_voice_releases_on_its_frame_on_the_block_and_frame_paths() {
    let mut reference = sampler_engine();
    reference.note_on(0, 36, 127, 60_000);
    let mut expected = Vec::new();
    render_serial(&mut reference, BLOCK + OFFSET, &mut expected);
    reference.note_off(0, 36);
    render_serial(&mut reference, 4 * BLOCK - OFFSET, &mut expected);

    let held = 2 * (BLOCK + OFFSET);
    assert_ne!(expected[held - 2].to_bits(), expected[held].to_bits());
    assert_bits_eq(&render_sample_release(OFFSET as u32, false), &expected);
    assert_bits_eq(&render_sample_release(OFFSET as u32, true), &expected);
}

#[test]
fn offset_zero_matches_an_untimed_event_bit_for_bit() {
    let mut untimed = SynthEngine::new(48_000);
    untimed.note_on(0, 60, 100, 200);
    untimed.cell_hit(0, 0, 0, 0, 100);
    let mut timed = SynthEngine::new(48_000);
    timed.with_event_frame_offset(0, |engine| {
        engine.note_on(0, 60, 100, 200);
        engine.cell_hit(0, 0, 0, 0, 100);
    });
    let mut expected = render_blocks(&mut untimed, 2);
    let mut rendered = render_blocks(&mut timed, 2);
    untimed.note_off(0, 60);
    timed.with_event_frame_offset(0, |engine| engine.note_off(0, 60));
    expected.extend(render_blocks(&mut untimed, 4));
    rendered.extend(render_blocks(&mut timed, 4));

    assert_bits_eq(&rendered, &expected);

    let mut untimed = sampler_engine();
    untimed.note_on(0, 36, 127, 60_000);
    let mut expected = render_blocks(&mut untimed, 1);
    untimed.note_off(0, 36);
    expected.extend(render_blocks(&mut untimed, 4));
    assert_bits_eq(&render_sample_release(0, false), &expected);
}

#[test]
fn stealing_for_an_offset_note_keeps_the_slot_full_and_the_output_continuous() {
    let mut engine = SynthEngine::new(48_000);
    for note in 0..MAX_SYNTH_VOICES_PER_SLOT {
        engine.note_on(0, 48 + note as u8, 100, 60_000);
    }
    let before = render_blocks(&mut engine, 4);
    let steals = engine.profile_snapshot().cumulative_voice_steals;
    engine.with_event_frame_offset(OFFSET as u32, |engine| engine.note_on(0, 84, 100, 60_000));
    let after = render_blocks(&mut engine, 2);

    let snapshot = engine.profile_snapshot();
    assert_eq!(snapshot.cumulative_voice_steals, steals + 1);
    assert_eq!(snapshot.active_synth_voices, MAX_SYNTH_VOICES_PER_SLOT);
    let last = before[before.len() - 2];
    assert!((after[0] - last).abs() < 0.05, "{last} -> {}", after[0]);
    assert!(after.iter().all(|sample| sample.is_finite()));
}

#[cfg(feature = "routing-tree-executor")]
fn render_routing_tree_note(mut engine: SynthEngine, note: u8, offset: u32) -> Vec<f32> {
    use super::{SourceWorkerLifecycle, SourceWorkerRenderDisposition};
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, BLOCK)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let (mut left, mut right, mut out) = (vec![0.0; BLOCK], vec![0.0; BLOCK], vec![0.0; BLOCK * 2]);
    let mut rendered = Vec::new();
    for block in 0..8 {
        let disposition = if block == 1 || block == 4 {
            engine.render_interleaved_block_with_source_runtime_ready_with_controls(
                &mut runtime,
                BLOCK,
                &mut left,
                &mut right,
                &mut out,
                |engine| {
                    engine.with_event_frame_offset(offset, |engine| {
                        if block == 1 {
                            engine.note_on(0, note, 100, 60_000);
                        } else {
                            engine.note_off(0, note);
                        }
                    });
                    Ok(())
                },
            )
        } else {
            engine.render_interleaved_block_with_source_runtime(
                &mut runtime,
                BLOCK,
                &mut left,
                &mut right,
                &mut out,
            )
        };
        assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
        rendered.extend_from_slice(&out);
    }
    let report = lifecycle.shutdown(runtime.retire());
    assert_eq!(report.joined_workers, 2);
    rendered
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_workers_start_and_release_offset_notes_on_their_frame() {
    let on_time = render_routing_tree_note(SynthEngine::new(48_000), 60, 0);
    let offset = render_routing_tree_note(SynthEngine::new(48_000), 60, OFFSET as u32);
    let onset = first_sounding_frame(&on_time).expect("routing-tree note sounds");

    assert_eq!(first_sounding_frame(&offset), Some(onset + OFFSET));
    let shift = OFFSET * 2;
    for (index, expected) in on_time[..on_time.len() - shift].iter().enumerate() {
        let actual = offset[index + shift];
        assert!(
            (actual - expected).abs() < 1.0e-6,
            "sample {index}: {actual} vs {expected}"
        );
    }
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_workers_release_offset_sample_notes_like_the_inline_executor() {
    let mut inline = sampler_engine();
    let mut expected = render_blocks(&mut inline, 1);
    inline.with_event_frame_offset(OFFSET as u32, |engine| engine.note_on(0, 36, 100, 60_000));
    expected.extend(render_blocks(&mut inline, 3));
    inline.with_event_frame_offset(OFFSET as u32, |engine| engine.note_off(0, 36));
    expected.extend(render_blocks(&mut inline, 4));

    let rendered = render_routing_tree_note(sampler_engine(), 36, OFFSET as u32);

    let pipeline = 2 * BLOCK;
    assert!(rendered[..pipeline].iter().all(|sample| *sample == 0.0));
    assert_bits_eq(
        &rendered[pipeline..],
        &expected[..expected.len() - pipeline],
    );
}
