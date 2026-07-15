use super::*;

pub(super) fn fx_bus_configs(buses: &[NativeFxBus]) -> Vec<NativeFxBusConfig> {
    buses
        .iter()
        .map(|bus| NativeFxBusConfig {
            name: bus.name.clone(),
            slot1_type: bus.slot1_type.clone(),
            slot1_params: bus.slot1_params.clone(),
            slot2_type: bus.slot2_type.clone(),
            slot2_params: bus.slot2_params.clone(),
            slot3_type: bus.slot3_type.clone(),
            slot3_params: bus.slot3_params.clone(),
            pan_pos: bus.pan_pos,
            volume_pct: bus.volume_pct,
            auto_name: bus.auto_name,
        })
        .collect()
}
