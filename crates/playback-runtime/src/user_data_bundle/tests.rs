use super::*;
use serde_json::{json, Value};

fn canonical_defaults() -> Value {
    serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap()
}

fn patch() -> Value {
    json!({
        "kind": "octessera.patch",
        "schemaVersion": 2,
        "runtimeConfig": {}
    })
}

fn bundle(media_included: bool, media: Vec<UserDataMediaReference>) -> UserDataBundle {
    new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: "desktop-simulator".into(),
            runtime_version: "0.8.0".into(),
        },
        vec![UserDataPreset {
            display_name: "Zed".into(),
            patch: patch(),
        }],
        UserDataMusicalState { patch: patch() },
        UserDataMusicalState { patch: patch() },
        UserPreferenceDelta::empty(),
        media_included,
        media,
        &canonical_defaults(),
    )
    .unwrap()
}

#[test]
fn safe_names_preserve_exact_display_names_and_reject_paths() {
    assert!(is_safe_user_data_name("Soft Kit 01"));
    assert!(!is_safe_user_data_name(" Soft Kit 01"));
    assert!(!is_safe_user_data_name("../escape"));
    assert!(!is_safe_user_data_name("CON"));
    assert!(!is_safe_user_data_name(
        &"x".repeat(USER_DATA_MAX_PRESET_NAME_CHARS + 1)
    ));

    let mut presets = vec![UserDataPreset {
        display_name: "Exact Display Name".into(),
        patch: patch(),
    }];
    let result = new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: "desktop".into(),
            runtime_version: "test".into(),
        },
        std::mem::take(&mut presets),
        UserDataMusicalState { patch: patch() },
        UserDataMusicalState { patch: patch() },
        UserPreferenceDelta::empty(),
        false,
        Vec::new(),
        &canonical_defaults(),
    )
    .unwrap();
    assert_eq!(result.presets[0].display_name, "Exact Display Name");
}

#[test]
fn limits_and_unknown_fields_are_rejected() {
    let too_many = (0..=USER_DATA_MAX_PRESETS)
        .map(|index| UserDataPreset {
            display_name: format!("Preset {index}"),
            patch: patch(),
        })
        .collect();
    let error = new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: "desktop".into(),
            runtime_version: "test".into(),
        },
        too_many,
        UserDataMusicalState { patch: patch() },
        UserDataMusicalState { patch: patch() },
        UserPreferenceDelta::empty(),
        false,
        Vec::new(),
        &canonical_defaults(),
    )
    .unwrap_err();
    assert!(error.contains("too many presets"));

    let oversized_media = UserDataMediaReference {
        id: "large".into(),
        kind: UserDataMediaKind::Audio,
        display_name: "large.wav".into(),
        size: USER_DATA_MAX_MEDIA_BYTES + 1,
        sha256: "0".repeat(64),
    };
    let error = new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: "desktop".into(),
            runtime_version: "test".into(),
        },
        Vec::new(),
        UserDataMusicalState { patch: patch() },
        UserDataMusicalState { patch: patch() },
        UserPreferenceDelta::empty(),
        true,
        vec![oversized_media],
        &canonical_defaults(),
    )
    .unwrap_err();
    assert!(error.contains("media reference"));

    let mut value: Value = serde_json::to_value(bundle(false, Vec::new())).unwrap();
    value["futureField"] = json!(true);
    let error =
        decode_user_data_bundle(&serde_json::to_vec(&value).unwrap(), &canonical_defaults())
            .unwrap_err();
    assert!(error.contains("unknown field") || error.contains("futureField"));

    let mut unknown_preference = bundle(false, Vec::new());
    unknown_preference
        .preferences
        .values
        .insert("futurePreference".into(), json!(true));
    let error = validate_user_data_bundle(&unknown_preference, &canonical_defaults()).unwrap_err();
    assert!(error.contains("futurePreference"));

    let mut unsafe_manifest = serde_json::to_value(bundle(false, Vec::new())).unwrap();
    unsafe_manifest["manifest"][0]["path"] = json!("../escape");
    let error = decode_user_data_bundle(
        &serde_json::to_vec(&unsafe_manifest).unwrap(),
        &canonical_defaults(),
    )
    .unwrap_err();
    assert!(error.contains("unsafe"));
}

