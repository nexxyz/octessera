use super::*;

#[test]
fn boot_sweep_golden_contract_has_strict_known_keys() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../resources/oled/boot-sweep-v1.json"
    ))
    .unwrap();
    super::schema_tests::validate_contract(&contract);
}

#[test]
fn every_golden_sample_matches_the_contract() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../resources/oled/boot-sweep-v1.json"
    ))
    .unwrap();
    super::schema_tests::validate_contract(&contract);
    for sample in contract["golden_samples"]["pixel_samples"]
        .as_array()
        .unwrap()
    {
        let frame_index = sample["frame_index"].as_u64().unwrap() as usize;
        let x = sample["x"].as_u64().unwrap() as usize;
        let y = sample["y"].as_u64().unwrap() as usize;
        let source_rgb565 = parse_rgb565(sample["source_rgb565"].as_str().unwrap());
        let expected_rgb565 = parse_rgb565(sample["expected_rgb565"].as_str().unwrap());
        let mut source = vec![0_u8; 128 * 128 * 2];
        let offset = (y * 128 + x) * 2;
        source[offset..offset + 2].copy_from_slice(&source_rgb565.to_be_bytes());
        assert_eq!(
            rgb565_at(&boot_sweep_frame_from(&source, frame_index), x, y),
            expected_rgb565,
            "{}",
            sample["sample_group"]
        );
    }
    for sample in contract["golden_samples"]["geometry_samples"]
        .as_array()
        .unwrap()
    {
        let frame_index = sample["frame_index"].as_u64().unwrap() as usize;
        let row_y = sample["row_y"].as_u64().unwrap() as i32;
        let origin = boot_sweep_bottom_row_origin(frame_index);
        let slant = row_y * BOOT_SWEEP_LEAN_NUMERATOR / BOOT_SWEEP_LEAN_DENOMINATOR;
        if let Some(expected) = sample.get("expected_bottom_row_origin_px") {
            assert_eq!(origin, expected.as_i64().unwrap() as i32);
        }
        assert_eq!(
            slant,
            sample["expected_slant_offset_px"].as_i64().unwrap() as i32
        );
        assert_eq!(origin + slant, sample["expected_slanted_origin_px"]);
    }
    assert_eq!(
        contract["golden_samples"]["endpoint_assertions"]["cycle"]["expected_frame_count"],
        BOOT_SWEEP_FRAMES
    );
    for (frame_name, frame_index) in [("frame_0", 0), ("frame_29", 29)] {
        let endpoint = &contract["golden_samples"]["endpoint_assertions"][frame_name];
        assert_eq!(
            endpoint["expected_bottom_row_origin_px"].as_i64().unwrap() as i32,
            boot_sweep_bottom_row_origin(frame_index)
        );
        assert_eq!(
            endpoint["expected_deadline_offset_ns"].as_u64().unwrap(),
            boot_sweep_deadline_offset_ns(frame_index)
        );
    }
}

fn parse_rgb565(value: &str) -> u16 {
    u16::from_str_radix(value, 16).unwrap()
}
