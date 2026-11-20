use super::super::*;

pub(in crate::native_runner) fn apply_trigger_probability_map_payload(
    target: &mut TileProbabilityMap,
    map: &[String],
) {
    for (tile, value) in target.iter_mut().zip(map) {
        if let Some(value) = TileProbability::from_wire_name(value) {
            *tile = value;
        }
    }
}
