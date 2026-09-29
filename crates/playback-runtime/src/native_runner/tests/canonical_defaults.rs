use super::*;
use sha2::{Digest, Sha256};

const PI_DEFAULT_BYTES: &[u8] =
    include_bytes!("fixtures/config_persistence/pi_canonical_default.json");
const PI_DEFAULT_SHA256: &str = "26a6205bbe99dfc4e122aed232c816d66eaf5a6c7ed3aebcedf73b9b3e55dcc2";

#[test]
pub(crate) fn pi_default_reproduces_complete_canonical_projections() {
    let base: Value =
        serde_json::from_str(include_str!("../../../../../config/defaults/base.json")).unwrap();
    let desktop: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap();
    let pi: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap();
    let desktop_override: Value =
        serde_json::from_str(include_str!("../../../../../config/defaults/desktop.json")).unwrap();
    let pi_override: Value =
        serde_json::from_str(include_str!("../../../../../config/defaults/pi.json")).unwrap();

    let layer_four_cells = base["runtimeConfig"]["layers"][3]["build"]["savedState"]["cells"]
        .as_array()
        .unwrap();
    assert_eq!(layer_four_cells.len(), 64);
    assert_eq!(
        layer_four_cells
            .iter()
            .enumerate()
            .filter_map(|(index, cell)| cell.as_bool().unwrap().then_some(index))
            .collect::<Vec<_>>(),
        vec![1, 3, 5, 7, 42, 46]
    );

    let expected_pluck = serde_json::json!({
        "amp": {"gainPct": 80, "velocitySensitivityPct": 100},
        "ampEnv": {"attackMs": 0, "decayMs": 0, "releaseMs": 900, "sustainPct": 100},
        "bodyAmountPct": 81,
        "bodyFrequencyHz": 884,
        "brightnessPct": 81,
        "decayMs": 1500,
        "dispersionPct": 37,
        "filter": {"cutoffHz": 16000, "envAmountPct": 0, "keyTrackingPct": 0, "resonance": 0, "type": "lowpass"},
        "filterEnv": {"attackMs": 5, "decayMs": 120, "releaseMs": 180, "sustainPct": 70},
        "pickDepthPct": 78,
        "pickPositionPct": 41
    });
    let expected_fm = serde_json::json!({
        "amp": {"gainPct": 80, "velocitySensitivityPct": 100},
        "ampEnv": {"attackMs": 5, "decayMs": 300, "releaseMs": 350, "sustainPct": 70},
        "filter": {"cutoffHz": 8000, "envAmountPct": 0, "keyTrackingPct": 0, "resonance": 20, "type": "lowpass"},
        "filterEnv": {"attackMs": 5, "decayMs": 120, "releaseMs": 180, "sustainPct": 70},
        "index": 38,
        "indexEnv": {"attackMs": 170, "decayMs": 230, "releaseMs": 85, "sustainPct": 54},
        "modMixPct": 0,
        "modShapePct": 0,
        "ratio": "2",
        "ratioFineCents": 0,
        "velocityToIndexPct": 0
    });

    for defaults in [&base, &desktop, &pi] {
        assert_default_looper_steps(defaults);
    }
    for (index, instrument_type, name) in [(2, "pluck", "Plucked"), (3, "fm", "FM")] {
        for defaults in [&base, &desktop, &pi] {
            let instrument = &defaults["runtimeConfig"]["instruments"][index];
            assert_eq!(instrument["autoName"], true);
            assert_eq!(instrument["name"], name);
            assert_eq!(instrument["type"], instrument_type);
            assert_eq!(
                instrument[if index == 2 { "pluck" } else { "fm" }],
                if index == 2 {
                    expected_pluck.clone()
                } else {
                    expected_fm.clone()
                }
            );
        }
    }
    let mut runner = runner_with_config(base.clone());
    let mut pi_default = pi_default();
    assert_default_looper_steps(&pi_default);
    for (index, key, expected) in [(2, "pluck", &expected_pluck), (3, "fm", &expected_fm)] {
        assert_eq!(
            pi_default["runtimeConfig"]["instruments"][index][key],
            *expected
        );
    }
    normalize_sample_paths(&mut pi_default);
    runner
        .apply_patch_payload_preserving_device(pi_default)
        .unwrap();
    for (index, instrument_type, name) in [(2, "pluck", "Plucked"), (3, "fm", "FM")] {
        assert_eq!(runner.instruments[index].kind, instrument_type);
        assert_eq!(runner.instruments[index].name, name);
        assert!(runner.instruments[index].auto_name);
    }

    assert_eq!(
        runner.patch_payload().unwrap()["runtimeConfig"]["activeBehavior"],
        "life"
    );
    let applied_payload = runner.patch_payload().unwrap();
    assert_eq!(
        applied_payload["runtimeConfig"]["instruments"][2]["pluck"],
        expected_pluck
    );
    assert_eq!(
        applied_payload["runtimeConfig"]["instruments"][3]["fm"],
        expected_fm
    );
    assert_eq!(
        device_config_payload_from_payload(runner.config_payload()).unwrap(),
        device_config_payload_from_payload(base.clone()).unwrap()
    );

    let mut expected_desktop = runner_with_config(base.clone());
    expected_desktop
        .apply_device_config_payload_preserving_patch(desktop_override)
        .unwrap();
    let mut expected_pi = runner_with_config(base.clone());
    expected_pi
        .apply_device_config_payload_preserving_patch(pi_override)
        .unwrap();
    assert_eq!(
        device_config_payload_from_payload(desktop.clone()).unwrap(),
        device_config_payload_from_payload(expected_desktop.config_payload()).unwrap()
    );
    assert_eq!(
        device_config_payload_from_payload(pi.clone()).unwrap(),
        device_config_payload_from_payload(expected_pi.config_payload()).unwrap()
    );
    assert_eq!(
        device_config_payload_from_payload(base.clone()).unwrap(),
        device_config_payload_from_payload(pi.clone()).unwrap()
    );

    assert_eq!(
        include_str!("../../../../../config/default.json"),
        include_str!("../../../../../config/generated/pi/default.json")
    );
}

