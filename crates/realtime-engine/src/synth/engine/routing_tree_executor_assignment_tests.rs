use super::render_plan::RenderPlan;
use super::routing_tree_executor::RoutingTreeBlockScratch;
use super::routing_tree_executor_test_support::*;
use super::routing_tree_plan::{RoutingTreePlan, INVALID_COMPONENT_ID, ROUTING_NODE_COUNT};
use super::support::InstrumentKind;
use super::*;
use crate::synth::{BUS_COUNT, INSTRUMENT_SLOT_COUNT};

#[test]
fn routing_tree_reassociation_changes_only_final_worker_sum() {
    let mut config = routed_config();
    config.mixer = None;
    for instrument in &mut config.instruments {
        instrument.mixer.as_mut().unwrap().route = "direct".into();
    }
    let mut tree = SynthEngine::new(48_000);
    let mut source_reference = SynthEngine::new(48_000);
    tree.set_instruments(config.clone());
    source_reference.set_instruments(config);
    tree.set_sample_banks(sample_banks());
    source_reference.set_sample_banks(sample_banks());
    for (slot, note, velocity) in [(0, 36, 127), (1, 60, 96), (2, 64, 88), (3, 67, 80)] {
        tree.note_on(slot, note, velocity, 1_000);
        source_reference.note_on(slot, note, velocity, 1_000);
    }
    let mut left = [0.0; 128];
    let mut right = [0.0; 128];
    assert!(tree.render_routing_tree_block_for_test(128, &mut left, &mut right));
    let (workers, plan) = tree.routing_tree_scratch.assignment_for_test();
    let mut saw_final_reassociation = false;
    for frame in 0..128 {
        let mut slot_out = [0.0; INSTRUMENT_SLOT_COUNT];
        source_reference.render_sample_voices(&mut slot_out);
        source_reference.render_preview_sample_voices(&mut slot_out);
        source_reference.render_synth_voices(&mut slot_out);
        let mut worker_left = [0.0; 2];
        let mut worker_right = [0.0; 2];
        let mut serial_left = 0.0;
        let mut serial_right = 0.0;
        for (slot, raw_sample) in slot_out.iter().enumerate() {
            let sample = *raw_sample * source_reference.slot_volume[slot];
            let (pan_left, pan_right) = source_reference.slot_pan_gains[slot];
            let contribution = (sample * pan_left, sample * pan_right);
            serial_left += contribution.0;
            serial_right += contribution.1;
            if plan.slot_component[slot] == INVALID_COMPONENT_ID {
                continue;
            }
            let worker = workers[plan.slot_component[slot] as usize] as usize;
            worker_left[worker] += contribution.0;
            worker_right[worker] += contribution.1;
        }
        let actual_workers = tree.routing_tree_scratch.worker_outputs_for_test(frame);
        assert_eq!(actual_workers[0].0, worker_left[0]);
        assert_eq!(actual_workers[0].1, worker_right[0]);
        assert_eq!(actual_workers[1].0, worker_left[1]);
        assert_eq!(actual_workers[1].1, worker_right[1]);
        let actual_workers = tree.routing_tree_scratch.worker_outputs_for_test(frame);
        assert_reassociated_close(
            left[frame],
            serial_left,
            actual_workers,
            0,
            "reassociated left",
        );
        assert_reassociated_close(
            right[frame],
            serial_right,
            actual_workers,
            1,
            "reassociated right",
        );
        saw_final_reassociation |= left[frame].to_bits() != serial_left.to_bits()
            || right[frame].to_bits() != serial_right.to_bits();
    }
    assert!(saw_final_reassociation);
}

