use super::*;

pub(super) fn validate(value: &serde_json::Value) {
    let wire = object(value);
    assert_exact_keys(
        wire,
        &[
            "spi_clock_hz",
            "address_command_bytes_per_cycle",
            "frame_data_bytes",
            "conservative_command_data_bytes_per_frame",
            "utilization_limit_percent",
            "cycle_frame_count",
            "cycle_payload_duration_ns",
            "cycle_utilization_percent",
            "utilization_headroom_to_limit_percent",
            "accepted_frame_count",
            "rejected_frame_count",
        ],
    );
    assert_eq!(wire["spi_clock_hz"], 16_000_000_u64);
    assert_eq!(wire["address_command_bytes_per_cycle"], 7);
    assert_eq!(wire["frame_data_bytes"], 32_768);
    assert_eq!(wire["conservative_command_data_bytes_per_frame"], 32_775);
    assert_eq!(wire["utilization_limit_percent"], 80);
    assert_eq!(wire["cycle_frame_count"], 30);
    assert_eq!(wire["cycle_payload_duration_ns"], 491_625_000_u64);
    assert_eq!(wire["cycle_utilization_percent"], 40.96875);
    assert_eq!(wire["utilization_headroom_to_limit_percent"], 39.03125);
    assert_eq!(wire["accepted_frame_count"], 58);
    assert_eq!(wire["rejected_frame_count"], 59);

    let spi_clock_hz = wire["spi_clock_hz"].as_u64().unwrap() as u128;
    let conservative_bytes = wire["conservative_command_data_bytes_per_frame"]
        .as_u64()
        .unwrap() as u128;
    let limit_percent = wire["utilization_limit_percent"].as_u64().unwrap() as u128;
    let under_limit = |frame_count: usize| {
        (frame_count as u128 * conservative_bytes * 8 * 100 * 1_000_000_000)
            <= spi_clock_hz * 1_200_000_000_u128 * limit_percent
    };
    assert!(under_limit(58));
    assert!(!under_limit(59));
    let payload_duration_ns = 30_u128 * conservative_bytes * 8 * 1_000_000_000 / spi_clock_hz;
    assert_eq!(payload_duration_ns, 491_625_000);
}
