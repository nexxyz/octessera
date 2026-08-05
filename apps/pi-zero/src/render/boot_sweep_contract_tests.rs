use super::*;

#[test]
fn boot_sweep_golden_contract_has_strict_known_keys() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../resources/oled/boot-sweep-v1.json"
    ))
    .unwrap();
    validate_contract(&contract);
}

#[test]
fn every_golden_sample_matches_the_contract() {
    let contract: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../resources/oled/boot-sweep-v1.json"
    ))
    .unwrap();
    validate_contract(&contract);
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
    for (frame_name, frame_index) in [("frame_0", 0), ("frame_23", 23)] {
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

fn assert_exact_keys(object: &serde_json::Map<String, serde_json::Value>, expected: &[&str]) {
    assert_eq!(
        object.len(),
        expected.len(),
        "keys={:?} expected={expected:?}",
        object.keys().collect::<Vec<_>>()
    );
    assert!(object.keys().all(|key| expected.contains(&key.as_str())));
}

fn validate_contract(contract: &serde_json::Value) {
    let root = object(contract);
    assert_exact_keys(
        root,
        &[
            "schema_version",
            "strictness",
            "coordinate_space",
            "source_pixel_rule",
            "bands",
            "slant",
            "timing",
            "travel",
            "golden_samples",
        ],
    );
    assert_eq!(contract["schema_version"], 1);
    let strictness = object(&contract["strictness"]);
    assert_exact_keys(strictness, &["unknown_keys", "missing_keys"]);
    assert_eq!(strictness["unknown_keys"], "reject");
    assert_eq!(strictness["missing_keys"], "reject");
    let coordinate = object(&contract["coordinate_space"]);
    assert_exact_keys(
        coordinate,
        &[
            "orientation",
            "width_px",
            "height_px",
            "x_direction",
            "y_direction",
        ],
    );
    assert_eq!(coordinate["orientation"], "physical_post_rotation");
    assert_eq!(coordinate["width_px"], 128);
    assert_eq!(coordinate["height_px"], 128);
    assert_eq!(coordinate["x_direction"], "rightward");
    assert_eq!(coordinate["y_direction"], "bottom_to_top");
    let source_rule = object(&contract["source_pixel_rule"]);
    assert_exact_keys(
        source_rule,
        &[
            "pixel_format",
            "recolor_match_rgb565",
            "match_action",
            "non_match_action",
        ],
    );
    assert_eq!(source_rule["pixel_format"], "rgb565_hex");
    assert_eq!(source_rule["recolor_match_rgb565"], "FFFF");
    assert_eq!(source_rule["match_action"], "recolor");
    assert_eq!(source_rule["non_match_action"], "preserve");
    validate_bands(&contract["bands"]);
    validate_slant(&contract["slant"]);
    validate_timing(&contract["timing"]);
    validate_travel(&contract["travel"]);
    validate_golden_keys(&contract["golden_samples"]);
}

fn validate_bands(value: &serde_json::Value) {
    let bands = object(value);
    assert_exact_keys(
        bands,
        &[
            "order",
            "band_count",
            "band_width_px",
            "train_width_px",
            "items",
        ],
    );
    assert_eq!(bands["order"], "increasing_x_travel_axis");
    assert_eq!(bands["band_count"], 4);
    assert_eq!(bands["band_width_px"], 8);
    assert_eq!(bands["train_width_px"], 32);
    let expected_items = [
        (0, "cyan", "07FF"),
        (1, "yellow", "FFE0"),
        (2, "green", "07E0"),
        (3, "magenta", "F81F"),
    ];
    let items = bands["items"].as_array().unwrap();
    assert_eq!(items.len(), expected_items.len());
    for (item, (index, name, color)) in items.iter().zip(expected_items) {
        let item = object(item);
        assert_exact_keys(item, &["band_index", "name", "color_rgb565", "width_px"]);
        assert_eq!(item["band_index"], index);
        assert_eq!(item["name"], name);
        assert_eq!(item["color_rgb565"], color);
        assert_eq!(item["width_px"], 8);
    }
}

fn validate_slant(value: &serde_json::Value) {
    let slant = object(value);
    assert_exact_keys(
        slant,
        &[
            "offset_formula",
            "offset_numerator_px",
            "offset_denominator_rows",
            "row_y_min",
            "row_y_max",
            "bottom_row_offset_px",
            "top_row_offset_px",
        ],
    );
    assert_eq!(slant["offset_formula"], "floor(row_y * 8 / 127)");
    assert_eq!(slant["offset_numerator_px"], 8);
    assert_eq!(slant["offset_denominator_rows"], 127);
    assert_eq!(slant["row_y_min"], 0);
    assert_eq!(slant["row_y_max"], 127);
    assert_eq!(slant["bottom_row_offset_px"], 0);
    assert_eq!(slant["top_row_offset_px"], 8);
}

fn validate_timing(value: &serde_json::Value) {
    let timing = object(value);
    assert_exact_keys(
        timing,
        &[
            "cycle_duration_ns",
            "frames_per_cycle",
            "frame_index_min",
            "frame_index_max",
            "frame_deadline_offset_formula",
            "frame_deadline_reference",
            "scheduling_mode",
            "cumulative_sleep_scheduling",
        ],
    );
    assert_eq!(timing["cycle_duration_ns"], 1_000_000_000_u64);
    assert_eq!(timing["frames_per_cycle"], 24);
    assert_eq!(timing["frame_index_min"], 0);
    assert_eq!(timing["frame_index_max"], 23);
    assert_eq!(
        timing["frame_deadline_offset_formula"],
        "floor(frame_index * 1000000000 / 24)"
    );
    assert_eq!(timing["frame_deadline_reference"], "absolute_cycle_start");
    assert_eq!(timing["scheduling_mode"], "absolute_deadline");
    assert_eq!(timing["cumulative_sleep_scheduling"], false);
}

fn validate_travel(value: &serde_json::Value) {
    let travel = object(value);
    assert_exact_keys(
        travel,
        &[
            "bottom_row_origin_formula",
            "frame_index_min",
            "frame_index_max",
            "start_bottom_row_origin_px",
            "end_bottom_row_origin_px",
            "travel_distance_px",
            "pixel_membership",
            "endpoint_blank_frames",
            "wrap",
        ],
    );
    assert_eq!(
        travel["bottom_row_origin_formula"],
        "-40 + floor(frame_index * 168 / 23)"
    );
    assert_eq!(travel["frame_index_min"], 0);
    assert_eq!(travel["frame_index_max"], 23);
    assert_eq!(travel["start_bottom_row_origin_px"], -40);
    assert_eq!(travel["end_bottom_row_origin_px"], 128);
    assert_eq!(travel["travel_distance_px"], 168);
    let membership = object(&travel["pixel_membership"]);
    assert_exact_keys(
        membership,
        &[
            "slanted_origin_formula",
            "local_x_formula",
            "in_band_condition",
            "band_index_formula",
            "outside_action",
            "inside_action",
        ],
    );
    assert_eq!(
        membership["slanted_origin_formula"],
        "bottom_row_origin + floor(row_y * 8 / 127)"
    );
    assert_eq!(membership["local_x_formula"], "x - slanted_origin");
    assert_eq!(membership["in_band_condition"], "0 <= local_x < 32");
    assert_eq!(membership["band_index_formula"], "floor(local_x / 8)");
    assert_eq!(membership["outside_action"], "preserve_source");
    assert_eq!(
        membership["inside_action"],
        "recolor_only_if_source_is_FFFF"
    );
    let endpoint = object(&travel["endpoint_blank_frames"]);
    assert_exact_keys(
        endpoint,
        &[
            "frame_indices",
            "intentional",
            "extra_pause_inserted",
            "frame_0",
            "frame_23",
        ],
    );
    assert_eq!(endpoint["frame_indices"], serde_json::json!([0, 23]));
    assert_eq!(endpoint["intentional"], true);
    assert_eq!(endpoint["extra_pause_inserted"], false);
    assert_exact_keys(
        object(&endpoint["frame_0"]),
        &[
            "bottom_row_origin_px",
            "top_row_origin_px",
            "rightmost_train_pixel_px",
            "fully_offscreen_left",
        ],
    );
    assert_eq!(endpoint["frame_0"]["bottom_row_origin_px"], -40);
    assert_eq!(endpoint["frame_0"]["top_row_origin_px"], -32);
    assert_eq!(endpoint["frame_0"]["rightmost_train_pixel_px"], -1);
    assert_eq!(endpoint["frame_0"]["fully_offscreen_left"], true);
    assert_exact_keys(
        object(&endpoint["frame_23"]),
        &[
            "bottom_row_origin_px",
            "top_row_origin_px",
            "leftmost_train_pixel_px",
            "fully_offscreen_right",
        ],
    );
    assert_eq!(endpoint["frame_23"]["bottom_row_origin_px"], 128);
    assert_eq!(endpoint["frame_23"]["top_row_origin_px"], 136);
    assert_eq!(endpoint["frame_23"]["leftmost_train_pixel_px"], 128);
    assert_eq!(endpoint["frame_23"]["fully_offscreen_right"], true);
    let wrap = object(&travel["wrap"]);
    assert_exact_keys(
        wrap,
        &[
            "after_frame_index",
            "next_frame_index",
            "extra_pause_inserted",
        ],
    );
    assert_eq!(wrap["after_frame_index"], 23);
    assert_eq!(wrap["next_frame_index"], 0);
    assert_eq!(wrap["extra_pause_inserted"], false);
}

fn validate_golden_keys(value: &serde_json::Value) {
    let golden = object(value);
    assert_exact_keys(
        golden,
        &["pixel_samples", "geometry_samples", "endpoint_assertions"],
    );
    for sample in golden["pixel_samples"].as_array().unwrap() {
        assert_exact_keys(
            object(sample),
            &[
                "sample_group",
                "frame_index",
                "x",
                "y",
                "source_rgb565",
                "expected_rgb565",
            ],
        );
    }
    for sample in golden["geometry_samples"].as_array().unwrap() {
        let keys = if sample.get("expected_bottom_row_origin_px").is_some() {
            &[
                "sample_group",
                "frame_index",
                "row_y",
                "expected_bottom_row_origin_px",
                "expected_slant_offset_px",
                "expected_slanted_origin_px",
            ][..]
        } else {
            &[
                "sample_group",
                "frame_index",
                "row_y",
                "expected_slant_offset_px",
                "expected_slanted_origin_px",
            ][..]
        };
        assert_exact_keys(object(sample), keys);
    }
    let endpoints = object(&golden["endpoint_assertions"]);
    assert_exact_keys(endpoints, &["frame_0", "frame_23", "cycle"]);
    assert_exact_keys(
        object(&endpoints["frame_0"]),
        &[
            "expected_bottom_row_origin_px",
            "expected_deadline_offset_ns",
            "expected_next_frame_index",
            "fully_offscreen_left",
        ],
    );
    assert_eq!(endpoints["frame_0"]["expected_bottom_row_origin_px"], -40);
    assert_eq!(endpoints["frame_0"]["expected_deadline_offset_ns"], 0);
    assert_eq!(endpoints["frame_0"]["expected_next_frame_index"], 1);
    assert_eq!(endpoints["frame_0"]["fully_offscreen_left"], true);
    assert_exact_keys(
        object(&endpoints["frame_23"]),
        &[
            "expected_bottom_row_origin_px",
            "expected_deadline_offset_ns",
            "expected_next_frame_index",
            "fully_offscreen_right",
        ],
    );
    assert_eq!(endpoints["frame_23"]["expected_bottom_row_origin_px"], 128);
    assert_eq!(
        endpoints["frame_23"]["expected_deadline_offset_ns"],
        958_333_333_u64
    );
    assert_eq!(endpoints["frame_23"]["expected_next_frame_index"], 0);
    assert_eq!(endpoints["frame_23"]["fully_offscreen_right"], true);
    assert_exact_keys(
        object(&endpoints["cycle"]),
        &[
            "expected_frame_count",
            "expected_first_frame_index",
            "expected_last_frame_index",
            "expected_wrap_frame_index",
            "extra_pause_inserted",
        ],
    );
    assert_eq!(endpoints["cycle"]["expected_frame_count"], 24);
    assert_eq!(endpoints["cycle"]["expected_first_frame_index"], 0);
    assert_eq!(endpoints["cycle"]["expected_last_frame_index"], 23);
    assert_eq!(endpoints["cycle"]["expected_wrap_frame_index"], 0);
    assert_eq!(endpoints["cycle"]["extra_pause_inserted"], false);
}

fn object(value: &serde_json::Value) -> &serde_json::Map<String, serde_json::Value> {
    value.as_object().unwrap()
}

fn parse_rgb565(value: &str) -> u16 {
    u16::from_str_radix(value, 16).unwrap()
}
