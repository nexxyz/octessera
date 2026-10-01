use super::super::{portable_patch_projection, split_system_patch_documents};
use super::*;
use crate::{NativeRunner, NativeRunnerConfig};
use serde_json::json;
use std::time::{Duration, Instant};

fn runner_with_local_samples() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].sample_paths[0] = Some("userdata/User Kit/custom.wav".into());
    runner.instruments[1].sample_paths[7] = Some("sd-card/octessera/samples/kick.wav".into());
    runner.instruments[1].sample_paths[6] = Some("sd-card/octessera/samples/missing.wav".into());
    runner.instruments[1].kind = "sampler".into();
    runner.display.ui.display_brightness = 41;
    runner.audio_outputs = crate::AudioOutputSet::from_flags(false, true, false).unwrap();
    runner
}

fn effect_payload(
    messages: Vec<crate::protocol::RunnerMessage>,
    matches: fn(&crate::protocol::RuntimePlatformEffect) -> bool,
) -> Value {
    messages
        .into_iter()
        .find_map(|message| match message {
            crate::protocol::RunnerMessage::PlatformEffects { effects } => {
                effects.into_iter().find(matches)
            }
            _ => None,
        })
        .and_then(|effect| match effect {
            crate::protocol::RuntimePlatformEffect::StoreSaveDefault { payload, .. }
            | crate::protocol::RuntimePlatformEffect::StoreSaveBackup { payload }
            | crate::protocol::RuntimePlatformEffect::StoreSaveRecovery { payload } => {
                Some(payload)
            }
            _ => None,
        })
        .expect("expected local Patch persistence payload")
}

#[test]
fn local_documents_round_trip_exact_paths_and_complete_inactive_sampler_arrays() {
    let runner = runner_with_local_samples();
    let full = runner.config_payload();
    let documents = split_local_system_patch_documents(&full).unwrap();

    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
        "userdata/User Kit/custom.wav"
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"][1]["sample"]["slots"][7]["path"],
        "sd-card/octessera/samples/kick.wav"
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"][1]["sample"]["slots"][6]["path"],
        "sd-card/octessera/samples/missing.wav"
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"]
            .as_array()
            .unwrap()
            .len(),
        platform_core::INSTRUMENT_COUNT
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"][1]["sample"]["slots"]
            .as_array()
            .unwrap()
            .len(),
        platform_core::SAMPLE_SLOT_COUNT
    );

    let recomposed =
        compose_local_system_patch_documents(&documents.system, &documents.patch).unwrap();
    assert_eq!(
        recomposed["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"],
        "userdata/User Kit/custom.wav"
    );
    assert_eq!(
        recomposed["runtimeConfig"]["instruments"][1]["sample"]["slots"][7]["path"],
        "sd-card/octessera/samples/kick.wav"
    );
    assert_eq!(
        recomposed["runtimeConfig"]["instruments"][1]["sample"]["slots"][6]["path"],
        "sd-card/octessera/samples/missing.wav"
    );
    assert_eq!(recomposed["runtimeConfig"]["displayBrightness"], 41);
    assert_eq!(
        recomposed["runtimeConfig"]["audioOutputs"],
        json!({ "dac": false, "usb": true, "hdmi": false })
    );
    assert!(split_system_patch_documents(&full).is_err());
}

#[test]
fn local_default_autosave_backup_recovery_and_load_preserve_paths_and_system_settings() {
    let mut runner = runner_with_local_samples();
    let manual = runner
        .platform_effect_for_action("default.save")
        .unwrap()
        .unwrap();
    let crate::protocol::RuntimePlatformEffect::StoreSaveDefault {
        payload: manual_payload,
        ..
    } = manual
    else {
        panic!("expected Patch Save")
    };
    assert_eq!(
        manual_payload["runtimeConfig"]["instruments"][1]["sample"]["slots"][7]["path"],
        "sd-card/octessera/samples/kick.wav"
    );

    let mut autosave_runner = runner_with_local_samples();
    autosave_runner.auto_save_default = true;
    autosave_runner.mark_fast_autosave_dirty();
    autosave_runner.pending.pending_autosave_payload_due_at =
        Some(Instant::now() - Duration::from_millis(1));
    let autosave = effect_payload(
        autosave_runner.flush_due_persistence_music_first().unwrap(),
        |effect| {
            matches!(
                effect,
                crate::protocol::RuntimePlatformEffect::StoreSaveDefault { .. }
            )
        },
    );
    assert_eq!(autosave, manual_payload);

    let mut backup_runner = runner_with_local_samples();
    backup_runner.auto_save_default = false;
    backup_runner.rolling_backups = true;
    backup_runner.config_dirty = true;
    let backup = effect_payload(backup_runner.messages_with_snapshot().unwrap(), |effect| {
        matches!(
            effect,
            crate::protocol::RuntimePlatformEffect::StoreSaveBackup { .. }
        )
    });
    assert_eq!(backup, manual_payload);

    let recovery = runner_with_local_samples()
        .platform_effect_for_action("system.reboot")
        .unwrap()
        .unwrap();
    let crate::protocol::RuntimePlatformEffect::StoreSaveRecovery {
        payload: recovery_payload,
    } = recovery
    else {
        panic!("expected local Patch recovery save")
    };
    assert_eq!(recovery_payload, manual_payload);

    let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    restored.display.ui.display_brightness = 63;
    restored.audio_outputs = crate::AudioOutputSet::from_flags(false, false, true).unwrap();
    restored.instruments[1].sample_paths[6] = Some("stale/old.wav".into());
    restored
        .apply_local_patch_payload_preserving_device(manual_payload)
        .unwrap();
    assert_eq!(restored.display.ui.display_brightness, 63);
    assert_eq!(
        restored.audio_outputs,
        crate::AudioOutputSet::from_flags(false, false, true).unwrap()
    );
    assert_eq!(
        restored.instruments[0].sample_paths[0].as_deref(),
        Some("userdata/User Kit/custom.wav")
    );
    assert_eq!(
        restored.instruments[1].sample_paths[7].as_deref(),
        Some("sd-card/octessera/samples/kick.wav")
    );
    assert_eq!(
        restored.instruments[1].sample_paths[6].as_deref(),
        Some("sd-card/octessera/samples/missing.wav")
    );
    assert_eq!(
        portable_patch_projection(&restored.config_payload()).unwrap(),
        autosave
    );
}

