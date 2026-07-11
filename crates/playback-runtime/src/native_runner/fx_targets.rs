use super::*;

pub(super) fn sparks_fx_cell_id(x: usize, y: usize) -> String {
    format!("momentary-fx:{x}:{y}")
}

pub(super) fn momentary_fx_target(value: &str) -> RuntimeMomentaryFxTarget {
    if let Some(index) = value
        .strip_prefix("fx_bus_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        return RuntimeMomentaryFxTarget::FxBus { index };
    }
    if let Some(index) = value
        .strip_prefix("instrument_")
        .and_then(|value| value.parse::<usize>().ok())
        .and_then(|value| value.checked_sub(1))
    {
        return RuntimeMomentaryFxTarget::Instrument { index };
    }
    RuntimeMomentaryFxTarget::Global
}
