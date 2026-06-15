use super::options::{duck_source_options, FX_BUS_SLOT_OPTIONS, GLOBAL_FX_SLOT_OPTIONS};
use super::{
    bool_item, enum_item, enum_item_from_strings, group, number_item, selected_index, text_item,
    NativeFxBusConfig, NativeMenuItem,
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
                            "Slot 1",
                            &format!("{prefix}.slot1"),
                            &bus.slot1_type,
                            &bus.slot1_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        fx_slot_group(
                            "Slot 2",
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
                        bool_item("Auto Name", format!("{prefix}.autoName"), bus.auto_name),
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
                    format!("Slot {}", slot_index + 1),
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

fn fx_param_items(
    slot_type: &str,
    prefix: &str,
    params: &serde_json::Value,
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    match slot_type {
        "duck" => {
            let options = duck_source_options(bus_index.unwrap_or(usize::MAX));
            vec![
                enum_item_from_strings(
                    "Source",
                    format!("{prefix}.source"),
                    options.clone(),
                    options
                        .iter()
                        .position(|option| option == &fx_param_string(params, "source", "I1"))
                        .unwrap_or(0),
                ),
                fx_number_item(
                    "Threshold",
                    prefix,
                    params,
                    "threshold",
                    0,
                    100,
                    1,
                    100.0,
                    0.08,
                ),
                fx_number_item(
                    "Amount %",
                    prefix,
                    params,
                    "amountPct",
                    0,
                    100,
                    1,
                    1.0,
                    60.0,
                ),
                fx_number_item("Attack ms", prefix, params, "attackMs", 1, 500, 1, 1.0, 8.0),
                fx_number_item(
                    "Release ms",
                    prefix,
                    params,
                    "releaseMs",
                    1,
                    5000,
                    5,
                    1.0,
                    160.0,
                ),
            ]
        }
        "delay" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 35.0),
            fx_number_item("Time ms", prefix, params, "timeMs", 1, 2000, 5, 1.0, 250.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", 0, 98, 1, 100.0, 0.35,
            ),
        ],
        "tremolo" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 5, 4000, 5, 100.0, 4.0),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 60.0),
        ],
        "saturator" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 200, 1, 10.0, 1.8),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "distortion" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 500, 5, 10.0, 2.5),
            fx_number_item("Clip", prefix, params, "clip", 5, 200, 5, 100.0, 0.6),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "bitcrusher" => vec![
            fx_number_item("Bits", prefix, params, "bits", 1, 16, 1, 1.0, 6.0),
            fx_number_item("Rate Div", prefix, params, "rateDiv", 1, 128, 1, 1.0, 4.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vibrato" | "chorus" | "flanger" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.8),
            fx_number_item("Depth ms", prefix, params, "depthMs", 0, 400, 1, 10.0, 6.0),
            fx_number_item("Base ms", prefix, params, "baseMs", 1, 800, 1, 10.0, 8.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", -95, 95, 1, 100.0, 0.0,
            ),
        ],
        "filter_lfo" | "wah" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item(
                "Center Hz",
                prefix,
                params,
                "centerHz",
                40,
                12000,
                20,
                1.0,
                1600.0,
            ),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 70.0),
            fx_number_item("Q", prefix, params, "q", 25, 2000, 25, 100.0, 1.0),
        ],
        "reverb" => vec![
            fx_number_item("Decay", prefix, params, "decay", 0, 995, 5, 1000.0, 0.72),
            fx_number_item("Damp", prefix, params, "damp", 0, 98, 1, 100.0, 0.35),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 30.0),
        ],
        "auto_pan" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 100.0),
        ],
        "glitch" => vec![
            fx_number_item("Chance %", prefix, params, "chancePct", 0, 100, 1, 1.0, 8.0),
            fx_number_item("Slice ms", prefix, params, "sliceMs", 5, 500, 5, 1.0, 80.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "compressor" => vec![
            fx_number_item(
                "Threshold dB",
                prefix,
                params,
                "thresholdDb",
                -120,
                0,
                1,
                2.0,
                -24.0,
            ),
            fx_number_item("Ratio", prefix, params, "ratio", 2, 40, 1, 2.0, 4.0),
            fx_number_item(
                "Attack ms",
                prefix,
                params,
                "attackMs",
                1,
                200,
                1,
                1.0,
                10.0,
            ),
            fx_number_item(
                "Release ms",
                prefix,
                params,
                "releaseMs",
                5,
                2000,
                5,
                1.0,
                100.0,
            ),
            fx_number_item("Makeup dB", prefix, params, "makeupDb", 0, 48, 1, 2.0, 0.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "eq" => vec![
            fx_number_item(
                "Low Gain dB",
                prefix,
                params,
                "lowGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "Mid Gain dB",
                prefix,
                params,
                "midGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "High Gain dB",
                prefix,
                params,
                "highGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "Mid Freq Hz",
                prefix,
                params,
                "midFreqHz",
                40,
                8000,
                10,
                1.0,
                1000.0,
            ),
            fx_number_item("Mid Q", prefix, params, "midQ", 25, 2000, 25, 100.0, 1.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vinyl" => vec![
            fx_number_item(
                "Saturation %",
                prefix,
                params,
                "saturationPct",
                0,
                100,
                1,
                1.0,
                15.0,
            ),
            fx_number_item(
                "Crackle %",
                prefix,
                params,
                "cracklePct",
                0,
                100,
                1,
                1.0,
                8.0,
            ),
            fx_number_item(
                "Warp Depth %",
                prefix,
                params,
                "warpDepthPct",
                0,
                100,
                1,
                1.0,
                5.0,
            ),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        _ => vec![],
    }
}

#[expect(clippy::too_many_arguments, reason = "FX menu specs are data rows")]
fn fx_number_item(
    label: impl Into<String>,
    prefix: &str,
    params: &serde_json::Value,
    key: &str,
    min: i32,
    max: i32,
    step: i32,
    scale: f64,
    default: f64,
) -> NativeMenuItem {
    number_item(
        label,
        format!("{prefix}.{key}"),
        ((fx_param_number(params, key, default) * scale).round() as i32).clamp(min, max),
        min,
        max,
        step,
    )
}

fn fx_param_number(params: &serde_json::Value, key: &str, default: f64) -> f64 {
    params
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default)
}

fn fx_param_string(params: &serde_json::Value, key: &str, default: &str) -> String {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
        .into()
}

pub(super) fn default_fx_bus_config() -> NativeFxBusConfig {
    NativeFxBusConfig {
        name: "(none)".into(),
        slot1_type: "none".into(),
        slot1_params: serde_json::json!({}),
        slot2_type: "none".into(),
        slot2_params: serde_json::json!({}),
        pan_pos: 16,
        auto_name: true,
    }
}
