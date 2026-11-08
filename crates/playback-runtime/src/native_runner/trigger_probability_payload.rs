use super::*;

pub(super) fn apply_trigger_probability_map_payload(
    target: &mut TileProbabilityMap,
    map: &[Value],
) {
    for (tile, value) in target.iter_mut().zip(map) {
        if let Some(value) = value.as_str().and_then(TileProbability::from_wire_name) {
            *tile = value;
        }
    }
}