#[test]
fn preference_delta_is_relative_and_excludes_device_identity_and_paths() {
    let defaults = canonical_defaults();
    let mut current = defaults.clone();
    current["runtimeConfig"]["audioOutputs"] = json!({
        "dac": false,
        "usb": true,
        "hdmi": false
    });
    current["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = json!(1024);
    current["runtimeConfig"]["displayBrightness"] = json!(42);
    current["runtimeConfig"]["sampleFavouriteDirs"] = json!(["/private/device/path"]);
    current["runtimeConfig"]["midi"]["outId"] = json!("device-specific-id");

    let delta = preference_delta_from_config(&current, &defaults).unwrap();
    assert_eq!(
        delta.values["audioOutputs"],
        current["runtimeConfig"]["audioOutputs"]
    );
    assert_eq!(delta.values["sound"]["audioOutputBufferFrames"], 1024);
    assert_eq!(delta.values["displayBrightness"], 42);
    assert!(!delta.values.contains_key("sampleFavouriteDirs"));
    assert!(!delta.values.contains_key("midi"));

    let applied = apply_user_preference_delta(&defaults, &delta).unwrap();
    assert_eq!(applied["runtimeConfig"]["audioOutputs"]["usb"], true);
    assert_eq!(
        applied["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        1024
    );
    assert_eq!(applied["runtimeConfig"]["displayBrightness"], 42);
}

#[test]
fn user_data_rehydration_applies_preferences_before_migrated_portable_patch() {
    let defaults = canonical_defaults();
    let mut preferences = UserPreferenceDelta::empty();
    preferences
        .values
        .insert("displayBrightness".into(), json!(42));
    let patch = json!({
        "kind": "octessera.patch",
        "schemaVersion": 1,
        "runtimeConfig": {
            "masterVolume": 1,
            "layers": [{
                "name": "Portable layer",
                "linkLfo": {
                    "enabled": true,
                    "target": { "key": "instruments.0.mixer.volume", "kind": "number" },
                    "period": "1/4",
                    "depthPct": 37
                }
            }]
        }
    });

    let restored = apply_user_data_patch_and_preferences(&defaults, &patch, &preferences).unwrap();
    assert_eq!(restored["runtimeConfig"]["displayBrightness"], 42);
    assert_eq!(
        restored["runtimeConfig"]["masterVolume"],
        defaults["runtimeConfig"]["masterVolume"]
    );
    assert_eq!(
        restored["runtimeConfig"]["layers"][0]["name"],
        "Portable layer"
    );
    assert_eq!(
        restored["runtimeConfig"]["layers"][1],
        defaults["runtimeConfig"]["layers"][1]
    );
    assert_eq!(restored["runtimeConfig"]["linkLfos"][0]["enabled"], true);
    assert_eq!(restored["runtimeConfig"]["linkLfos"][0]["depthPct"], 37);
}

#[test]
fn optional_media_is_explicit_and_manifested_without_device_paths() {
    let media = media_reference_from_bytes(
        UserDataMediaKind::Sample,
        "kit-kick".into(),
        "Kick.wav".into(),
        b"sample bytes",
    )
    .unwrap();
    let error = new_user_data_bundle(
        UserDataBundleMetadata {
            board_profile: "desktop".into(),
            runtime_version: "test".into(),
        },
        Vec::new(),
        UserDataMusicalState { patch: patch() },
        UserDataMusicalState { patch: patch() },
        UserPreferenceDelta::empty(),
        false,
        vec![media.clone()],
        &canonical_defaults(),
    )
    .unwrap_err();
    assert!(error.contains("mediaIncluded"));

    let result = bundle(true, vec![media]);
    assert!(result.media_included);
    assert!(result
        .manifest
        .iter()
        .any(|entry| entry.path == "media/sample/kit-kick"));
    let bytes = encode_user_data_bundle(&result).unwrap();
    let restored = decode_user_data_bundle(&bytes, &canonical_defaults()).unwrap();
    assert_eq!(restored, result);
}

#[test]
fn media_reference_from_bytes_hashes_and_validates_content() {
    let reference = media_reference_from_bytes(
        UserDataMediaKind::Audio,
        "kick".into(),
        "Kick.wav".into(),
        b"sample bytes",
    )
    .unwrap();
    assert_eq!(reference.size, 12);
    assert_eq!(
        reference.sha256,
        "8e13f6c598de092f99affe5c64d3aca48f4b4e0bea6e396bc257c40482674e3a"
    );

    let error = media_reference_from_bytes(
        UserDataMediaKind::Audio,
        "../escape".into(),
        "Kick.wav".into(),
        b"sample bytes",
    )
    .unwrap_err();
    assert!(error.contains("media id"));
}

#[test]
fn migration_adds_envelope_and_migrates_legacy_patch_schema() {
    let legacy = json!({
        "presets": [{"name": "Legacy", "payload": {
            "kind": "octessera.patch",
            "schemaVersion": 1,
            "runtimeConfig": {}
        }}],
        "current": {
            "kind": "octessera.patch",
            "schemaVersion": 1,
            "runtimeConfig": {}
        },
        "default": {
            "kind": "octessera.patch",
            "schemaVersion": 1,
            "runtimeConfig": {}
        }
    });
    let migrated = migrate_user_data_bundle(legacy, &canonical_defaults()).unwrap();
    assert_eq!(migrated.kind, USER_DATA_BUNDLE_KIND);
    assert_eq!(migrated.schema_version, USER_DATA_BUNDLE_SCHEMA_VERSION);
    assert_eq!(migrated.presets[0].display_name, "Legacy");
    assert_eq!(migrated.presets[0].patch["schemaVersion"], 2);
    assert_eq!(migrated.current_state.patch["schemaVersion"], 2);
    assert!(!migrated.media_included);
}

#[test]
fn bundle_encoding_is_deterministic_and_hashes_are_stable() {
    let first = bundle(false, Vec::new());
    let mut second = first.clone();
    second.presets.reverse();
    second.manifest = manifest_for_user_data_bundle(&second).unwrap();
    assert_eq!(
        encode_user_data_bundle(&first).unwrap(),
        encode_user_data_bundle(&second).unwrap()
    );
    assert!(first
        .manifest
        .iter()
        .all(|entry| entry.sha256.len() == 64 && entry.sha256.chars().all(|c| !c.is_uppercase())));
}
