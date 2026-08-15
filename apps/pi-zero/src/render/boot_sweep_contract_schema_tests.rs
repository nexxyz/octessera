use super::*;

#[path = "boot_sweep_wire_budget_tests.rs"]
mod wire_budget_tests;
fn assert_exact_keys(object: &serde_json::Map<String, serde_json::Value>, expected: &[&str]) {
    assert_eq!(
        object.len(),
        expected.len(),
        "keys={:?} expected={expected:?}",
        object.keys().collect::<Vec<_>>()
    );
    assert!(object.keys().all(|key| expected.contains(&key.as_str())));
}

pub(super) fn validate_contract(contract: &serde_json::Value) {
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
            "wire_budget",
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
            "physical_motion",
        ],
    );
    assert_eq!(coordinate["orientation"], "physical_post_rotation");
    assert_eq!(coordinate["width_px"], 128);
    assert_eq!(coordinate["height_px"], 128);
    assert_eq!(coordinate["x_direction"], "leftward_controller_axis");
    assert_eq!(coordinate["y_direction"], "bottom_to_top");
    assert_eq!(
        coordinate["physical_motion"],
        "left_to_right_after_mounted_ssd1351_remap"
    );
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
    wire_budget_tests::validate(&contract["wire_budget"]);
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
            "separator_width_px",
            "separator_color_rgb565",
            "separator_semantics",
            "train_width_px",
            "items",
        ],
    );
    assert_eq!(bands["order"], "increasing_x_travel_axis");
    assert_eq!(bands["band_count"], 4);
    assert_eq!(bands["band_width_px"], 8);
    assert_eq!(bands["separator_width_px"], 4);
    assert_eq!(bands["separator_color_rgb565"], "FFFF");
    assert_eq!(
        bands["separator_semantics"],
        "white_separator_before_each_color_band"
    );
    assert_eq!(bands["train_width_px"], 48);
    let expected_items = [
        (0, "magenta", "F81F"),
        (1, "green", "07E0"),
        (2, "yellow", "FFE0"),
        (3, "cyan", "07FF"),
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
    assert_eq!(slant["offset_formula"], "-row_y");
    assert_eq!(slant["offset_numerator_px"], -1);
    assert_eq!(slant["offset_denominator_rows"], 1);
    assert_eq!(slant["row_y_min"], 0);
    assert_eq!(slant["row_y_max"], 127);
    assert_eq!(slant["bottom_row_offset_px"], 0);
    assert_eq!(slant["top_row_offset_px"], -127);
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
    assert_eq!(timing["cycle_duration_ns"], 1_200_000_000_u64);
    assert_eq!(timing["frames_per_cycle"], 30);
    assert_eq!(timing["frame_index_min"], 0);
    assert_eq!(timing["frame_index_max"], 29);
    assert_eq!(
        timing["frame_deadline_offset_formula"],
        "floor(frame_index * 1200000000 / 30)"
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
        "255 - floor(frame_index * 303 / 29)"
    );
    let first_frame = 0;
    let last_frame = BOOT_SWEEP_FRAMES - 1;
    assert_eq!(travel["frame_index_min"], first_frame);
    assert_eq!(travel["frame_index_max"], last_frame);
    let start_origin = boot_sweep_bottom_row_origin(first_frame);
    let end_origin = boot_sweep_bottom_row_origin(last_frame);
    assert_eq!(travel["start_bottom_row_origin_px"], start_origin);
    assert_eq!(travel["end_bottom_row_origin_px"], end_origin);
    assert_eq!(travel["travel_distance_px"], end_origin - start_origin);
    let membership = object(&travel["pixel_membership"]);
    assert_exact_keys(
        membership,
        &[
            "slanted_origin_formula",
            "local_x_formula",
            "in_band_condition",
            "band_index_formula",
            "separator_condition",
            "separator_action",
            "outside_action",
            "inside_action",
        ],
    );
    assert_eq!(
        membership["slanted_origin_formula"],
        "bottom_row_origin - row_y"
    );
    assert_eq!(membership["local_x_formula"], "x - slanted_origin");
    assert_eq!(membership["in_band_condition"], "0 <= local_x < 48");
    assert_eq!(membership["band_index_formula"], "floor(local_x / 12)");
    assert_eq!(membership["separator_condition"], "local_x % 12 < 4");
    assert_eq!(membership["separator_action"], "white_separator");
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
            "frame_29",
        ],
    );
    assert_eq!(endpoint["frame_indices"], serde_json::json!([0, 29]));
    assert_eq!(endpoint["intentional"], true);
    assert_eq!(endpoint["extra_pause_inserted"], false);
    assert_exact_keys(
        object(&endpoint["frame_0"]),
        &[
            "bottom_row_origin_px",
            "top_row_origin_px",
            "leftmost_train_pixel_px",
            "fully_offscreen_right",
        ],
    );
    let frame_0_top = expected_slanted_origin(first_frame, 127);
    let frame_0_leftmost = frame_0_top;
    assert_eq!(endpoint["frame_0"]["bottom_row_origin_px"], start_origin);
    assert_eq!(endpoint["frame_0"]["top_row_origin_px"], frame_0_top);
    assert_eq!(
        endpoint["frame_0"]["leftmost_train_pixel_px"],
        frame_0_leftmost
    );
    assert_eq!(
        endpoint["frame_0"]["fully_offscreen_right"],
        frame_0_leftmost >= 128
    );
    assert_exact_keys(
        object(&endpoint["frame_29"]),
        &[
            "bottom_row_origin_px",
            "top_row_origin_px",
            "rightmost_train_pixel_px",
            "fully_offscreen_left",
        ],
    );
    let frame_29_top = expected_slanted_origin(last_frame, 127);
    let frame_29_rightmost = end_origin + BOOT_SWEEP_TRAIN_WIDTH - 1;
    assert_eq!(endpoint["frame_29"]["bottom_row_origin_px"], end_origin);
    assert_eq!(endpoint["frame_29"]["top_row_origin_px"], frame_29_top);
    assert_eq!(
        endpoint["frame_29"]["rightmost_train_pixel_px"],
        frame_29_rightmost
    );
    assert_eq!(
        endpoint["frame_29"]["fully_offscreen_left"],
        frame_29_rightmost < 0
    );
    let wrap = object(&travel["wrap"]);
    assert_exact_keys(
        wrap,
        &[
            "after_frame_index",
            "next_frame_index",
            "extra_pause_inserted",
        ],
    );
    assert_eq!(wrap["after_frame_index"], 29);
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
    let mut pixel_identities = std::collections::HashSet::new();
    for sample in golden["pixel_samples"].as_array().unwrap() {
        let identity = (
            sample["frame_index"].as_u64().unwrap(),
            sample["x"].as_u64().unwrap(),
            sample["y"].as_u64().unwrap(),
            sample["source_rgb565"].as_str().unwrap(),
        );
        assert!(
            pixel_identities.insert(identity),
            "duplicate pixel golden sample identity: {identity:?}"
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
    let mut geometry_identities = std::collections::HashSet::new();
    for sample in golden["geometry_samples"].as_array().unwrap() {
        let identity = (
            sample["frame_index"].as_u64().unwrap(),
            sample["row_y"].as_u64().unwrap(),
        );
        assert!(
            geometry_identities.insert(identity),
            "duplicate geometry golden sample identity: {identity:?}"
        );
    }
    let endpoints = object(&golden["endpoint_assertions"]);
    assert_exact_keys(endpoints, &["frame_0", "frame_29", "cycle"]);
    assert_exact_keys(
        object(&endpoints["frame_0"]),
        &[
            "expected_bottom_row_origin_px",
            "expected_deadline_offset_ns",
            "expected_next_frame_index",
            "fully_offscreen_right",
        ],
    );
    for (frame_name, frame_index) in [("frame_0", 0), ("frame_29", BOOT_SWEEP_FRAMES - 1)] {
        let endpoint = object(&endpoints[frame_name]);
        assert_eq!(
            endpoint["expected_bottom_row_origin_px"],
            boot_sweep_bottom_row_origin(frame_index)
        );
        assert_eq!(
            endpoint["expected_deadline_offset_ns"],
            boot_sweep_deadline_offset_ns(frame_index)
        );
        assert_eq!(
            endpoint["expected_next_frame_index"],
            (frame_index + 1) % BOOT_SWEEP_FRAMES
        );
    }
    assert_eq!(endpoints["frame_0"]["fully_offscreen_right"], true);
    assert_exact_keys(
        object(&endpoints["frame_29"]),
        &[
            "expected_bottom_row_origin_px",
            "expected_deadline_offset_ns",
            "expected_next_frame_index",
            "fully_offscreen_left",
        ],
    );
    assert_eq!(endpoints["frame_29"]["fully_offscreen_left"], true);
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
    assert_eq!(
        endpoints["cycle"]["expected_frame_count"],
        BOOT_SWEEP_FRAMES
    );
    assert_eq!(endpoints["cycle"]["expected_first_frame_index"], 0);
    assert_eq!(
        endpoints["cycle"]["expected_last_frame_index"],
        BOOT_SWEEP_FRAMES - 1
    );
    assert_eq!(endpoints["cycle"]["expected_wrap_frame_index"], 0);
    assert_eq!(endpoints["cycle"]["extra_pause_inserted"], false);
}

fn expected_slanted_origin(frame_index: usize, row_y: i32) -> i32 {
    boot_sweep_bottom_row_origin(frame_index)
        + row_y * BOOT_SWEEP_LEAN_NUMERATOR / BOOT_SWEEP_LEAN_DENOMINATOR
}

fn object(value: &serde_json::Value) -> &serde_json::Map<String, serde_json::Value> {
    value.as_object().unwrap()
}
