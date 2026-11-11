use super::{
    ArpDto, ArpMode, ArpSource, AxisDto, LaneCurve, LinkDto, LinkEventTiming, MappingEventDto,
    NativeLinkArp, NativeLinkLayer, NativeValueLane, PitchDto, RangeMode, ScanAxis, ScanDirection,
    ScanMode, SlotDto, TriggerAction, TriggerProbabilityMode, ValueLaneDto, INSTRUMENT_COUNT,
};

pub(super) fn apply_link_payload(layer: &mut NativeLinkLayer, payload: &LinkDto) {
    apply_scan_and_trigger_payload(layer, payload);
    if let Some(mapping) = &payload.mapping {
        let events = [
            (
                &mapping.scanned,
                &mut layer.scanned_slot,
                &mut layer.scanned_action,
                &mut layer.scanned_timing,
            ),
            (
                &mapping.scanned_empty,
                &mut layer.scanned_empty_slot,
                &mut layer.scanned_empty_action,
                &mut layer.scanned_empty_timing,
            ),
            (
                &mapping.activate,
                &mut layer.activate_slot,
                &mut layer.activate_action,
                &mut layer.activate_timing,
            ),
            (
                &mapping.stable,
                &mut layer.stable_slot,
                &mut layer.stable_action,
                &mut layer.stable_timing,
            ),
            (
                &mapping.deactivate,
                &mut layer.deactivate_slot,
                &mut layer.deactivate_action,
                &mut layer.deactivate_timing,
            ),
        ];
        for (event, slot, action, timing) in events {
            if let Some(event) = event {
                apply_mapping_event_payload(event, slot, action, timing);
            }
        }
    }
    if let Some(pitch) = &payload.pitch {
        apply_pitch_payload(layer, pitch);
    }
    layer.arp = payload
        .arp
        .as_ref()
        .map_or_else(NativeLinkArp::default, arp_from_payload);
    if let Some(axis) = &payload.x {
        apply_axis_payload(
            axis,
            (&mut layer.x_from, &mut layer.x_to),
            (
                &mut layer.x_pitch_enabled,
                &mut layer.x_pitch_steps,
                &mut layer.x_pitch_restart_each_section,
            ),
            [
                &mut layer.x_velocity,
                &mut layer.x_filter_cutoff,
                &mut layer.x_filter_resonance,
            ],
        );
    }
    if let Some(axis) = &payload.y {
        apply_axis_payload(
            axis,
            (&mut layer.y_from, &mut layer.y_to),
            (
                &mut layer.y_pitch_enabled,
                &mut layer.y_pitch_steps,
                &mut layer.y_pitch_restart_each_section,
            ),
            [
                &mut layer.y_velocity,
                &mut layer.y_filter_cutoff,
                &mut layer.y_filter_resonance,
            ],
        );
    }
}

/// Takes a saved value-set value; a value outside the set changes nothing.
fn assign_wire<T>(value: &Option<String>, target: &mut T, from_wire_name: fn(&str) -> Option<T>) {
    if let Some(value) = value.as_deref().and_then(from_wire_name) {
        *target = value;
    }
}

fn assign<T: Copy>(value: Option<T>, target: &mut T) {
    if let Some(value) = value {
        *target = value;
    }
}

fn arp_from_payload(arp: &ArpDto) -> NativeLinkArp {
    let mut next = NativeLinkArp::default();
    assign_wire(&arp.mode, &mut next.mode, ArpMode::from_wire_name);
    assign_wire(&arp.source, &mut next.source, ArpSource::from_wire_name);
    if let Some(value) = arp.step_interval_steps {
        next.step_interval_steps = value.clamp(1, 16) as u8;
    }
    if let Some(value) = arp.note_length_ms {
        next.note_length_ms = value.clamp(10, 2000) as u16;
    }
    if let Some(value) = arp.gate_pct {
        next.gate_pct = value.clamp(1, 100) as u8;
    }
    if let Some(value) = arp.octave_spread {
        next.octave_spread = value.clamp(0, 3) as u8;
    }
    next
}

