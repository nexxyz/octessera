use super::{
    action_item, enum_item, enum_item_from_strings, group, keyed_group, number_item,
    selected_index, xy_pad_items, NativeMenuAction, NativeMenuConfig, NativeMenuItem,
    NativeMenuValue,
};
use crate::native_menu::binding_picker::sparks_fx_targets;

pub(super) fn sparks_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "4: Sparks",
        vec![
            keyed_group("Mix", "sparks.page.mix", vec![]),
            keyed_group("Pan", "sparks.page.pan", vec![]),
            keyed_group(
                "FX",
                "sparks.page.fx",
                sparks_fx_page_items(config)
                    .into_iter()
                    .chain([group("Aux Map", sparks_aux_map_items(config))])
                    .collect(),
            ),
            keyed_group("Trigger Gate", "sparks.page.trigger-gate", vec![]),
            keyed_group("Transpose", "sparks.page.transpose", vec![]),
            keyed_group("XY", "sparks.page.xy", xy_pad_items(config)),
        ],
    )
}

fn sparks_aux_map_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    sparks_fx_page_items(config)
        .into_iter()
        .filter(|item| {
            item.key
                .as_deref()
                .is_some_and(|key| key.starts_with("sparks.fx.params."))
                || matches!(
                    &item.value,
                    NativeMenuValue::Action(NativeMenuAction::PlatformEffect(effect))
                        if effect == "sparks.fx.map"
                )
        })
        .map(aux_map_path_item)
        .collect()
}

fn aux_map_path_item(mut item: NativeMenuItem) -> NativeMenuItem {
    if let Some(key) = item.key.as_deref() {
        item.label = format!("{}: {key}", item.label);
    } else if matches!(
        &item.value,
        NativeMenuValue::Action(NativeMenuAction::PlatformEffect(effect)) if effect == "sparks.fx.map"
    ) {
        item.label = format!("{}: sparks.fx.map", item.label);
    }
    item
}

pub(super) fn sparks_fx_page_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let fx_types = vec!["none", "stutter", "freeze", "filter_sweep", "pitch_shift"];
    let targets = sparks_fx_targets();
    let mut children = vec![
        enum_item(
            "FX Type",
            "sparks.fx.type",
            fx_types.clone(),
            selected_index(&fx_types, &config.sparks_fx_type),
        ),
        enum_item_from_strings(
            "Target",
            "sparks.fx.target",
            targets.clone(),
            targets
                .iter()
                .position(|target| target == &config.sparks_fx_target)
                .unwrap_or(0),
        ),
    ];
    match config.sparks_fx_type.as_str() {
        "stutter" => {
            children.push(number_item(
                "Rate Hz",
                "sparks.fx.params.rateHz",
                number_param(&config.sparks_fx_params, "rateHz", 8),
                1,
                32,
                1,
            ));
            children.push(number_item(
                "Depth",
                "sparks.fx.params.depthPct",
                number_param(&config.sparks_fx_params, "depthPct", 100),
                0,
                100,
                1,
            ));
        }
        "freeze" => {
            children.push(number_item(
                "Release Ms",
                "sparks.fx.params.releaseMs",
                number_param(&config.sparks_fx_params, "releaseMs", 500),
                10,
                5000,
                10,
            ));
            children.push(number_item(
                "Mix",
                "sparks.fx.params.mixPct",
                number_param(&config.sparks_fx_params, "mixPct", 100),
                0,
                100,
                1,
            ));
        }
        "filter_sweep" => {
            children.push(number_item(
                "Cutoff",
                "sparks.fx.params.cutoffPct",
                number_param(&config.sparks_fx_params, "cutoffPct", 50),
                0,
                100,
                1,
            ));
            children.push(number_item(
                "Res",
                "sparks.fx.params.resonancePct",
                number_param(&config.sparks_fx_params, "resonancePct", 0),
                0,
                100,
                1,
            ));
            children.push(number_item(
                "Sweep In",
                "sparks.fx.params.sweepInMs",
                number_param(&config.sparks_fx_params, "sweepInMs", 120),
                10,
                3000,
                10,
            ));
            children.push(number_item(
                "Sweep Out",
                "sparks.fx.params.sweepOutMs",
                number_param(&config.sparks_fx_params, "sweepOutMs", 180),
                10,
                3000,
                10,
            ));
        }
        "pitch_shift" => {
            children.push(number_item(
                "Semitones",
                "sparks.fx.params.semitones",
                number_param(&config.sparks_fx_params, "semitones", 0),
                -24,
                24,
                1,
            ));
            children.push(number_item(
                "Cents",
                "sparks.fx.params.cents",
                number_param(&config.sparks_fx_params, "cents", 0),
                -100,
                100,
                1,
            ));
            children.push(number_item(
                "Mix",
                "sparks.fx.params.mixPct",
                number_param(&config.sparks_fx_params, "mixPct", 100),
                0,
                100,
                1,
            ));
        }
        _ => {}
    }
    children.push(action_item(
        "Map to Grid",
        "sparks.fx.map",
        NativeMenuAction::PlatformEffect("sparks.fx.map".into()),
    ));
    children
}

fn number_param(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: i32,
) -> i32 {
    params
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .map(|value| value as i32)
        .unwrap_or(default)
}
