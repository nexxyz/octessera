use super::super::types::{MomentaryFxTarget, BUS_COUNT};
use super::bus_chain_owner::BusChainCarrier;
use super::retired_state::{store_retired_momentary, store_retired_preview};
use super::routing_tree_source_bank::RoutingTreeSourceBank;
use super::routing_tree_worker::RoutingTreeOwnerData;
use super::SynthEngine;

pub(super) fn take_source_bank(engine: &mut SynthEngine) -> Option<Box<RoutingTreeSourceBank>> {
    let mut bank = RoutingTreeSourceBank::empty();
    if !engine
        .synth_voice_pool
        .take_routing_bank_into(&mut bank.synth)
    {
        return None;
    }
    if !engine
        .sample_voice_pool
        .take_routing_bank_into(&mut bank.sample)
    {
        assert!(
            engine
                .synth_voice_pool
                .install_routing_bank(&mut bank.synth),
            "failed to restore synth routing bank after sample bank extraction failure"
        );
        return None;
    }
    Some(bank)
}

pub(super) fn install_source_bank(
    engine: &mut SynthEngine,
    bank: &mut RoutingTreeSourceBank,
) -> bool {
    engine
        .synth_voice_pool
        .install_routing_bank(&mut bank.synth)
        && engine
            .sample_voice_pool
            .install_routing_bank(&mut bank.sample)
}

pub(super) fn restore_source_bank(
    engine: &mut SynthEngine,
    bank: &mut RoutingTreeSourceBank,
) -> bool {
    if !engine.synth_voice_pool.restore_empty_routing_home()
        || !engine.sample_voice_pool.restore_empty_routing_home()
    {
        return false;
    }
    install_source_bank(engine, bank)
}

pub(super) fn move_owner_state_to_engine(
    engine: &mut SynthEngine,
    first: &mut RoutingTreeOwnerData,
    second: &mut RoutingTreeOwnerData,
) -> bool {
    let Some(assignment) = engine.routing_tree_assignment() else {
        return false;
    };
    if !preflight_owner_state_installed(engine, &assignment, first, second) {
        return false;
    }
    transfer_owner_retirements(engine, first, second);
    let Some(first_bank) = first.source_bank.as_mut() else {
        return false;
    };
    let Some(second_bank) = second.source_bank.as_mut() else {
        return false;
    };
    if !first_bank.merge_from(second_bank) || !install_source_bank(engine, first_bank) {
        return false;
    }
    for index in 0..first.preview_sample_voices.len() {
        if engine.preview_sample_voices[index].is_some() {
            return false;
        }
        let first_order = first.preview_sample_orders[index];
        let first_voice = first.preview_sample_voices[index].take();
        let first_present = first_voice.is_some();
        let second_order = second.preview_sample_orders[index];
        let second_voice = second.preview_sample_voices[index].take();
        engine.preview_sample_voices[index] = first_voice.or(second_voice);
        engine.preview_sample_orders[index] = if first_present {
            first_order
        } else {
            second_order
        };
        first.preview_sample_orders[index] = 0;
        second.preview_sample_orders[index] = 0;
    }
    first.momentary_fx.append(&mut engine.momentary_fx);
    std::mem::swap(&mut first.momentary_fx, &mut engine.momentary_fx);
    engine.momentary_fx.append(&mut second.momentary_fx);
    true
}

pub(super) fn move_engine_aux_to_owners(
    engine: &mut SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
    first: &mut RoutingTreeOwnerData,
    second: &mut RoutingTreeOwnerData,
) -> bool {
    if !preflight_engine_aux(engine, assignment, first, second) {
        return false;
    }
    for index in 0..engine.preview_sample_voices.len() {
        let Some(voice) = engine.preview_sample_voices[index].take() else {
            continue;
        };
        let Some(worker) = assignment.worker_for_slot(voice.slot) else {
            return false;
        };
        let target = if worker == 0 {
            &mut *first
        } else {
            &mut *second
        };
        if target.preview_sample_voices[index].is_some() {
            return false;
        }
        target.preview_sample_orders[index] = engine.preview_sample_orders[index];
        target.preview_sample_voices[index] = Some(voice);
        engine.preview_sample_orders[index] = 0;
    }
    let mut index = 0;
    while index < engine.momentary_fx.len() {
        if engine.momentary_fx[index].target == MomentaryFxTarget::Global {
            index += 1;
            continue;
        }
        let state = engine.momentary_fx.remove(index);
        let worker = match state.target {
            MomentaryFxTarget::Instrument { index } => assignment.worker_for_slot(index),
            MomentaryFxTarget::FxBus { index } => assignment.worker_for_bus(index),
            MomentaryFxTarget::Global => None,
        };
        let Some(worker) = worker else {
            return false;
        };
        let target = if worker == 0 {
            &mut *first
        } else {
            &mut *second
        };
        target.momentary_fx.push(state);
    }
    true
}

