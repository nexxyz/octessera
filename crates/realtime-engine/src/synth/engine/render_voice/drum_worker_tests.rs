use super::super::*;
use super::drum_tests::drum_engine;
use crate::synth::DrumConfig;

/// Renders `blocks` blocks on the routing-tree workers and inline and asserts they match bit for
/// bit. `cut` is the block before which a note-off of `note` reaches both engines between blocks.
/// The worker pipeline renders one block ahead, so the controls handed to the call that returns
/// block `cut - 1` are the ones that land on block `cut`'s start, where the inline engine applies
/// them.
#[cfg(feature = "routing-tree-executor")]
fn assert_cut_renders_match(
    mut worker: SynthEngine,
    mut inline: SynthEngine,
    note: u8,
    cut: usize,
) {
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut worker, 128).unwrap();
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    for block in 0..100 {
        if block == cut {
            inline.with_event_frame_offset(0, |engine| engine.note_off(0, note));
        }
        let disposition = if block + 1 == cut {
            worker.render_interleaved_block_with_source_runtime_ready_with_controls(
                &mut runtime,
                128,
                &mut left,
                &mut right,
                &mut out,
                |engine| {
                    engine.with_event_frame_offset(0, |engine| engine.note_off(0, note));
                    Ok(())
                },
            )
        } else {
            worker.render_interleaved_block_with_source_runtime(
                &mut runtime,
                128,
                &mut left,
                &mut right,
                &mut out,
            )
        };
        assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
        let (mut expected_left, mut expected_right, mut expected) =
            (Vec::new(), Vec::new(), Vec::new());
        inline.render_interleaved_block(
            128,
            &mut expected_left,
            &mut expected_right,
            &mut expected,
        );
        for (a, b) in out.iter().zip(expected) {
            assert_eq!(a.to_bits(), b.to_bits(), "block {block}: {a} vs {b}");
        }
    }
    let retirement = runtime.retire();
    assert_eq!(lifecycle.shutdown(retirement).joined_workers, 2);
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn drum_routing_tree_worker_matches_inline_for_distinct_hits_and_cuts() {
    let mut worker = drum_engine(DrumConfig::default(), 44_100);
    let mut inline = drum_engine(DrumConfig::default(), 44_100);
    for engine in [&mut worker, &mut inline] {
        engine.tile_hit(0, 0, 0, -2400, 120);
        engine.tile_hit(0, 2, 0, 2400, 100);
        let cut = engine.synth_voice_pool.lane(0).unwrap().midi_note;
        engine.with_event_frame_offset(64, |engine| engine.note_off(0, cut));
    }
    let between_blocks = inline.synth_voice_pool.lane(1).unwrap().midi_note;
    assert_cut_renders_match(worker, inline, between_blocks, 20);
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn synth_routing_tree_worker_matches_inline_for_a_note_off_between_blocks() {
    let mut worker = SynthEngine::new(44_100);
    let mut inline = SynthEngine::new(44_100);
    for engine in [&mut worker, &mut inline] {
        engine.note_on(0, 60, 100, 60_000);
    }
    assert_cut_renders_match(worker, inline, 60, 20);
}
