use super::super::dsp_config::BusIdleThreshold;
use super::super::fx_params::{DuckSource, FxBusParams};
use super::super::types::{
    MomentaryFxTarget, BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT, VOICE_PARTITION_COUNT,
};
use super::bus_chain_owner::{BusChainCarrier, BusChainFrameOutput, BusChainOwner};
use super::source_worker_lifecycle::OwnerEnvelope;
use super::source_worker_protocol::WorkStamp;
use super::SynthEngine;
use super::BLOCK_SLOT_SCRATCH_FRAMES;

pub(super) fn stage_bus_block(
    engine: &mut SynthEngine,
    owners: &mut [OwnerEnvelope; VOICE_PARTITION_COUNT],
    slot_out: &[Vec<f32>; INSTRUMENT_SLOT_COUNT],
    frames: usize,
) -> bool {
    if frames > BLOCK_SLOT_SCRATCH_FRAMES || !valid_carrier_layout(owners) {
        return false;
    }
    for owner in owners.iter_mut() {
        for carrier in owner.bus_carriers.iter_mut().flatten() {
            if !carrier.scratch.prepare(frames) {
                return false;
            }
        }
    }
    for frame in 0..frames {
        for (slot, slot_output) in slot_out.iter().enumerate() {
            let mut sample =
                slot_output.get(frame).copied().unwrap_or(0.0) * engine.slot_volume[slot];
            if !engine.momentary_fx.is_empty() {
                let (left, right) = engine.process_momentary_fx_target(
                    MomentaryFxTarget::Instrument { index: slot },
                    sample,
                    sample,
                );
                sample = (left + right) * 0.5;
            }
            let route = engine.slot_route[slot];
            if route == 0 {
                continue;
            }
            let bus = route - 1;
            if bus >= engine.bus_pan_pos.len() {
                continue;
            }
            let Some(carrier) = carrier_mut(owners, bus) else {
                return false;
            };
            carrier.scratch.input[frame] += sample;
        }
    }
    for bus in 0..BUS_COUNT {
        for slot in 0..BUS_SLOTS_PER_BUS {
            let Some(source) = duck_source(owners, bus, slot) else {
                continue;
            };
            for frame in 0..frames {
                let source_value = match source {
                    DuckSource::Instrument(index) => slot_out
                        .get(index)
                        .and_then(|output| output.get(frame))
                        .copied()
                        .unwrap_or(0.0),
                    DuckSource::Bus(index) => carrier_ref(owners, index)
                        .map(|carrier| carrier.scratch.input[frame])
                        .unwrap_or(0.0),
                };
                let Some(carrier) = carrier_mut(owners, bus) else {
                    return false;
                };
                carrier.scratch.resolved_duck[slot][frame] = source_value;
            }
        }
    }
    true
}

pub(super) fn stage_source_block(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; BUS_COUNT],
    frames: usize,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    if frames > BLOCK_SLOT_SCRATCH_FRAMES
        || engine.bus_pan_pos.len() > BUS_COUNT
        || left.len() < frames
        || right.len() < frames
    {
        return false;
    }
    for carrier in carriers.iter_mut().flatten() {
        if !carrier.scratch.prepare(frames) {
            return false;
        }
    }
    left[..frames].fill(0.0);
    right[..frames].fill(0.0);
    for frame in 0..frames {
        let mut slot_out = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        let mut sample_active = false;
        let mut synth_active = false;
        for (slot, output) in slot_out.iter_mut().enumerate() {
            *output = engine.block_slot_scratch.sample_slot_out[slot][frame];
            sample_active |= engine.block_slot_scratch.sample_active[slot][frame];
        }
        let preview_active = engine.render_preview_sample_voices(&mut slot_out);
        for (slot, output) in slot_out.iter_mut().enumerate() {
            *output += engine.block_slot_scratch.synth_slot_out[slot][frame];
            synth_active |= engine.block_slot_scratch.synth_active[slot][frame];
        }
        engine.block_slot_scratch.source_active[frame] =
            sample_active || preview_active || synth_active;
        if !stage_carrier_frame(engine, carriers, &slot_out, frame, left, right) {
            return false;
        }
        for bus in 0..engine.bus_pan_pos.len() {
            for slot in 0..BUS_SLOTS_PER_BUS {
                if matches!(
                    bus_duck_source(engine, bus, slot),
                    Some(DuckSource::Instrument(_))
                ) {
                    let source = match bus_duck_source(engine, bus, slot) {
                        Some(DuckSource::Instrument(index)) => {
                            slot_out.get(index).copied().unwrap_or(0.0)
                        }
                        _ => 0.0,
                    };
                    let Some(carrier) = carriers.get_mut(bus).and_then(Option::as_mut) else {
                        return false;
                    };
                    carrier.scratch.resolved_duck[slot][frame] = source;
                }
            }
        }
    }
    for bus in 0..engine.bus_pan_pos.len() {
        for slot in 0..BUS_SLOTS_PER_BUS {
            if let Some(DuckSource::Bus(index)) = bus_duck_source(engine, bus, slot) {
                for frame in 0..frames {
                    let source = carriers
                        .get(index)
                        .and_then(|carrier| carrier.as_ref())
                        .map(|carrier| carrier.scratch.input[frame])
                        .unwrap_or(0.0);
                    let Some(carrier) = carriers.get_mut(bus).and_then(Option::as_mut) else {
                        return false;
                    };
                    carrier.scratch.resolved_duck[slot][frame] = source;
                }
            }
        }
    }
    reactivate_parked_carriers(engine, carriers, frames);
    true
}

