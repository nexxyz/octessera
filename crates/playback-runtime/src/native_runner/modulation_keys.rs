pub(super) fn parse_sense_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("parts.")?;
    let (index, field) = rest.split_once(".l2.")?;
    Some((index.parse::<usize>().ok()?, field))
}

pub(super) fn parse_fx_bus_binding_key(key: &str) -> Option<(usize, &str, &str)> {
    let rest = key.strip_prefix("mixer.buses.")?;
    let (index, field) = rest.split_once('.')?;
    let field = if let Some(field) = field.strip_prefix("slot1.") {
        ("slot1", field)
    } else if let Some(field) = field.strip_prefix("slot2.") {
        ("slot2", field)
    } else {
        return Some((index.parse::<usize>().ok()?, "bus", field));
    };
    Some((index.parse::<usize>().ok()?, field.0, field.1))
}

pub(super) fn parse_global_fx_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("mixer.master.slots.")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse::<usize>().ok()?, field))
}

pub(super) fn parse_instrument_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("instruments.")?;
    let (index, field) = rest.split_once('.')?;
    Some((index.parse::<usize>().ok()?, field))
}

pub(super) fn parse_part_behavior_config_binding_key(key: &str) -> Option<(usize, &str)> {
    let rest = key.strip_prefix("parts.")?;
    let (index, field) = rest.split_once(".l1.behaviorConfig.")?;
    Some((index.parse::<usize>().ok()?, field))
}
