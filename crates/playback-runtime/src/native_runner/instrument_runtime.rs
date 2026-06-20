use super::*;

pub(super) fn normalize_route(route: &str) -> String {
    route
        .strip_prefix("bus_")
        .map(|suffix| format!("fx_bus_{suffix}"))
        .unwrap_or_else(|| route.to_string())
}

pub(super) fn trigger_target(
    slot: usize,
    action: &str,
    velocity: u8,
    duration_ms: u32,
) -> TriggerTarget {
    let action = if slot >= INSTRUMENT_COUNT {
        TriggerAction::None
    } else {
        match action {
            "note_off" => TriggerAction::NoteOff,
            "none" => TriggerAction::None,
            _ => TriggerAction::NoteOn,
        }
    };
    TriggerTarget {
        action,
        channel: slot.min(15) as u8,
        velocity,
        duration_ms,
    }
}

pub(super) fn slot_payload(slot: usize) -> Value {
    if slot >= INSTRUMENT_COUNT {
        Value::String("none".into())
    } else {
        Value::String(slot.to_string())
    }
}