fn stage_carrier_frame(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; BUS_COUNT],
    slot_out: &[f32; INSTRUMENT_SLOT_COUNT],
    frame: usize,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    for (slot, output) in slot_out.iter().enumerate() {
        let mut sample = *output * engine.slot_volume[slot];
        if !engine.momentary_fx.is_empty() {
            let (processed_left, processed_right) = engine.process_momentary_fx_target(
                MomentaryFxTarget::Instrument { index: slot },
                sample,
                sample,
            );
            sample = (processed_left + processed_right) * 0.5;
        }
        let route = engine.slot_route[slot];
        let bus = route.saturating_sub(1);
        if route == 0 || bus >= engine.bus_pan_pos.len() {
            let (pan_left, pan_right) = engine.slot_pan_gains[slot];
            left[frame] += sample * pan_left;
            right[frame] += sample * pan_right;
            continue;
        }
        let Some(carrier) = carriers[bus].as_mut() else {
            return false;
        };
        carrier.scratch.input[frame] += sample;
    }
    true
}

fn bus_duck_source(engine: &SynthEngine, bus: usize, slot: usize) -> Option<DuckSource> {
    engine
        .bus_chains
        .iter()
        .find(|owner| owner.logical_bus_id == bus)
        .and_then(|owner| match owner.slot_params[slot] {
            FxBusParams::Duck { source, .. } => Some(source),
            _ => None,
        })
}

fn reactivate_parked_carriers(
    engine: &mut SynthEngine,
    carriers: &mut [Option<BusChainCarrier>; BUS_COUNT],
    frames: usize,
) {
    let threshold = engine.dsp_config.bus_idle_threshold;
    for (bus, carrier) in carriers.iter().enumerate().take(engine.bus_pan_pos.len()) {
        let input_is_loud = carrier.as_ref().is_some_and(|carrier| {
            carrier.scratch.input[..frames]
                .iter()
                .any(|input| BusChainOwner::is_loud(*input, 0.0, threshold))
        });
        let Some(chain) = engine
            .bus_chains
            .iter()
            .find(|owner| owner.logical_bus_id == bus)
        else {
            continue;
        };
        if !input_is_loud || chain.assigned_worker.is_some() || chain.cost_units() == 0 {
            continue;
        }
        let cost = chain.cost_units();
        let Some(worker) = engine.choose_bus_worker(cost) else {
            continue;
        };
        if let Some(chain) = engine
            .bus_chains
            .iter_mut()
            .find(|owner| owner.logical_bus_id == bus)
        {
            chain.assigned_worker = Some(worker);
        }
    }
}

pub(super) fn render_bus_block(
    owner: &mut OwnerEnvelope,
    expected_parity: usize,
    stamp: WorkStamp,
    frames: usize,
    sample_rate: u32,
    threshold: BusIdleThreshold,
    hold_frames: u32,
) -> Result<u16, ()> {
    if frames != stamp.frames
        || frames > BLOCK_SLOT_SCRATCH_FRAMES
        || sample_rate == 0
        || owner.parity != expected_parity
        || owner.parity >= VOICE_PARTITION_COUNT
        || owner.partitions.synth.parity() != owner.parity
        || owner.partitions.sample.parity() != owner.parity
        || !valid_owner_carriers(owner)
    {
        return Err(());
    }
    let mut executed_cost: u16 = 0;
    for logical_bus_id in 0..BUS_COUNT {
        let Some(carrier) = owner.bus_carriers[logical_bus_id].as_mut() else {
            continue;
        };
        let Some(chain) = carrier.owner.as_mut() else {
            if !carrier.scratch.prepare_for_render(frames) {
                return Err(());
            }
            continue;
        };
        executed_cost = executed_cost.saturating_add(chain.process_block(
            &mut carrier.scratch,
            frames,
            sample_rate,
            threshold,
            hold_frames,
        )?);
    }
    Ok(executed_cost)
}

pub(super) fn apply_bus_block(
    engine: &mut SynthEngine,
    owners: &[OwnerEnvelope; VOICE_PARTITION_COUNT],
    frames: usize,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    if frames > BLOCK_SLOT_SCRATCH_FRAMES
        || left.len() < frames
        || right.len() < frames
        || !valid_carrier_layout(owners)
        || !valid_carrier_residency(owners)
    {
        return false;
    }
    let carriers = std::array::from_fn(|logical_bus_id| carrier_ref(owners, logical_bus_id));
    apply_bus_output(engine, &carriers, frames, left, right)
}