#[test]
fn routing_tree_assignment_is_deterministic_and_balances_whole_components() {
    let mut first = SynthEngine::new(48_000);
    let mut second = SynthEngine::new(48_000);
    let mut reference = SynthEngine::new(48_000);
    let config = direct_synth_config();
    first.set_instruments(config.clone());
    second.set_instruments(config.clone());
    reference.set_instruments(config);
    for note in [48, 55, 62] {
        first.note_on(0, note, 100, 1_000);
        second.note_on(0, note, 100, 1_000);
        reference.note_on(0, note, 100, 1_000);
    }
    first.note_on(1, 72, 100, 1_000);
    second.note_on(1, 72, 100, 1_000);
    reference.note_on(1, 72, 100, 1_000);
    let mut first_left = vec![0.0; 128];
    let mut first_right = vec![0.0; 128];
    let mut second_left = vec![0.0; 128];
    let mut second_right = vec![0.0; 128];
    assert!(first.render_routing_tree_block_for_test(128, &mut first_left, &mut first_right));
    assert!(second.render_routing_tree_block_for_test(128, &mut second_left, &mut second_right));
    let (first_workers, first_plan) = first.routing_tree_scratch.assignment_for_test();
    let (second_workers, _) = second.routing_tree_scratch.assignment_for_test();
    assert_eq!(first_workers, second_workers);
    assert_eq!(first_plan.slot_component[0], 0);
    assert_eq!(first_plan.slot_component[1], 1);
    assert_eq!(first_workers[0], 0);
    assert_eq!(first_workers[1], 1);
    assert_eq!(first_left, second_left);
    assert_eq!(first_right, second_right);
    for frame in 0..128 {
        let (expected_left, expected_right) = reference.next_stereo_sample();
        assert_ulp_close(first_left[frame], expected_left, 8, "assignment left");
        assert_ulp_close(first_right[frame], expected_right, 8, "assignment right");
    }
}

#[test]
fn routing_tree_assignment_covers_bus_costs_zero_costs_and_ties() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(bus_assignment_config());
    let plan = RoutingTreePlan::from_render_plan(&engine.render_plan);
    let mut scratch = RoutingTreeBlockScratch::new();
    let none_kinds = [InstrumentKind::None; INSTRUMENT_SLOT_COUNT];
    assert!(scratch.assign_workers_for_test(
        plan,
        none_kinds,
        [0; INSTRUMENT_SLOT_COUNT],
        [0; INSTRUMENT_SLOT_COUNT],
        [3, 5, 0, 0],
        2,
    ));
    let (workers, _) = scratch.assignment_for_test();
    assert_eq!(&workers[..plan.component_count], &[0, 0, 0, 1]);

    let saturated_plan = RoutingTreePlan::from_render_plan(&engine.render_plan);
    assert!(!scratch.assign_workers_for_test(
        saturated_plan,
        [InstrumentKind::Synth; INSTRUMENT_SLOT_COUNT],
        [usize::MAX; INSTRUMENT_SLOT_COUNT],
        [0; INSTRUMENT_SLOT_COUNT],
        [u16::MAX; 4],
        2,
    ));

    let tie_plan = RoutingTreePlan::from_render_plan(&engine.render_plan);
    assert!(scratch.assign_workers_for_test(
        tie_plan,
        none_kinds,
        [0; INSTRUMENT_SLOT_COUNT],
        [0; INSTRUMENT_SLOT_COUNT],
        [0; BUS_COUNT],
        2,
    ));
    let (tie_workers, _) = scratch.assignment_for_test();
    assert_eq!(&tie_workers[..tie_plan.component_count], &[0, 0, 0, 0]);

    assert!(!scratch.assign_workers_for_test(
        tie_plan,
        none_kinds,
        [0; INSTRUMENT_SLOT_COUNT],
        [0; INSTRUMENT_SLOT_COUNT],
        [0; BUS_COUNT],
        BUS_COUNT + 1,
    ));
}

#[test]
fn routing_tree_worker_lookup_rejects_invalid_components_and_indices() {
    let mut scratch = RoutingTreeBlockScratch::new();
    let mut plan = RoutingTreePlan::from_render_plan(&RenderPlan::new());
    plan.component_count = 1;
    plan.slot_component[0] = 0;
    plan.bus_component[0] = ROUTING_NODE_COUNT as u8;
    scratch.set_assignment_for_test([0; ROUTING_NODE_COUNT], plan);
    assert_eq!(scratch.worker_for_slot(0), Some(0));
    assert_eq!(scratch.worker_for_slot(INSTRUMENT_SLOT_COUNT), None);
    assert_eq!(scratch.worker_for_bus(0), None);
    assert_eq!(scratch.worker_for_bus(BUS_COUNT), None);

    scratch.set_assignment_for_test([u8::MAX; ROUTING_NODE_COUNT], plan);
    assert_eq!(scratch.worker_for_slot(0), None);
    scratch.set_assignment_for_test([2; ROUTING_NODE_COUNT], plan);
    assert_eq!(scratch.worker_for_slot(0), None);
    plan.slot_component[0] = INVALID_COMPONENT_ID;
    scratch.set_assignment_for_test([0; ROUTING_NODE_COUNT], plan);
    assert_eq!(scratch.worker_for_slot(0), None);
}
