use super::NativeRunner;
use platform_core::BUS_FX_WARNING_SLOT_COUNT;

impl NativeRunner {
    pub(super) fn active_bus_fx_slot_count(&self) -> usize {
        self.fx_buses
            .iter()
            .map(|bus| {
                usize::from(bus.slot1_type != "none")
                    + usize::from(bus.slot2_type != "none")
                    + usize::from(bus.slot3_type != "none")
            })
            .sum::<usize>()
    }

    pub(super) fn warn_if_bus_fx_over_budget(&mut self) {
        let active_fx_slots = self.active_bus_fx_slot_count();
        if active_fx_slots > BUS_FX_WARNING_SLOT_COUNT {
            self.show_toast(format!(
                "FX budget warning ({}/{})",
                active_fx_slots, BUS_FX_WARNING_SLOT_COUNT
            ));
        }
    }
}
