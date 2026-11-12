/// The instrument and sample slots of a `sample.choose:` browse row key, without its prefix.
pub(super) fn parse_sample_choose_slots(slots: &str) -> Option<(usize, usize)> {
    let (instrument_slot, sample_slot) = slots.split_once(':')?;
    Some((instrument_slot.parse().ok()?, sample_slot.parse().ok()?))
}

pub(super) fn parent_dir(dir: &str) -> String {
    let normalized = dir.replace('\\', "/");
    let mut layers = normalized
        .split('/')
        .filter(|layer| !layer.is_empty())
        .collect::<Vec<_>>();
    let _ = layers.pop();
    layers.join("/")
}

pub(super) fn same_sample_path(left: &str, right: &str) -> bool {
    left.replace('\\', "/") == right.replace('\\', "/")
}

pub(super) fn parse_named_slot_index(value: &str) -> Option<usize> {
    value
        .strip_prefix('I')
        .and_then(|rest| rest.split(':').next())
        .and_then(|number| number.parse::<usize>().ok())
        .and_then(|number| number.checked_sub(1))
}
