pub(super) fn parse_sample_action(rest: &str) -> Result<(usize, usize, Option<String>), String> {
    let mut layers = rest.splitn(3, ':');
    let instrument_slot = layers
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid sample action `{rest}`"))?;
    let sample_slot = layers
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .ok_or_else(|| format!("invalid sample action `{rest}`"))?;
    Ok((
        instrument_slot,
        sample_slot,
        layers.next().map(str::to_string),
    ))
}

pub(super) fn parent_dir(dir: &str) -> String {
    let mut layers = dir
        .split('/')
        .filter(|layer| !layer.is_empty())
        .collect::<Vec<_>>();
    let _ = layers.pop();
    layers.join("/")
}

pub(super) fn parse_slot_index(value: &str) -> Option<usize> {
    if let Ok(index) = value.parse::<usize>() {
        return Some(if index == 0 { 0 } else { index - 1 });
    }
    value
        .strip_prefix('I')
        .and_then(|rest| rest.split(':').next())
        .and_then(|number| number.parse::<usize>().ok())
        .and_then(|number| number.checked_sub(1))
}
