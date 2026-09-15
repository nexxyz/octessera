use super::*;

#[test]
pub(crate) fn duck_menu_exposes_canonical_parameter_ranges() {
    let mut config = config();
    config.fx_buses[0].slot2_params = serde_json::json!({
        "source": "I2",
        "threshold": 1.0,
        "amountPct": 100,
        "attackMs": 500,
        "releaseMs": 5000
    });
    let mut menu = NativeMenuModel::new(config);
    assert!(menu.focus_item_key("mixer.buses.0.slot2.params.threshold"));

    for (label, value, min, max) in [
        ("Threshold", 100, 0, 100),
        ("Amount %", 100, 0, 100),
        ("Attack", 500, 1, 500),
        ("Release", 5000, 1, 5000),
    ] {
        let item = menu
            .current_siblings()
            .iter()
            .find(|item| item.label == label)
            .unwrap_or_else(|| panic!("missing Duck menu item {label}"));
        assert!(matches!(
            item.value,
            NativeMenuValue::Number {
                value: actual,
                min: actual_min,
                max: actual_max,
                ..
            } if actual == value && actual_min == min && actual_max == max
        ));
    }
}

#[test]
pub(crate) fn duck_menu_places_source_tap_after_source_and_displays_labels() {
    let mut config = config();
    config.fx_buses[0].slot1_type = "duck".into();
    config.fx_buses[0].slot1_params = serde_json::json!({
        "source": "I2",
        "sourceTap": "post"
    });
    let mut menu = NativeMenuModel::new(config);

    assert!(menu.focus_item_key("mixer.buses.0.slot1.params.sourceTap"));
    let source_index = menu
        .current_siblings()
        .iter()
        .position(|item| item.label == "Source")
        .expect("Duck Source menu item");
    let source_tap = menu
        .current_siblings()
        .get(source_index + 1)
        .expect("Duck Source Tap menu item");
    assert_eq!(source_tap.label, "Source Tap");
    assert_eq!(
        menu.value_for_key("mixer.buses.0.slot1.params.sourceTap"),
        Some("post".into())
    );
    assert!(matches!(
        &source_tap.value,
        NativeMenuValue::Enum { options, selected }
            if options == &["pre".to_string(), "post".to_string()] && *selected == 1
    ));
    assert!(menu
        .snapshot()
        .lines
        .iter()
        .any(|line| line == "> Source Tap Post"));
}
