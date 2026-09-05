use super::SynthEngine;

pub(super) fn worker_for_slot(engine: &SynthEngine, slot: usize) -> Option<usize> {
    #[cfg(feature = "routing-tree-benchmark")]
    if let Some(assignment) = engine.routing_tree_assignment.as_ref() {
        return assignment.worker_for_slot(slot);
    }
    let _ = (engine, slot);
    None
}

pub(super) fn choose_lane(
    engine: &SynthEngine,
    new_cost_units: u16,
    victim_lane: Option<usize>,
    inactive_lanes: [Option<usize>; 2],
) -> Option<usize> {
    let load = engine.source_worker_load.as_ref()?;
    let victim = victim_lane.map(|lane| (lane % 2, new_cost_units));
    let selected_worker = load.choose_worker(
        engine.source_worker_active_cost_units(),
        new_cost_units,
        victim,
        [inactive_lanes[0].is_some(), inactive_lanes[1].is_some()],
    )?;
    if victim_lane.is_some_and(|lane| lane % 2 == selected_worker) {
        victim_lane
    } else {
        inactive_lanes[selected_worker]
    }
}

pub(super) fn choose_lane_for_worker(
    required_worker: usize,
    victim_lane: Option<usize>,
    inactive_lanes: [Option<usize>; 2],
) -> Option<usize> {
    if required_worker >= 2 {
        return None;
    }
    if let Some(victim_lane) = victim_lane {
        return (victim_lane % 2 == required_worker).then_some(victim_lane);
    }
    inactive_lanes[required_worker]
}
