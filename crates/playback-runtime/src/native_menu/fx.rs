use super::fx_params::fx_param_items;
use super::options::{FX_BUS_SLOT_OPTIONS, GLOBAL_FX_SLOT_OPTIONS};
use super::{
    bool_item, enum_item, group, number_item, selected_index, text_item, NativeFxBusConfig,
    NativeMenuItem,
};
use platform_core::{BUS_COUNT as FX_BUS_COUNT, GLOBAL_FX_SLOT_COUNT};

pub(super) fn fx_buses_group(config: &[NativeFxBusConfig]) -> NativeMenuItem {
    group(
        "FX Buses",
        (0..FX_BUS_COUNT)
            .map(|bus_index| {
                let prefix = format!("mixer.buses.{bus_index}");
                let bus = config
                    .get(bus_index)
                    .cloned()
                    .unwrap_or_else(default_fx_bus_config);
                group(
                    format!("B{}: {}", bus_index + 1, bus.name),
                    vec![
                        fx_slot_group(
                            slot_group_label(1, &bus.slot1_type),
                            &format!("{prefix}.slot1"),
                            &bus.slot1_type,
                            &bus.slot1_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        fx_slot_group(
                            slot_group_label(2, &bus.slot2_type),
                            &format!("{prefix}.slot2"),
                            &bus.slot2_type,
                            &bus.slot2_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        number_item(
                            "Pan Pos",
                            format!("{prefix}.panPos"),
                            i32::from(bus.pan_pos),
                            0,
                            32,
                            1,
                        ),
                        bool_item("Auto Label", format!("{prefix}.autoName"), bus.auto_name),
                        text_item("Name", format!("{prefix}.name"), bus.name.clone(), 32),
                    ],
                )
            })
            .collect(),
    )
}

pub(super) fn global_fx_group(config: &[String], params: &[serde_json::Value]) -> NativeMenuItem {
    group(
        "Global FX",
        (0..GLOBAL_FX_SLOT_COUNT)
            .map(|slot_index| {
                let prefix = format!("mixer.master.slots.{slot_index}");
                let slot_type = config.get(slot_index).map(String::as_str).unwrap_or("none");
                let slot_params = params.get(slot_index).unwrap_or(&serde_json::Value::Null);
                group(
                    slot_group_label(slot_index + 1, slot_type),
                    fx_slot_children(
                        &prefix,
                        slot_type,
                        slot_params,
                        GLOBAL_FX_SLOT_OPTIONS,
                        None,
                    ),
                )
            })
            .collect(),
    )
}

fn fx_slot_group(
    label: impl Into<String>,
    prefix: &str,
    slot_type: &str,
    params: &serde_json::Value,
    options: &[&str],
    bus_index: Option<usize>,
) -> NativeMenuItem {
    group(
        label,
        fx_slot_children(prefix, slot_type, params, options, bus_index),
    )
}

fn slot_group_label(slot_number: usize, slot_type: &str) -> String {
    format!("Slot {slot_number}: {}", fx_type_label(slot_type))
}

fn fx_type_label(slot_type: &str) -> String {
    match slot_type {
        "none" => "None".into(),
        "delay" => "Delay".into(),
        "duck" => "Duck".into(),
        "reverb" => "Reverb".into(),
        "tremolo" => "Tremolo".into(),
        "saturator" => "Saturator".into(),
        "distortion" => "Distortion".into(),
        "bitcrusher" => "Bitcrusher".into(),
        "vibrato" => "Vibrato".into(),
        "chorus" => "Chorus".into(),
        "flanger" => "Flanger".into(),
        "filter_lfo" => "Filter LFO".into(),
        "wah" => "Wah".into(),
        "auto_pan" => "Auto Pan".into(),
        "glitch" => "Glitch".into(),
        "compressor" => "Compressor".into(),
        "eq" => "EQ".into(),
        "vinyl" => "Vinyl".into(),
        _ => slot_type.into(),
    }
}

fn fx_slot_children(
    prefix: &str,
    slot_type: &str,
    params: &serde_json::Value,
    options: &[&str],
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    let mut children = vec![enum_item(
        "Type",
        format!("{prefix}.type"),
        options.to_vec(),
        selected_index(options, slot_type),
    )];
    children.extend(fx_param_items(
        slot_type,
        &format!("{prefix}.params"),
        params,
        bus_index,
    ));
    children
}

pub(super) fn default_fx_bus_config() -> NativeFxBusConfig {
    NativeFxBusConfig {
        name: "None".into(),
        slot1_type: "none".into(),
        slot1_params: serde_json::json!({}),
        slot2_type: "none".into(),
        slot2_params: serde_json::json!({}),
        pan_pos: 16,
        auto_name: true,
    }
}
