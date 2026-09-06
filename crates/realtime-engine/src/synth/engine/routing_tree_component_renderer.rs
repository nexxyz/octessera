use super::bus_chain_owner::BusChainFrameOutput;
use super::render_momentary_fx::process_momentary_fx_states;
use super::render_plan::RenderPlanRoute;
use super::render_routing::{render_bus_stereo_output, FxBusOutputSpreadState};
use super::routing_tree_worker::{RoutingTreeOwnerData, RoutingTreeWorkerContext};
use super::source_worker_lifecycle::OwnerEnvelope;
use crate::synth::fx_params::DuckSource;
use crate::synth::types::{BUS_COUNT, BUS_SLOTS_PER_BUS, INSTRUMENT_SLOT_COUNT};

pub(super) fn stage_components(
    owner: &mut OwnerEnvelope,
    routing: &mut RoutingTreeOwnerData,
    context: &RoutingTreeWorkerContext,
    frames: usize,
) -> Result<u16, ()> {
    let mut executed_cost = 0_u16;
    routing.output.bus_active[..frames].fill(false);
    for frame in 0..frames {
        let mut raw_slots = [0.0_f32; INSTRUMENT_SLOT_COUNT];
        let source_active = routing.scratch.preview_active[frame]
            || routing
                .scratch
                .source
                .sample_rendered_frames
                .iter()
                .chain(routing.scratch.source.synth_rendered_frames.iter())
                .any(|prefix| *prefix > frame);
        let mut local_active = routing.scratch.momentary_active[frame];
        for (slot, raw) in raw_slots.iter_mut().enumerate() {
            *raw = routing.scratch.slot_out[slot][frame];
            let worker = context.slot_worker[slot];
            if worker == owner.parity as u8 {
                let target = super::super::types::MomentaryFxTarget::Instrument { index: slot };
                let sample = *raw * context.slot_volume[slot];
                let (fx_l, fx_r) = process_momentary_fx_states(
                    &mut routing.momentary_fx,
                    target,
                    sample,
                    sample,
                    Some(&mut routing.retired_momentary_fx),
                    context.sample_rate,
                );
                let sample = (fx_l + fx_r) * 0.5;
                let target_active = routing.momentary_fx.iter().any(|fx| fx.target == target);
                local_active |= target_active;
                match context.slot_route[slot] {
                    RenderPlanRoute::Direct => {
                        routing.output.left[frame] += sample * context.slot_pan_gains[slot].0;
                        routing.output.right[frame] += sample * context.slot_pan_gains[slot].1;
                    }
                    RenderPlanRoute::Bus(bus) => {
                        if bus >= context.bus_count || context.bus_worker[bus] != owner.parity as u8
                        {
                            return Err(());
                        }
                        routing.scratch.bus_input[bus][frame] += sample;
                    }
                }
            } else if *raw != 0.0 && context.slot_worker[slot] != u8::MAX {
                return Err(());
            }
        }
        routing.output.source_active[frame] = source_active || local_active;
        let bus_snapshot: [f32; BUS_COUNT] =
            std::array::from_fn(|bus| routing.scratch.bus_input[bus][frame]);
        for bus in 0..context.bus_count {
            if context.bus_worker[bus] != owner.parity as u8 {
                continue;
            }
            let Some(carrier) = owner.bus_carriers[bus].as_mut() else {
                return Err(());
            };
            if carrier.owner.is_none() {
                continue;
            }
            carrier.scratch.input[frame] = routing.scratch.bus_input[bus][frame];
            for slot in 0..BUS_SLOTS_PER_BUS {
                let source = match carrier.owner.as_ref().map(|chain| chain.slot_params[slot]) {
                    Some(super::super::fx_params::FxBusParams::Duck { source, .. }) => source,
                    _ => continue,
                };
                carrier.scratch.resolved_duck[slot][frame] = match source {
                    DuckSource::Instrument(slot) if slot < INSTRUMENT_SLOT_COUNT => raw_slots[slot],
                    DuckSource::Bus(source_bus) if source_bus < context.bus_count => {
                        if context.bus_worker[source_bus] != owner.parity as u8 {
                            return Err(());
                        }
                        bus_snapshot[source_bus]
                    }
                    _ => return Err(()),
                };
            }
        }
    }
    for bus in 0..context.bus_count {
        if context.bus_worker[bus] != owner.parity as u8 {
            continue;
        }
        let Some(carrier) = owner.bus_carriers[bus].as_mut() else {
            return Err(());
        };
        let Some(chain) = carrier.owner.as_mut() else {
            continue;
        };
        if carrier.routing_tree_spread_state.is_none() {
            carrier.routing_tree_spread_state =
                Some(FxBusOutputSpreadState::new(context.sample_rate));
        }
        executed_cost = executed_cost.saturating_add(
            chain
                .process_block(
                    &mut carrier.scratch,
                    frames,
                    context.sample_rate,
                    context.bus_idle_threshold,
                    context.fx_activity_hold_frames,
                )
                .map_err(|_| ())?,
        );
        let spread_state = carrier
            .routing_tree_spread_state
            .as_mut()
            .expect("spread state");
        for frame in 0..frames {
            let target = super::super::types::MomentaryFxTarget::FxBus { index: bus };
            let processed = carrier.scratch.executed && frame < carrier.scratch.processed_prefix;
            let (fx_l, fx_r) = if processed {
                process_momentary_fx_states(
                    &mut routing.momentary_fx,
                    target,
                    carrier.scratch.mono_output[frame],
                    carrier.scratch.mono_output[frame],
                    Some(&mut routing.retired_momentary_fx),
                    context.sample_rate,
                )
            } else {
                (
                    carrier.scratch.mono_output[frame],
                    carrier.scratch.mono_output[frame],
                )
            };
            let target_active = routing.momentary_fx.iter().any(|fx| fx.target == target);
            routing.output.bus_active[frame] |= carrier.scratch.active[frame] || target_active;
            if target_active {
                routing.output.source_active[frame] = true;
            }
            routing.scratch.momentary_active[frame] |= target_active;
            if !processed {
                continue;
            }
            let output = BusChainFrameOutput {
                mono: (fx_l + fx_r) * 0.5,
                auto_pan_pos: (!carrier.scratch.auto_pan_pos[frame].is_nan())
                    .then_some(carrier.scratch.auto_pan_pos[frame]),
                spread: carrier.scratch.spread,
            };
            let (left, right) = render_bus_stereo_output(
                output,
                spread_state,
                Some(context.bus_pan_pos[bus]),
                context.pan_positions,
                context.bus_pan_gains[bus],
                context.bus_volume[bus],
            );
            routing.output.left[frame] += left;
            routing.output.right[frame] += right;
        }
    }
    routing.output.active_synth_voices = routing
        .source_bank
        .as_ref()
        .map(|bank| bank.synth.iter().filter(|voice| voice.active).count())
        .unwrap_or(0);
    routing.output.active_sample_voices = routing
        .source_bank
        .as_ref()
        .map(|bank| bank.sample.iter().filter(|voice| voice.active).count())
        .unwrap_or(0);
    routing.output.active_preview_sample_voices = routing
        .preview_sample_voices
        .iter()
        .filter(|voice| voice.is_some())
        .count();
    routing.output.active_momentary_fx = routing.momentary_fx.len();
    routing.output.active_bus_fx_slots = owner
        .bus_carriers
        .iter()
        .filter_map(|carrier| carrier.as_ref().and_then(|carrier| carrier.owner.as_ref()))
        .map(|chain| chain.active_slot_count)
        .sum();
    Ok(executed_cost)
}
