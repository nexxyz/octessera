use super::*;
use crate::SystemPatchDocuments;
use serde_json::{json, Value};

fn canonical_defaults() -> Value {
    serde_json::from_str(include_str!("../../../../config/defaults/base.json")).unwrap()
}

fn destination_full() -> Value {
    let mut full = canonical_defaults();
    full["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "level": null, "sampleSlot": 1, "x": 7, "y": 7 }
    ]);
    full["runtimeConfig"]["instruments"][0]["sample"]["slots"][1]["path"] =
        json!("samples/Drum/kick/Kick2.wav");
    full
}

fn destination_documents() -> SystemPatchDocuments {
    split_system_patch_documents(&destination_full()).unwrap()
}

fn archive_patches() -> (Value, Value) {
    let defaults = canonical_defaults();
    let mut current = defaults.clone();
    current["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "level": null, "sampleSlot": 3, "x": 2, "y": 4 }
    ]);
    current["runtimeConfig"]["instruments"][0]["sample"]["slots"][3]["path"] =
        json!("samples/Drum/kick/Kick2.wav");
    let mut default = defaults;
    default["runtimeConfig"]["instruments"][0]["sample"]["assignments"] = json!([
        { "level": null, "sampleSlot": 5, "x": 6, "y": 1 }
    ]);
    default["runtimeConfig"]["instruments"][0]["sample"]["slots"][5]["path"] =
        json!("samples/Drum/clap/448221__toxicfilmzsrsounds__hip-hop-clap.wav");
    (
        split_system_patch_documents(&current).unwrap().patch,
        split_system_patch_documents(&default).unwrap().patch,
    )
}

fn reconstruct(
    system: &Value,
    target_patch: &Value,
    current: &Value,
    default: &Value,
    preferences: &UserPreferenceDelta,
) -> Result<UserDataArchiveReconstruction, String> {
    reconstruct_user_data_archive(
        &canonical_defaults(),
        system,
        target_patch,
        current,
        default,
        preferences,
    )
}

#[test]
fn reconstruct_user_data_archive_uses_archive_music_not_destination_music() {
    let destination = destination_documents();
    let (current, default) = archive_patches();
    assert_ne!(destination.patch, current);
    assert_ne!(destination.patch, default);

    let result = reconstruct(
        &destination.system,
        &destination.patch,
        &current,
        &default,
        &UserPreferenceDelta::empty(),
    )
    .unwrap();

    assert_eq!(
        result.current_patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"][0]
            ["sampleSlot"],
        3
    );
    assert_eq!(
        result.default_patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"][0]
            ["sampleSlot"],
        5
    );
    assert_eq!(
        result.current_patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][3]["path"],
        "samples/Drum/kick/Kick2.wav"
    );
    assert_eq!(
        result.default_patch["runtimeConfig"]["instruments"][0]["sample"]["slots"][5]["path"],
        "samples/Drum/clap/448221__toxicfilmzsrsounds__hip-hop-clap.wav"
    );
    assert_eq!(
        result.current_patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"],
        current["runtimeConfig"]["instruments"][0]["sample"]["assignments"]
    );
    assert_eq!(
        result.default_patch["runtimeConfig"]["instruments"][0]["sample"]["assignments"],
        default["runtimeConfig"]["instruments"][0]["sample"]["assignments"]
    );
}

#[test]
fn reconstruct_user_data_archive_applies_midi_sync_preference_but_keeps_local_identity() {
    let destination = destination_documents();
    let (current, default) = archive_patches();
    let mut system = destination.system;
    system["runtimeConfig"]["midi"]["inId"] = json!("local-midi-in");
    system["runtimeConfig"]["midi"]["outId"] = json!("local-midi-out");
    system["runtimeConfig"]["sampleFavouriteDirs"] = json!(["/local/favourites"]);
    let mut preferences = UserPreferenceDelta::empty();
    preferences
        .values
        .insert("midi".into(), json!({ "syncMode": "external" }));

    let result = reconstruct(
        &system,
        &destination.patch,
        &current,
        &default,
        &preferences,
    )
    .unwrap();

    assert_eq!(
        result.system["runtimeConfig"]["midi"]["syncMode"],
        "external"
    );
    assert_eq!(
        result.system["runtimeConfig"]["midi"]["inId"],
        "local-midi-in"
    );
    assert_eq!(
        result.system["runtimeConfig"]["midi"]["outId"],
        "local-midi-out"
    );
    assert_eq!(
        result.system["runtimeConfig"]["sampleFavouriteDirs"],
        json!(["/local/favourites"])
    );
}

