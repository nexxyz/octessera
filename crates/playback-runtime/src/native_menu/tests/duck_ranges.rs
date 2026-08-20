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
    menu.state.stack = vec![2, 1, 0, 1];

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
