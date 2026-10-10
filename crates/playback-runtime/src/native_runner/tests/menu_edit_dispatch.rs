use super::*;
use crate::native_menu::{NativeMenuItem, NativeMenuValue};

fn editable_keys(node: &NativeMenuItem, out: &mut Vec<String>) {
    if let (
        Some(key),
        NativeMenuValue::Number { .. }
        | NativeMenuValue::Enum { .. }
        | NativeMenuValue::Bool { .. },
    ) = (&node.key, &node.value)
    {
        if !(1..8).any(|index| key.contains(&format!(".{index}."))) {
            out.push(key.clone());
        }
    }
    for child in &node.children {
        editable_keys(child, out);
    }
}

fn edit_with_encoder(runner: &mut NativeRunner, key: &str) {
    assert!(runner.menu.focus_item_key(key), "{key} is not in the menu");
    for input in [
        json!({ "type": "encoder_press", "id": "main" }),
        json!({ "type": "encoder_turn", "delta": 1, "id": "main" }),
        json!({ "type": "encoder_press", "id": "main" }),
    ] {
        runner
            .send(HostMessage::DeviceInput {
                input,
                request_snapshot: None,
            })
            .unwrap_or_else(|error| panic!("{key}: {error}"));
    }
}

fn configured_runner(configure: &dyn Fn(&mut NativeRunner)) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    configure(&mut runner);
    runner.menu.rebuild(runner.menu_config());
    runner
}

fn rejected_edits(
    configure: &dyn Fn(&mut NativeRunner),
    seen: &mut std::collections::BTreeSet<String>,
) -> Vec<String> {
    let runner = configured_runner(configure);
    let mut keys = Vec::new();
    editable_keys(&runner.menu.root, &mut keys);
    keys.retain(|key| seen.insert(key.clone()));
    let mut rejected = Vec::new();
    for key in &keys {
        let mut runner = configured_runner(configure);
        for delta in [1, -1] {
            if !runner.menu.focus_item_key(key) || !runner.menu.turn_key(key, delta) {
                continue;
            }
            if let Err(error) = runner.apply_or_schedule_menu_key(key) {
                rejected.push(format!("{key} {delta:+}: {error}"));
            }
        }
    }
    rejected
}

#[test]
fn every_editable_menu_row_is_accepted_by_the_edit_dispatch() {
    let mut seen = std::collections::BTreeSet::new();
    let mut rejected = rejected_edits(
        &|runner| {
            runner.link_layers[0].scan_mode = "scanning".into();
        },
        &mut seen,
    );
    assert!(seen.len() > 100, "{} keys", seen.len());
    for kind in ["fm", "pluck", "drum", "sampler", "midi"] {
        rejected.extend(rejected_edits(
            &|runner| {
                runner.instruments[0].kind = kind.into();
            },
            &mut seen,
        ));
    }
    for fx in [
        "tremolo",
        "delay",
        "vibrato",
        "chorus",
        "flanger",
        "filter_lfo",
        "wah",
        "vinyl",
        "eq",
        "compressor",
        "reverb",
        "glitch",
        "auto_pan",
        "duck",
        "saturator",
        "distortion",
        "bitcrusher",
    ] {
        rejected.extend(rejected_edits(
            &|runner| {
                let mut deltas = std::collections::BTreeMap::new();
                runner.apply_param_binding_value(
                    "mixer.buses.0.slot1.type",
                    json!(fx),
                    &mut deltas,
                );
                if ["vinyl", "eq", "compressor", "saturator", "distortion"].contains(&fx) {
                    runner.apply_param_binding_value(
                        "mixer.master.slots.0.type",
                        json!(fx),
                        &mut deltas,
                    );
                }
            },
            &mut seen,
        ));
    }
    assert!(rejected.is_empty(), "{rejected:#?}");
}

#[test]
fn auto_label_rows_rename_instruments_and_buses_from_the_encoder() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].name = "manual inst".into();
    runner.instruments[0].auto_name = false;
    runner.fx_buses[0].slot1_type = "delay".into();
    runner.fx_buses[0].name = "manual bus".into();
    runner.fx_buses[0].auto_name = false;
    runner.menu.rebuild(runner.menu_config());

    edit_with_encoder(&mut runner, "instruments.0.autoName");
    edit_with_encoder(&mut runner, "mixer.buses.0.autoName");

    assert!(runner.instruments[0].auto_name);
    assert_eq!(runner.instruments[0].name, "Synth");
    assert_eq!(
        runner.menu.value_for_key("instruments.0.name").as_deref(),
        Some("Synth")
    );
    assert!(runner.fx_buses[0].auto_name);
    assert_eq!(runner.fx_buses[0].name, "Delay");
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][0]["autoName"],
        true
    );
}

#[test]
fn link_axis_invert_and_recording_max_time_apply_from_the_encoder() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.param_mods[0].x[0] = Some(NativeParamBinding {
        key: "instruments.0.mixer.volume".into(),
        label: None,
        kind: "number".into(),
        min: Some(0.0),
        max: Some(100.0),
        step: Some(1.0),
        user_min: None,
        user_max: None,
        options: vec![],
        invert: false,
    });
    runner.menu.rebuild(runner.menu_config());
    let max_minutes = runner.recording_max_minutes;

    edit_with_encoder(&mut runner, "layers.0.paramMods.x.0.invert");
    edit_with_encoder(&mut runner, "recording.maxMinutes");

    assert!(runner.param_mods[0].x[0].as_ref().unwrap().invert);
    assert!(runner.recording_max_minutes > max_minutes);
    assert_eq!(
        runner.menu.number_for_key("recording.maxMinutes"),
        Some(i32::from(runner.recording_max_minutes))
    );
}

#[test]
fn an_lfo_without_a_target_stays_off_and_its_row_says_so() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    edit_with_encoder(&mut runner, "linkLfos.0.enabled");
    assert!(!runner.link_lfos[0].enabled);
    assert_eq!(
        runner.menu.value_for_key("linkLfos.0.enabled").as_deref(),
        Some("false")
    );
}