pub(super) fn move_engine_state_to_owners(
    engine: &mut SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
    first: &mut RoutingTreeOwnerData,
    second: &mut RoutingTreeOwnerData,
) -> bool {
    let Some(first_bank) = first.source_bank.as_mut() else {
        return false;
    };
    let Some(second_bank) = second.source_bank.as_mut() else {
        return false;
    };
    if !preflight_engine_source_state(engine, assignment, first_bank, second_bank) {
        return false;
    }
    if !engine
        .synth_voice_pool
        .take_routing_bank_into(&mut first_bank.synth)
        || !engine
            .sample_voice_pool
            .take_routing_bank_into(&mut first_bank.sample)
    {
        return false;
    }
    second_bank.clear();
    if !first_bank.reassign_to(second_bank, assignment) {
        return false;
    }
    move_engine_aux_to_owners(engine, assignment, first, second)
}

pub(super) fn preflight_owner_state(
    engine: &SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
    first: &RoutingTreeOwnerData,
    second: &RoutingTreeOwnerData,
) -> bool {
    let result = super::source_worker_transfer::source_partitions_vacant(engine)
        && engine.bus_chains.is_empty()
        && engine.preview_sample_voices.iter().all(Option::is_none)
        && owner_state_matches(first, 0, assignment)
        && owner_state_matches(second, 1, assignment)
        && retirement_capacity_available(engine, first, second);
    result
}

fn preflight_owner_state_installed(
    engine: &SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
    first: &RoutingTreeOwnerData,
    second: &RoutingTreeOwnerData,
) -> bool {
    engine.synth_voice_pool.has_home()
        && engine.sample_voice_pool.has_home()
        && engine.preview_sample_voices.iter().all(Option::is_none)
        && owner_state_matches(first, 0, assignment)
        && owner_state_matches(second, 1, assignment)
        && retirement_capacity_available(engine, first, second)
}

fn owner_state_matches(
    owner: &RoutingTreeOwnerData,
    parity: usize,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
) -> bool {
    let Some(bank) = owner.source_bank.as_ref() else {
        return false;
    };
    bank.synth
        .iter()
        .filter(|voice| voice.active)
        .all(|voice| assignment.worker_for_slot(voice.instrument_slot as usize) == Some(parity))
        && bank
            .sample
            .iter()
            .filter(|voice| voice.active)
            .all(|voice| assignment.worker_for_slot(voice.instrument_slot as usize) == Some(parity))
        && owner
            .preview_sample_voices
            .iter()
            .flatten()
            .all(|voice| assignment.worker_for_slot(voice.slot) == Some(parity))
        && owner.momentary_fx.iter().all(|state| match state.target {
            MomentaryFxTarget::Instrument { index } => {
                assignment.worker_for_slot(index) == Some(parity)
            }
            MomentaryFxTarget::FxBus { index } => assignment.worker_for_bus(index) == Some(parity),
            MomentaryFxTarget::Global => false,
        })
}

fn preflight_engine_aux(
    engine: &SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
    first: &RoutingTreeOwnerData,
    second: &RoutingTreeOwnerData,
) -> bool {
    first.preview_sample_voices.iter().all(Option::is_none)
        && second.preview_sample_voices.iter().all(Option::is_none)
        && first.momentary_fx.len() + second.momentary_fx.len() <= super::control::MAX_MOMENTARY_FX
        && engine
            .preview_sample_voices
            .iter()
            .enumerate()
            .all(|(index, voice)| {
                voice.as_ref().is_none_or(|voice| {
                    assignment.worker_for_slot(voice.slot).is_some()
                        && first.preview_sample_voices[index].is_none()
                        && second.preview_sample_voices[index].is_none()
                })
            })
        && engine.momentary_fx.iter().all(|state| match state.target {
            MomentaryFxTarget::Global => true,
            MomentaryFxTarget::Instrument { index } => assignment.worker_for_slot(index).is_some(),
            MomentaryFxTarget::FxBus { index } => assignment.worker_for_bus(index).is_some(),
        })
}