#[test]
fn reconstruct_user_data_archive_restores_explicit_and_empty_brightness_delta() {
    let destination = destination_documents();
    let (current, default) = archive_patches();
    let mut system = destination.system.clone();
    system["runtimeConfig"]["displayBrightness"] = json!(12);
    let mut preferences = UserPreferenceDelta::empty();
    preferences
        .values
        .insert("displayBrightness".into(), json!(42));
    let restored = reconstruct(
        &system,
        &destination.patch,
        &current,
        &default,
        &preferences,
    )
    .unwrap();
    assert_eq!(restored.system["runtimeConfig"]["displayBrightness"], 42);

    let reset = reconstruct(
        &system,
        &destination.patch,
        &current,
        &default,
        &UserPreferenceDelta::empty(),
    )
    .unwrap();
    assert_eq!(
        reset.system["runtimeConfig"]["displayBrightness"],
        canonical_defaults()["runtimeConfig"]["displayBrightness"]
    );
}

#[test]
fn reconstruct_user_data_archive_blocks_host_usb_preferences_but_restores_other_preferences() {
    let mut destination = destination_full();
    destination["runtimeConfig"]["usb"]["dataRole"] = json!("host");
    destination["runtimeConfig"]["usb"]["midiOutEnabled"] = json!(false);
    destination["runtimeConfig"]["audioOutputs"]["usb"] = json!(false);
    let destination = split_system_patch_documents(&destination).unwrap();
    let (current, default) = archive_patches();
    let mut preferences = UserPreferenceDelta::empty();
    preferences
        .values
        .insert("audioOutputs".into(), json!({ "usb": true }));
    preferences
        .values
        .insert("usb".into(), json!({ "midiOutEnabled": true }));
    preferences
        .values
        .insert("displayBrightness".into(), json!(42));

    let result = reconstruct(
        &destination.system,
        &destination.patch,
        &current,
        &default,
        &preferences,
    )
    .unwrap();

    assert_eq!(result.system["runtimeConfig"]["usb"]["dataRole"], "host");
    assert_eq!(
        result.system["runtimeConfig"]["usb"]["midiOutEnabled"],
        false
    );
    assert_eq!(result.system["runtimeConfig"]["audioOutputs"]["usb"], false);
    assert_eq!(result.system["runtimeConfig"]["displayBrightness"], 42);
}

#[test]
fn reconstruct_user_data_archive_rejects_shift_aux_same_side_conflicts() {
    let mut target = destination_full();
    target["runtimeConfig"]["shiftAuxBindings"]["aux2"]["pressAction"] =
        json!({ "kind": "platform_effect", "action": "midi.panic" });
    let target = split_system_patch_documents(&target).unwrap();
    let mut archive = canonical_defaults();
    archive["runtimeConfig"]["shiftAuxBindings"]["aux2"]["pressAction"] =
        json!({ "kind": "behavior_action", "actionType": "source.shift" });
    let archive_patch = split_system_patch_documents(&archive).unwrap().patch;

    let error = reconstruct(
        &target.system,
        &target.patch,
        &archive_patch,
        &archive_patch,
        &UserPreferenceDelta::empty(),
    )
    .unwrap_err();
    assert!(error.contains("shiftAuxBindings.aux2.pressAction"));
}

#[test]
fn reconstruct_user_data_archive_rejects_malformed_target_archive_and_preferences() {
    let destination = destination_documents();
    let (current, default) = archive_patches();
    let mut malformed_target = destination.system.clone();
    malformed_target["schemaVersion"] = json!(2);
    assert!(reconstruct(
        &malformed_target,
        &destination.patch,
        &current,
        &default,
        &UserPreferenceDelta::empty()
    )
    .is_err());

    let mut malformed_archive = current.clone();
    malformed_archive["schemaVersion"] = json!(99);
    assert!(reconstruct(
        &destination.system,
        &destination.patch,
        &malformed_archive,
        &default,
        &UserPreferenceDelta::empty()
    )
    .is_err());

    let mut preferences = UserPreferenceDelta::empty();
    preferences
        .values
        .insert("unknownPreference".into(), json!(true));
    assert!(reconstruct(
        &destination.system,
        &destination.patch,
        &current,
        &default,
        &preferences
    )
    .is_err());
}

#[test]
fn reconstruct_user_data_archive_accepts_present_nullable_midi_endpoint_ids() {
    let destination = destination_documents();
    assert!(destination.system["runtimeConfig"]["midi"]
        .as_object()
        .unwrap()
        .contains_key("inId"));
    assert!(destination.system["runtimeConfig"]["midi"]
        .as_object()
        .unwrap()
        .contains_key("outId"));
    assert!(destination.system["runtimeConfig"]["midi"]["inId"].is_null());
    assert!(destination.system["runtimeConfig"]["midi"]["outId"].is_null());
    let (current, default) = archive_patches();
    reconstruct(
        &destination.system,
        &destination.patch,
        &current,
        &default,
        &UserPreferenceDelta::empty(),
    )
    .unwrap();
}
