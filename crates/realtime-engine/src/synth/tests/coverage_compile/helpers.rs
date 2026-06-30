pub(super) use crate::synth::fx_params::{
    compile_fx_bus_params, DuckSource, FilterLfoKind, FxBusParams,
};
pub(super) use serde_json::json;
pub(super) use std::collections::BTreeMap;

pub(super) use crate::synth::FxBusSlotConfig;

pub(super) fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1.0e-6,
        "expected {expected}, got {actual}"
    );
}

pub(super) fn fx_config(
    kind: &str,
    params: BTreeMap<String, serde_json::Value>,
) -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: kind.to_string(),
        params,
    }
}
