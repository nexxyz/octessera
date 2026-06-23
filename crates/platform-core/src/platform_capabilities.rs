#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub grid_width: usize,
    pub grid_height: usize,
    pub part_count: usize,
    pub instrument_count: usize,
    pub sample_slot_count: usize,
    pub bus_count: usize,
    pub global_fx_slot_count: usize,
    pub aux_encoder_count: usize,
    pub touch_fx_max_concurrent: usize,
    pub max_active_bus_fx_slots: usize,
    pub max_active_global_fx_slots: usize,
    pub scan_section_counts: &'static [usize],
    pub pan_position_count: u8,
    pub oled_width: usize,
    pub oled_height: usize,
}

include!(concat!(
    env!("OUT_DIR"),
    "/platform_capabilities.generated.rs"
));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_capabilities_match_expected_hardware_profile() {
        assert_eq!(GRID_WIDTH, 8);
        assert_eq!(GRID_HEIGHT, 8);
        assert_eq!(PART_COUNT, 8);
        assert_eq!(INSTRUMENT_COUNT, 8);
        assert_eq!(SAMPLE_SLOT_COUNT, 8);
        assert_eq!(BUS_COUNT, 4);
        assert_eq!(GLOBAL_FX_SLOT_COUNT, 2);
        assert_eq!(AUX_ENCODER_COUNT, 3);
        assert_eq!(TOUCH_FX_MAX_CONCURRENT, 4);
        assert_eq!(MAX_ACTIVE_BUS_FX_SLOTS, 4);
        assert_eq!(MAX_ACTIVE_GLOBAL_FX_SLOTS, 1);
        assert_eq!(SCAN_SECTION_COUNTS, &[1, 2, 4, 8]);
        assert_eq!(PAN_POSITION_COUNT, 33);
        assert_eq!(OLED_WIDTH, 128);
        assert_eq!(OLED_HEIGHT, 128);
        assert_eq!(PLATFORM_CAPABILITIES.grid_width, GRID_WIDTH);
    }
}
