use super::*;

pub(super) fn apply_trigger_probability_map_payload(
    target: &mut TileProbabilityMap,
    map: &[String],
) {
    for (tile, value) in target.iter_mut().zip(map) {
        if let Some(value) = TileProbability::from_wire_name(value) {
            *tile = value;
        }
    }
}