fn assert_default_looper_steps(config: &Value) {
    let looper = &config["runtimeConfig"]["layers"][2]["build"];
    assert_eq!(looper["stepRate"], "1/16");
    assert_eq!(looper["savedState"]["lengthSteps"], 32);
    let steps = looper["savedState"]["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 32);
    let nonempty = steps
        .iter()
        .enumerate()
        .filter(|(_, events)| !events.as_array().unwrap().is_empty())
        .map(|(index, events)| (index, events.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        nonempty,
        vec![
            (7, serde_json::json!([{"cell": 49, "kind": "press"}])),
            (
                8,
                serde_json::json!([
                    {"cell": 49, "kind": "release"},
                    {"cell": 42, "kind": "press"},
                    {"cell": 42, "kind": "press"}
                ])
            ),
            (
                10,
                serde_json::json!([
                    {"cell": 49, "kind": "press"},
                    {"cell": 42, "kind": "release"},
                    {"cell": 42, "kind": "release"}
                ])
            ),
            (12, serde_json::json!([{"cell": 49, "kind": "release"}])),
            (15, serde_json::json!([{"cell": 49, "kind": "press"}])),
            (16, serde_json::json!([{"cell": 49, "kind": "release"}])),
            (
                24,
                serde_json::json!([
                    {"cell": 49, "kind": "press"},
                    {"cell": 49, "kind": "release"},
                    {"cell": 42, "kind": "press"}
                ])
            ),
            (26, serde_json::json!([{"cell": 42, "kind": "release"}])),
            (27, serde_json::json!([{"cell": 49, "kind": "press"}])),
            (28, serde_json::json!([{"cell": 49, "kind": "release"}]))
        ]
    );
    assert_eq!(
        nonempty
            .iter()
            .map(|(_, events)| events.as_array().unwrap().len())
            .sum::<usize>(),
        16
    );
}

fn runner_with_config(payload: Value) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner
}

fn pi_default() -> Value {
    let digest = Sha256::digest(PI_DEFAULT_BYTES);
    assert_eq!(format!("{digest:x}"), PI_DEFAULT_SHA256);
    serde_json::from_slice(PI_DEFAULT_BYTES).unwrap()
}

fn normalize_sample_paths(payload: &mut Value) {
    let instruments = payload["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap();
    for instrument in instruments {
        let Some(slots) = instrument["sample"]["slots"].as_array_mut() else {
            continue;
        };
        for slot in slots {
            let Some(path) = slot["path"].as_str().map(str::to_owned) else {
                continue;
            };
            let path = path.replace('\\', "/");
            slot["path"] = if path.starts_with("samples/") {
                Value::String(path)
            } else {
                Value::String(format!("samples/{path}"))
            };
        }
    }
}