pub(super) fn apply_bus_block_from_carriers(
    engine: &mut SynthEngine,
    carriers: &[Option<BusChainCarrier>; BUS_COUNT],
    frames: usize,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    if carriers
        .iter()
        .enumerate()
        .any(|(logical_bus_id, carrier)| {
            carrier.as_ref().is_none_or(|carrier| {
                carrier.logical_bus_id != logical_bus_id || !carrier.within_worker_capacity()
            })
        })
    {
        return false;
    }
    let carriers = std::array::from_fn(|logical_bus_id| carriers[logical_bus_id].as_ref());
    apply_bus_output(engine, &carriers, frames, left, right)
}

fn apply_bus_output(
    engine: &mut SynthEngine,
    carriers: &[Option<&BusChainCarrier>; BUS_COUNT],
    frames: usize,
    left: &mut [f32],
    right: &mut [f32],
) -> bool {
    for frame in 0..frames {
        let mut active_buses = 0;
        for (logical_bus_id, carrier) in carriers.iter().enumerate() {
            let Some(carrier) = *carrier else {
                return false;
            };
            if carrier.owner.is_none() {
                continue;
            }
            if carrier.owner.as_ref().is_some_and(BusChainOwner::is_active) {
                active_buses += 1;
            }
            if !carrier.scratch.executed || frame >= carrier.scratch.processed_prefix {
                continue;
            }
            let mut mono = carrier.scratch.mono_output[frame];
            if !engine.momentary_fx.is_empty() {
                let (processed_left, processed_right) = engine.process_momentary_fx_target(
                    MomentaryFxTarget::FxBus {
                        index: logical_bus_id,
                    },
                    mono,
                    mono,
                );
                mono = (processed_left + processed_right) * 0.5;
            }
            let output = BusChainFrameOutput {
                mono,
                auto_pan_pos: (!carrier.scratch.auto_pan_pos[frame].is_nan())
                    .then_some(carrier.scratch.auto_pan_pos[frame]),
                spread: carrier.scratch.spread,
            };
            let (processed_left, processed_right) =
                engine.fx_bus_stereo_output(logical_bus_id, output);
            left[frame] += processed_left;
            right[frame] += processed_right;
        }
        engine.block_slot_scratch.bus_active[frame] = active_buses > 0;
    }
    engine.active_bus_activity_count = carriers
        .iter()
        .filter_map(|carrier| carrier.and_then(|carrier| carrier.owner.as_ref()))
        .filter(|owner| owner.is_active())
        .count();
    true
}

fn duck_source(
    owners: &[OwnerEnvelope; VOICE_PARTITION_COUNT],
    bus: usize,
    slot: usize,
) -> Option<DuckSource> {
    carrier_ref(owners, bus)
        .and_then(|carrier| carrier.owner.as_ref())
        .and_then(|owner| match owner.slot_params[slot] {
            FxBusParams::Duck { source, .. } => Some(source),
            _ => None,
        })
}

fn carrier_ref(
    owners: &[OwnerEnvelope; VOICE_PARTITION_COUNT],
    logical_bus_id: usize,
) -> Option<&BusChainCarrier> {
    owners
        .iter()
        .find_map(|owner| owner.bus_carriers[logical_bus_id].as_ref())
}

fn carrier_mut(
    owners: &mut [OwnerEnvelope; VOICE_PARTITION_COUNT],
    logical_bus_id: usize,
) -> Option<&mut BusChainCarrier> {
    owners
        .iter_mut()
        .find_map(|owner| owner.bus_carriers[logical_bus_id].as_mut())
}

fn valid_carrier_layout(owners: &[OwnerEnvelope; VOICE_PARTITION_COUNT]) -> bool {
    (0..BUS_COUNT).all(|logical_bus_id| {
        let carriers = owners
            .iter()
            .filter_map(|owner| owner.bus_carriers[logical_bus_id].as_ref());
        let mut count = 0;
        for carrier in carriers {
            count += 1;
            if carrier.logical_bus_id != logical_bus_id {
                return false;
            }
        }
        count == 1
    })
}

fn valid_carrier_residency(owners: &[OwnerEnvelope; VOICE_PARTITION_COUNT]) -> bool {
    (0..BUS_COUNT).all(|logical_bus_id| {
        let Some(carrier) = carrier_ref(owners, logical_bus_id) else {
            return false;
        };
        let actual_parity = owners
            .iter()
            .find(|owner| owner.bus_carriers[logical_bus_id].is_some())
            .map(|owner| owner.parity);
        let expected_parity = carrier
            .owner
            .as_ref()
            .and_then(|owner| owner.assigned_worker)
            .unwrap_or(logical_bus_id % VOICE_PARTITION_COUNT);
        actual_parity == Some(expected_parity)
    })
}

fn valid_owner_carriers(owner: &OwnerEnvelope) -> bool {
    (0..BUS_COUNT).all(|logical_bus_id| {
        let Some(carrier) = owner.bus_carriers[logical_bus_id].as_ref() else {
            return true;
        };
        carrier.logical_bus_id == logical_bus_id
            && carrier
                .owner
                .as_ref()
                .is_none_or(|chain| chain.logical_bus_id == logical_bus_id)
            && carrier.within_worker_capacity()
    })
}
