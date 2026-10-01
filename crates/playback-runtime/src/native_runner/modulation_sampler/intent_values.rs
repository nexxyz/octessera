use super::super::{NativeLinkLayer, NativeValueLane, GRID_HEIGHT, GRID_WIDTH};
use platform_core::{CellTriggerIntent, MusicalEvent};

pub(in super::super) fn cc_events_from_intent(
    intent: &CellTriggerIntent,
    sense: &NativeLinkLayer,
    channel: u8,
) -> Vec<MusicalEvent> {
    let mut events = Vec::new();
    push_lane_cc(
        &mut events,
        &sense.x_filter_cutoff,
        intent.x,
        GRID_WIDTH,
        channel,
        74,
    );
    push_lane_cc(
        &mut events,
        &sense.y_filter_cutoff,
        intent.y,
        GRID_HEIGHT,
        channel,
        74,
    );
    push_lane_cc(
        &mut events,
        &sense.x_filter_resonance,
        intent.x,
        GRID_WIDTH,
        channel,
        71,
    );
    push_lane_cc(
        &mut events,
        &sense.y_filter_resonance,
        intent.y,
        GRID_HEIGHT,
        channel,
        71,
    );
    events
}

fn push_lane_cc(
    events: &mut Vec<MusicalEvent>,
    lane: &NativeValueLane,
    index: usize,
    size: usize,
    channel: u8,
    controller: u8,
) {
    if !lane.enabled {
        return;
    }
    events.push(MusicalEvent::Cc {
        channel: channel.min(15),
        controller,
        value: value_from_lane(index, size, lane),
    });
}

pub(in super::super) fn velocity_from_intent(
    intent: &CellTriggerIntent,
    sense: &NativeLinkLayer,
) -> Option<u8> {
    let mut values = Vec::new();
    if sense.x_velocity.enabled {
        values.push(value_from_lane(intent.x, GRID_WIDTH, &sense.x_velocity));
    }
    if sense.y_velocity.enabled {
        values.push(value_from_lane(intent.y, GRID_HEIGHT, &sense.y_velocity));
    }
    if values.is_empty() {
        return None;
    }
    Some(
        ((values.iter().map(|value| u16::from(*value)).sum::<u16>() / values.len() as u16)
            .clamp(1, 127)) as u8,
    )
}

pub(in super::super) fn value_from_lane(index: usize, size: usize, lane: &NativeValueLane) -> u8 {
    let size = size.max(1);
    let shifted = ((index as i32 + lane.grid_offset).rem_euclid(size as i32)) as f32;
    let norm = (shifted / (size.saturating_sub(1).max(1) as f32)).clamp(0.0, 1.0);
    let shaped = if lane.curve == "curve" {
        norm * norm
    } else {
        norm
    };
    (f32::from(lane.from) + shaped * (f32::from(lane.to) - f32::from(lane.from)))
        .round()
        .clamp(
            f32::from(lane.from.min(lane.to)),
            f32::from(lane.from.max(lane.to)),
        ) as u8
}
