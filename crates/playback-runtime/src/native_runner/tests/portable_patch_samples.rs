use super::*;

fn generated_pi_default() -> Value {
    serde_json::from_str(include_str!(
        "../../../../../config/generated/pi/default.json"
    ))
    .unwrap()
}

fn sample_path_payload(path: &str) -> Value {
    let mut payload = generated_pi_default();
    payload["runtimeConfig"]["instruments"][1]["sample"]["slots"][0]["path"] = json!(path);
    payload
}

fn transitioned_sample_path_payload(path: &str, instrument_type: &str) -> Value {
    let mut payload = sample_path_payload(path);
    payload["runtimeConfig"]["instruments"][1]["type"] = json!(instrument_type);
    payload
}

#[test]
pub(crate) fn partial_sampler_assignments_use_explicit_portable_schema() {
    let patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 2,
        "runtimeConfig": {
            "instruments": [
                {},
                {
                    "sample": {
                        "assignments": [
                            { "level": null, "sampleSlot": 3, "x": 2, "y": 4 }
                        ]
                    }
                }
            ]
        }
    });
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_patch_payload_preserving_device(patch.clone())
        .unwrap();
    assert_eq!(
        runner.config_payload()["runtimeConfig"]["instruments"][1]["sample"]["assignments"][0],
        patch["runtimeConfig"]["instruments"][1]["sample"]["assignments"][0]
    );

    let mut invalid = patch;
    invalid["runtimeConfig"]["instruments"][1]["sample"]["assignments"][0]["futureField"] =
        json!(true);
    let error = NativeRunner::new(NativeRunnerConfig::default())
        .unwrap()
        .apply_patch_payload_preserving_device(invalid)
        .unwrap_err();
    assert!(
        error.contains("$.runtimeConfig.instruments[1].sample.assignments[0].futureField"),
        "{error}"
    );
}

#[test]
pub(crate) fn portable_sample_ids_are_manifest_wavs_and_invalid_ids_are_rejected() {
    let valid = sample_path_payload("samples/Drum/kick/Kick2.wav");
    assert!(portable_patch_bytes(&valid).is_ok());

    for invalid in [
        "userdata/User Kit/custom.wav",
        "sd-card/octessera/samples/kick.wav",
        "Drums/kick.wav",
        r"samples\Drum\kick\Kick2.wav",
        "/samples/Drum/kick/Kick2.wav",
        "C:/samples/Drum/kick/Kick2.wav",
        "samples/../Drum/kick/Kick2.wav",
        "samples/Drum/kick/not-in-manifest.wav",
        "samples/Drum/kick/not-audio.mp3",
    ] {
        let payload = sample_path_payload(invalid);
        let error = portable_patch_bytes(&payload).unwrap_err();
        assert!(
            error.contains("$.runtimeConfig.instruments[1].sample.slots[0].path"),
            "{invalid}: {error}"
        );

        let mut validation_runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        validation_runner
            .apply_config_payload(generated_pi_default())
            .unwrap();
        let error = prepare_patch_payload(
            portable_patch_projection(&payload).unwrap(),
            &validation_runner.config_payload(),
        )
        .unwrap_err();
        assert!(
            error.contains("$.runtimeConfig.instruments[1].sample.slots[0].path"),
            "{invalid}: {error}"
        );

        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.apply_config_payload(payload).unwrap();
        let error = runner
            .platform_effect_for_action("preset.saveAs")
            .unwrap_err();
        assert!(
            error.contains("$.runtimeConfig.instruments[1].sample.slots[0].path"),
            "{invalid}: {error}"
        );
    }
}

#[test]
pub(crate) fn retained_sample_paths_are_rejected_for_non_sampler_portable_saves() {
    for instrument_type in ["synth", "midi", "none"] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner
            .apply_config_payload(transitioned_sample_path_payload(
                "userdata/User Kit/custom.wav",
                instrument_type,
            ))
            .unwrap();
        let error = runner
            .platform_effect_for_action("preset.saveAs")
            .unwrap_err();
        assert!(
            error.contains("$.runtimeConfig.instruments[1].sample.slots[0].path"),
            "{instrument_type}: {error}"
        );
    }
}

#[test]
pub(crate) fn retained_sample_paths_are_rejected_for_non_sampler_v2_loads() {
    for instrument_type in ["synth", "midi", "none"] {
        let payload =
            transitioned_sample_path_payload("userdata/User Kit/custom.wav", instrument_type);
        let patch = portable_patch_projection(&payload).unwrap();
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.apply_config_payload(generated_pi_default()).unwrap();
        let error = runner
            .apply_patch_payload_preserving_device(patch)
            .unwrap_err();
        assert!(
            error.contains("$.runtimeConfig.instruments[1].sample.slots[0].path"),
            "{instrument_type}: {error}"
        );
    }
}

#[test]
pub(crate) fn full_config_saves_preserve_local_and_sd_sample_paths() {
    for sample_path in [
        "userdata/User Kit/custom.wav",
        "sd-card/octessera/samples/kick.wav",
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner
            .apply_config_payload(sample_path_payload(sample_path))
            .unwrap();
        let assert_saved_path = |payload: &Value| {
            assert_eq!(
                payload["runtimeConfig"]["instruments"][1]["sample"]["slots"][0]["path"].as_str(),
                Some(sample_path)
            );
        };
        assert_saved_path(&runner.config_payload());

        for action in ["default.save", "system.reboot", "usb.applyReboot"] {
            let effect = runner.platform_effect_for_action(action).unwrap().unwrap();
            let payload = match effect {
                RuntimePlatformEffect::StoreSaveDefault { payload, .. }
                | RuntimePlatformEffect::StoreSaveRecovery { payload }
                | RuntimePlatformEffect::ApplyDeviceConfigReboot { payload } => payload,
                _ => panic!("unexpected full-save effect for {action}"),
            };
            assert_saved_path(&payload);
        }

        runner.config_dirty = true;
        let backup = runner
            .messages_with_snapshot()
            .unwrap()
            .into_iter()
            .find_map(|message| match message {
                RunnerMessage::PlatformEffects { effects } => {
                    effects.into_iter().find_map(|effect| match effect {
                        RuntimePlatformEffect::StoreSaveBackup { payload } => Some(payload),
                        _ => None,
                    })
                }
                _ => None,
            })
            .expect("backup save effect");
        assert_saved_path(&backup);
    }
}

#[test]
pub(crate) fn rejected_preset_sample_path_is_presented_as_a_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .apply_config_payload(sample_path_payload("userdata/User Kit/custom.wav"))
        .unwrap();

    let result = runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect("preset.saveAs".into()))
        .unwrap();

    assert!(result.is_none());
    assert!(runner
        .display
        .toast
        .as_ref()
        .is_some_and(|toast| toast.message.contains("Preset save rejected")));
}
