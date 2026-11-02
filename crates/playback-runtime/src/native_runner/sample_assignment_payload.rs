use super::*;

pub(super) fn sample_assignments_payload(assignments: &[NativeSampleAssignment]) -> Value {
    Value::Array(
        assignments
            .iter()
            .map(|assignment| {
                let mut tile = json!({
                    "x": assignment.x,
                    "y": assignment.y,
                    "sampleSlot": assignment.sample_slot,
                    "level": assignment.level,
                });
                if assignment.tune_semis != 0 {
                    tile["tuneSemis"] = assignment.tune_semis.into();
                }
                if assignment.tune_cents != 0 {
                    tile["tuneCents"] = assignment.tune_cents.into();
                }
                tile
            })
            .collect(),
    )
}

pub(super) fn sample_assignment_from_payload(value: &Value) -> Option<NativeSampleAssignment> {
    let level = value
        .get("level")
        .and_then(Value::as_str)
        .and_then(|level| {
            if matches!(level, "high" | "medium" | "low") {
                Some(level.to_string())
            } else {
                None
            }
        });
    Some(NativeSampleAssignment {
        x: (value.get("x")?.as_u64()? as usize).min(GRID_WIDTH - 1),
        y: (value.get("y")?.as_u64()? as usize).min(GRID_HEIGHT - 1),
        sample_slot: (value.get("sampleSlot")?.as_u64()? as usize).min(7),
        level,
        tune_semis: tile_offset(value, "tuneSemis", 24),
        tune_cents: tile_offset(value, "tuneCents", 100),
    })
}

pub(super) fn tile_offset(value: &Value, key: &str, limit: i64) -> i8 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map_or(0, |offset| offset.clamp(-limit, limit) as i8)
}
