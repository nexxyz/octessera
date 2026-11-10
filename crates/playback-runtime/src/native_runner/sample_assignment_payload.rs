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

pub(super) fn tile_offset(value: &Value, key: &str, limit: i64) -> i8 {
    value
        .get(key)
        .and_then(Value::as_i64)
        .map_or(0, |offset| offset.clamp(-limit, limit) as i8)
}
