use super::*;

pub(super) fn fx_bus_configs(buses: &[NativeFxBus]) -> Vec<NativeFxBusConfig> {
    buses
        .iter()
        .map(|bus| {
            let [slot1, slot2, slot3] = &bus.slots;
            NativeFxBusConfig {
                name: bus.name.clone(),
                slot1_type: slot1.fx_type.clone(),
                slot1_params: slot1.params.clone(),
                slot2_type: slot2.fx_type.clone(),
                slot2_params: slot2.params.clone(),
                slot3_type: slot3.fx_type.clone(),
                slot3_params: slot3.params.clone(),
                pan_pos: bus.pan_pos,
                volume_pct: bus.volume_pct,
                auto_name: bus.auto_name,
            }
        })
        .collect()
}
