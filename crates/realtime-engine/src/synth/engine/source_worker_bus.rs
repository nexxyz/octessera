use super::super::dsp_config::BusIdleThreshold;
use super::super::fx_params::{DuckSource, FxBusParams};
use super::super::types::{
    MomentaryFxTarget, BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT, VOICE_PARTITION_COUNT,
};
use super::bus_chain_owner::{BusChainCarrier, BusChainFrameOutput};
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
            if bus >= BUS_COUNT {
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
    if frames > BLOCK_SLOT_SCRATCH_FRAMES || left.len() < frames || right.len() < frames {
        return false;
    }
    left[..frames].fill(0.0);
    right[..frames].fill(0.0);
    for frame in 0..frames {
        for logical_bus_id in 0..BUS_COUNT {
            let Some(carrier) = carrier_ref(owners, logical_bus_id) else {
                return false;
            };
            if carrier.owner.is_none() {
                continue;
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
    }
    engine.active_bus_activity_count = owners
        .iter()
        .flat_map(|owner| owner.bus_carriers.iter().flatten())
        .filter_map(|carrier| carrier.owner.as_ref())
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
