use super::{
    action_item, enum_item, enum_item_from_strings, group, keyed_group, number_item,
    selected_index, xy_pad_items, NativeMenuAction, NativeMenuConfig, NativeMenuItem,
    NativeMenuValue, PlatformAction,
};
use crate::native_menu::section_labels::PLAY_LABEL;
use crate::native_runner::play_fx_config::{play_fx_param_specs, play_fx_targets, play_fx_types};
use crate::native_runner::play_pages::{PlayMode, PLAY_PAGES};

pub(super) fn play_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let pages = PLAY_PAGES.iter().map(|page| {
        let children = match page.mode {
            PlayMode::Fx => play_fx_page_children(config),
            PlayMode::Xy => xy_pad_items(config),
            PlayMode::Faders => super::play_faders::faders_page_children(config),
            PlayMode::Mix
            | PlayMode::Pan
            | PlayMode::Route
            | PlayMode::Tempo
            | PlayMode::Lfo
            | PlayMode::Transpose
            | PlayMode::Rate
            | PlayMode::TriggerGate => vec![],
        };
        keyed_group(
            page.label,
            format!("play.page.{}", page.mode.wire_name()),
            children,
        )
    });
    group(PLAY_LABEL, pages.collect())
}

pub(super) fn play_fx_page_children(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    play_fx_page_items(config)
        .into_iter()
        .chain([group("Aux Map", play_aux_map_items(config))])
        .collect()
}

fn play_aux_map_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    play_fx_page_items(config)
        .into_iter()
        .filter(|item| {
            item.key
                .as_deref()
                .is_some_and(|key| key.starts_with("play.fx.params."))
                || matches!(
                    &item.value,
                    NativeMenuValue::Action(NativeMenuAction::PlatformEffect(
                        PlatformAction::PlayFxMap
                    ))
                )
        })
        .collect()
}

pub(super) fn play_fx_page_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let fx_types = play_fx_types().collect::<Vec<_>>();
    let targets = play_fx_targets();
    let mut children = vec![
        enum_item(
            "FX Type",
            "play.fx.type",
            fx_types.clone(),
            selected_index(&fx_types, &config.play_fx_type),
        ),
        enum_item_from_strings(
            "Target",
            "play.fx.target",
            targets.clone(),
            targets
                .iter()
                .position(|target| target == &config.play_fx_target)
                .unwrap_or(0),
        ),
    ];
    for spec in play_fx_param_specs(&config.play_fx_type) {
        children.push(number_item(
            spec.label,
            format!("play.fx.params.{}", spec.key),
            number_param(&config.play_fx_params, spec.key, spec.default),
            spec.min,
            spec.max,
            spec.step,
        ));
    }
    children.push(action_item(
        "Map to Grid",
        "play.fx.map",
        NativeMenuAction::PlatformEffect(PlatformAction::PlayFxMap),
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