#[test]
fn unsafe_local_paths_fail_without_mutation_or_save_effect_and_remain_visible_dirty_errors() {
    for path in [
        "../escape.wav",
        "C:/escape.wav",
        "/escape.wav",
        "escape.aiff",
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner.instruments[0].sample_paths[0] = Some(path.into());
        runner.config_dirty = true;
        let before = runner.config_payload();
        let baseline = runner.pending.saved_patch_baseline.clone();
        assert!(runner
            .platform_effect_for_action("default.save")
            .unwrap()
            .is_none());
        assert_eq!(runner.config_payload(), before, "{path}");
        assert!(runner.config_dirty, "{path}");
        assert!(
            runner.display.runtime_error_presentation.is_some(),
            "{path}"
        );
        runner.display.runtime_error_presentation = None;
        let messages = runner.messages_with_snapshot().unwrap();

        assert_eq!(runner.config_payload(), before, "{path}");
        assert!(runner.config_dirty, "{path}");
        assert_eq!(runner.pending.saved_patch_baseline, baseline, "{path}");
        assert!(
            runner.display.runtime_error_presentation.is_some(),
            "{path}"
        );
        assert!(messages
            .iter()
            .any(|message| matches!(message, crate::protocol::RunnerMessage::Snapshot { .. })));
        assert!(!messages.iter().any(|message| matches!(
            message,
            crate::protocol::RunnerMessage::PlatformEffects { effects }
                if effects.iter().any(|effect| matches!(
                    effect,
                    crate::protocol::RuntimePlatformEffect::StoreSaveDefault { .. }
                        | crate::protocol::RuntimePlatformEffect::StoreSaveBackup { .. }
                ))
        )));

        let patch = portable_patch_projection(&before).unwrap();
        let mut target = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let target_before = target.config_payload();
        target.config_dirty = true;
        target
            .apply_store_result(crate::protocol::RuntimeStoreResult::LoadDefaultResult {
                payload: Some(patch),
            })
            .unwrap();
        assert_eq!(target.config_payload(), target_before, "{path}");
        assert!(target.config_dirty, "{path}");
        assert!(
            target.display.runtime_error_presentation.is_some(),
            "{path}"
        );
    }
}

#[test]
fn named_presets_and_user_data_archives_keep_portable_sample_validation() {
    let mut runner = runner_with_local_samples();
    assert!(runner
        .platform_effect_for_action("preset.saveAs")
        .unwrap_err()
        .contains("canonical default-library WAV sample ID"));
    let patch = portable_patch_projection(&runner.config_payload()).unwrap();
    let before = runner.config_payload();
    assert!(runner.apply_patch_payload_preserving_device(patch).is_err());
    assert_eq!(runner.config_payload(), before);

    let mut portable = portable_patch_projection(&before).unwrap();
    portable["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]["path"] =
        json!("userdata/User Kit/custom.wav");
    let defaults: Value = serde_json::from_str(include_str!(
        "../../../../../config/generated/desktop/default.json"
    ))
    .unwrap();
    for media_included in [false, true] {
        let result = crate::new_user_data_bundle(
            crate::UserDataBundleMetadata {
                board_profile: "desktop-simulator".into(),
                runtime_version: "0.8.7".into(),
            },
            Vec::new(),
            crate::UserDataMusicalState {
                patch: portable.clone(),
            },
            crate::UserDataMusicalState {
                patch: portable.clone(),
            },
            crate::UserPreferenceDelta::empty(),
            media_included,
            Vec::new(),
            &defaults,
        );
        assert!(result.is_err(), "mediaIncluded={media_included}");
    }
}