fn apply_scan_and_trigger_payload(layer: &mut NativeLinkLayer, payload: &LinkDto) {
    assign_wire(
        &payload.scan_mode,
        &mut layer.scan_mode,
        ScanMode::from_wire_name,
    );
    assign_wire(
        &payload.scan_axis,
        &mut layer.scan_axis,
        ScanAxis::from_wire_name,
    );
    if let Some(unit) = &payload.scan_unit {
        layer.scan_unit.clone_from(unit);
    }
    assign_wire(
        &payload.scan_direction,
        &mut layer.scan_direction,
        ScanDirection::from_wire_name,
    );
    assign(
        payload.scan_sections.map(|value| value.min(8)),
        &mut layer.scan_sections,
    );
    assign(payload.event_enabled, &mut layer.event_enabled);
    assign(payload.state_notes_enabled, &mut layer.state_notes_enabled);
    assign(payload.seeded, &mut layer.seeded);
    assign_wire(
        &payload.trigger_probability_mode,
        &mut layer.trigger_probability_mode,
        TriggerProbabilityMode::from_wire_name,
    );
    assign(
        payload
            .trigger_probability_low_pct
            .map(|value| value.min(100)),
        &mut layer.trigger_probability_low_pct,
    );
    assign(
        payload
            .trigger_probability_high_pct
            .map(|value| value.min(100)),
        &mut layer.trigger_probability_high_pct,
    );
    assign(
        payload.trigger_probability_note_offs,
        &mut layer.trigger_probability_note_offs,
    );
}

fn apply_mapping_event_payload(
    event: &MappingEventDto,
    slot: &mut usize,
    action: &mut TriggerAction,
    timing: &mut LinkEventTiming,
) {
    match &event.slot {
        Some(SlotDto::None(value)) if value == "none" => *slot = usize::MAX,
        Some(SlotDto::Index(index)) => *slot = (*index).min(INSTRUMENT_COUNT - 1),
        _ => {}
    }
    assign_wire(&event.action, action, TriggerAction::from_wire_name);
    assign(
        event.delay_steps.map(|value| value.min(16)),
        &mut timing.delay_steps,
    );
    assign(
        event.retrigger_count.map(|value| value.min(8)),
        &mut timing.retrigger_count,
    );
}

fn apply_pitch_payload(layer: &mut NativeLinkLayer, pitch: &PitchDto) {
    assign(
        pitch.lowest_note.map(|value| value.min(127)),
        &mut layer.lowest_note,
    );
    assign(
        pitch.highest_note.map(|value| value.min(127)),
        &mut layer.highest_note,
    );
    assign(
        pitch.starting_note.map(|value| value.min(127)),
        &mut layer.starting_note,
    );
    if let Some(scale) = &pitch.scale {
        layer.scale.clone_from(scale);
    }
    if let Some(root) = &pitch.root {
        layer.root.clone_from(root);
    }
    assign_wire(
        &pitch.out_of_range,
        &mut layer.out_of_range,
        RangeMode::from_wire_name,
    );
}

fn apply_axis_payload(
    axis: &AxisDto,
    (from, to): (&mut u8, &mut u8),
    (pitch_enabled, pitch_steps, pitch_restart_each_section): (&mut bool, &mut i32, &mut bool),
    [velocity, filter_cutoff, filter_resonance]: [&mut NativeValueLane; 3],
) {
    assign(axis.from.map(|value| value.min(7)), from);
    assign(axis.to.map(|value| value.min(7)), to);
    if let Some(pitch) = &axis.pitch {
        assign(pitch.enabled, pitch_enabled);
        assign(pitch.steps.map(|value| value.clamp(-16, 16)), pitch_steps);
        assign(pitch.restart_each_section, pitch_restart_each_section);
    }
    for (lane, target) in [
        (&axis.velocity, velocity),
        (&axis.filter_cutoff, filter_cutoff),
        (&axis.filter_resonance, filter_resonance),
    ] {
        if let Some(lane) = lane {
            apply_value_lane_payload(target, lane);
        }
    }
}

fn apply_value_lane_payload(target: &mut NativeValueLane, lane: &ValueLaneDto) {
    assign(lane.enabled, &mut target.enabled);
    assign(lane.from.map(|value| value.min(127)), &mut target.from);
    assign(lane.to.map(|value| value.min(127)), &mut target.to);
    assign(
        lane.grid_offset.map(|value| value.clamp(-7, 7)),
        &mut target.grid_offset,
    );
    assign_wire(&lane.curve, &mut target.curve, LaneCurve::from_wire_name);
}