fn preflight_engine_source_state(
    engine: &SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
    first: &RoutingTreeSourceBank,
    second: &RoutingTreeSourceBank,
) -> bool {
    first.synth.iter().all(|voice| !voice.active)
        && first.sample.iter().all(|voice| !voice.active)
        && second.synth.iter().all(|voice| !voice.active)
        && second.sample.iter().all(|voice| !voice.active)
        && pools_match_assignment(engine, assignment)
        && engine_aux_targets_valid(engine, assignment)
}

fn pools_match_assignment(
    engine: &SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
) -> bool {
    (0..super::super::types::SYNTH_VOICE_LANE_CAPACITY).all(|lane| {
        engine.synth_voice_pool.lane(lane).is_none_or(|voice| {
            !voice.active
                || assignment
                    .worker_for_slot(voice.instrument_slot as usize)
                    .is_some()
        })
    }) && (0..super::super::types::SAMPLE_VOICE_LANE_CAPACITY).all(|lane| {
        engine.sample_voice_pool.lane(lane).is_none_or(|voice| {
            !voice.active
                || assignment
                    .worker_for_slot(voice.instrument_slot as usize)
                    .is_some()
        })
    })
}

fn engine_aux_targets_valid(
    engine: &SynthEngine,
    assignment: &super::routing_tree_executor::RoutingTreeAssignment,
) -> bool {
    engine
        .preview_sample_voices
        .iter()
        .flatten()
        .all(|voice| assignment.worker_for_slot(voice.slot).is_some())
        && engine.momentary_fx.iter().all(|state| match state.target {
            MomentaryFxTarget::Global => true,
            MomentaryFxTarget::Instrument { index } => assignment.worker_for_slot(index).is_some(),
            MomentaryFxTarget::FxBus { index } => assignment.worker_for_bus(index).is_some(),
        })
}

fn retirement_capacity_available(
    engine: &SynthEngine,
    first: &RoutingTreeOwnerData,
    second: &RoutingTreeOwnerData,
) -> bool {
    let preview_count = first.retired_preview_samples.iter().flatten().count()
        + second.retired_preview_samples.iter().flatten().count();
    let momentary_count = first.retired_momentary_fx.iter().flatten().count()
        + second.retired_momentary_fx.iter().flatten().count();
    preview_count
        <= engine
            .pending_render_retired
            .preview_sample_voices
            .iter()
            .filter(|slot| slot.is_none())
            .count()
        && momentary_count
            <= engine
                .pending_render_retired
                .displaced_momentary_fx
                .iter()
                .filter(|slot| slot.is_none())
                .count()
}

fn transfer_owner_retirements(
    engine: &mut SynthEngine,
    first: &mut RoutingTreeOwnerData,
    second: &mut RoutingTreeOwnerData,
) {
    for owner in [first, second] {
        for voice in owner
            .retired_preview_samples
            .iter_mut()
            .filter_map(Option::take)
        {
            store_retired_preview(
                &mut engine.pending_render_retired.preview_sample_voices,
                voice,
            );
        }
        for state in owner
            .retired_momentary_fx
            .iter_mut()
            .filter_map(Option::take)
        {
            store_retired_momentary(
                &mut engine.pending_render_retired.displaced_momentary_fx,
                state,
            );
        }
    }
}

pub(super) fn sync_routing_tree_spread_states_to_engine(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; BUS_COUNT],
) -> bool {
    for (bus, carrier) in carriers.iter_mut().enumerate() {
        let Some(carrier) = carrier.as_mut() else {
            return false;
        };
        let Some(state) = carrier.routing_tree_spread_state.as_mut() else {
            return false;
        };
        if let Some(engine_state) = engine.bus_output_spread_state.get_mut(bus) {
            std::mem::swap(state, engine_state);
        } else if carrier.owner.is_some() {
            return false;
        }
    }
    true
}

pub(super) fn sync_routing_tree_spread_states_to_carriers(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; BUS_COUNT],
) -> bool {
    sync_routing_tree_spread_states_to_engine(engine, carriers)
}
