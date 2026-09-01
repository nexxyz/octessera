#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlatformCapabilities {
    pub grid_width: usize,
    pub grid_height: usize,
    pub layer_count: usize,
    pub instrument_count: usize,
    pub sample_slot_count: usize,
    pub audio_output_buffer_frames: usize,
    pub audio_render_quantum_frames: usize,
    pub synth_voice_lane_capacity: usize,
    pub sample_voice_lane_capacity: usize,
    pub bus_count: usize,
    pub global_fx_slot_count: usize,
    pub aux_encoder_count: usize,
    pub sparks_fx_max_concurrent: usize,
    pub bus_fx_warning_slot_count: usize,
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
        assert_eq!(LAYER_COUNT, 8);
        assert_eq!(INSTRUMENT_COUNT, 8);
        assert_eq!(SAMPLE_SLOT_COUNT, 8);
        assert_eq!(AUDIO_OUTPUT_BUFFER_FRAMES, 256);
        assert_eq!(AUDIO_RENDER_QUANTUM_FRAMES, 128);
        assert_eq!(SYNTH_VOICE_LANE_CAPACITY, 64);
        assert_eq!(SAMPLE_VOICE_LANE_CAPACITY, 64);
        assert_eq!(BUS_COUNT, 4);
        assert_eq!(GLOBAL_FX_SLOT_COUNT, 2);
        assert_eq!(AUX_ENCODER_COUNT, 3);
        assert_eq!(SPARKS_FX_MAX_CONCURRENT, 2);
        assert_eq!(BUS_FX_WARNING_SLOT_COUNT, 12);
        assert_eq!(SCAN_SECTION_COUNTS, &[1, 2, 4, 8]);
        assert_eq!(PAN_POSITION_COUNT, 33);
        assert_eq!(OLED_WIDTH, 128);
        assert_eq!(OLED_HEIGHT, 128);
        assert_eq!(PLATFORM_CAPABILITIES.grid_width, GRID_WIDTH);
        assert_eq!(
            PLATFORM_CAPABILITIES.audio_output_buffer_frames,
            AUDIO_OUTPUT_BUFFER_FRAMES
        );
        assert_eq!(
            PLATFORM_CAPABILITIES.audio_render_quantum_frames,
            AUDIO_RENDER_QUANTUM_FRAMES
        );
        assert_eq!(
            PLATFORM_CAPABILITIES.synth_voice_lane_capacity,
            SYNTH_VOICE_LANE_CAPACITY
        );
        assert_eq!(
            PLATFORM_CAPABILITIES.sample_voice_lane_capacity,
            SAMPLE_VOICE_LANE_CAPACITY
        );
    }
}
