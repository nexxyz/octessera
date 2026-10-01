use super::*;
use serde_json::json;

const DESKTOP_DEFAULT: &str = include_str!("../../../../config/generated/desktop/default.json");
const PI_DEFAULT: &str = include_str!("../../../../config/generated/pi/default.json");

fn distinct_full_config(source: &str) -> Value {
    let mut config: Value = serde_json::from_str(source).unwrap();
    config["revision"] = json!(913);
    let runtime = &mut config["runtimeConfig"];
    runtime["masterVolume"] = json!(37);
    runtime["sampleFavouriteDirs"] = json!(["local/favourites/only"]);
    runtime["displayBrightness"] = json!(73);
    runtime["inputEventsWhilePaused"] = json!(true);
    runtime["midi"]["outId"] = json!("device-midi-out");
    runtime["midi"]["inId"] = json!("device-midi-in");
    runtime["playMode"] = json!("pan");
    runtime["noteLengthMs"] = json!(777);
    runtime["sound"]["noteLengthMs"] = json!(777);
    runtime["velocityScalePct"] = json!(143);
    runtime["sound"]["velocityScalePct"] = json!(143);
    runtime["bpm"] = json!(137.25);
    runtime["transport"]["bpm"] = json!(137.25);
    runtime["instruments"][0]["midi"]["channel"] = json!(13);
    runtime["instruments"][0]["mixer"]["volume"] = json!(31);
    runtime["instruments"][platform_core::INSTRUMENT_COUNT - 1]["mixer"]["volume"] = json!(79);
    runtime["layers"][0]["name"] = json!("first-patch-layer");
    runtime["layers"][0]["autoName"] = json!(false);
    runtime["layers"][platform_core::LAYER_COUNT - 1]["name"] = json!("last-patch-layer");
    runtime["layers"][platform_core::LAYER_COUNT - 1]["autoName"] = json!(false);
    runtime["auxBindings"] = json!({
        "aux1": {
            "turnKey": "sound.noteLengthMs",
            "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
        },
        "aux2": {
            "turnKey": "displayBrightness",
            "pressAction": { "kind": "behavior_action", "actionType": "source.patch" }
        }
    });
    runtime["shiftAuxBindings"] = json!({
        "aux1": {
            "turnKey": "sound.velocityScalePct",
            "pressAction": { "kind": "behavior_action", "actionType": "source.shift" }
        },
        "aux2": {
            "turnKey": "gridBrightness",
            "pressAction": { "kind": "platform_effect", "action": "midi.panic" }
        }
    });
    config["system"]["playMode"] = json!("pan");
    config
}

fn assert_round_trip(source: &str) {
    let full = distinct_full_config(source);
    let normalized = normalize_full_config(&full).unwrap();
    let documents = split_system_patch_documents(&full).unwrap();

    assert_eq!(documents.system["kind"], "octessera.system");
    assert_eq!(documents.system["schemaVersion"], SYSTEM_DOCUMENT_VERSION);
    assert_eq!(documents.patch["kind"], PATCH_KIND);
    assert_eq!(documents.patch["schemaVersion"], CONFIG_SCHEMA_VERSION);
    assert!(documents.system.get("revision").is_none());
    assert!(documents.patch.get("revision").is_none());
    assert_eq!(documents.patch["mappingConfig"], full["mappingConfig"]);
    assert_eq!(documents.patch["runtimeConfig"]["playMode"], "pan");
    assert_eq!(documents.system["runtimeConfig"]["masterVolume"], 37);
    assert_eq!(documents.system["runtimeConfig"]["displayBrightness"], 73);
    assert_eq!(
        documents.system["runtimeConfig"]["inputEventsWhilePaused"],
        true
    );
    assert_eq!(
        documents.system["runtimeConfig"]["sampleFavouriteDirs"],
        json!(["local/favourites/only"])
    );
    assert_eq!(
        documents.system["runtimeConfig"]["midi"]["outId"],
        "device-midi-out"
    );
    assert_eq!(documents.patch["runtimeConfig"]["noteLengthMs"], 777);
    assert_eq!(
        documents.patch["runtimeConfig"]["sound"]["noteLengthMs"],
        777
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["sound"]["voiceStealingMode"],
        full["runtimeConfig"]["sound"]["voiceStealingMode"]
    );
    assert_eq!(
        documents.system["runtimeConfig"]["sound"]["audioOutputBufferFrames"],
        full["runtimeConfig"]["sound"]["audioOutputBufferFrames"]
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"][0]["midi"]["channel"],
        13
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"]
            .as_array()
            .unwrap()
            .len(),
        platform_core::INSTRUMENT_COUNT
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["layers"]
            .as_array()
            .unwrap()
            .len(),
        platform_core::LAYER_COUNT
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["layers"][platform_core::LAYER_COUNT - 1]["name"],
        "last-patch-layer"
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["instruments"][platform_core::INSTRUMENT_COUNT - 1]
            ["mixer"]["volume"],
        79
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "sound.noteLengthMs"
    );
    assert!(documents.patch["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"].is_null());
    assert_eq!(
        documents.system["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"]["action"],
        "midi.panic"
    );
    assert_eq!(
        documents.system["runtimeConfig"]["auxBindings"]["aux2"]["turnKey"],
        "displayBrightness"
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["auxBindings"]["aux2"]["pressAction"]["actionType"],
        "source.patch"
    );
    assert_eq!(
        documents.patch["runtimeConfig"]["shiftAuxBindings"]["aux1"]["pressAction"]["actionType"],
        "source.shift"
    );
    assert_eq!(
        documents.system["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"],
        "gridBrightness"
    );
    assert!(documents.patch["runtimeConfig"]["instruments"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|instrument| instrument["sample"]["slots"]
            .as_array()
            .into_iter()
            .flatten())
        .any(|slot| slot["path"].as_str().is_some()));

    let composed = compose_system_patch_documents(&documents.system, &documents.patch).unwrap();
    assert!(composed.get("revision").is_none());
    assert_eq!(composed["runtimeConfig"], normalized["runtimeConfig"]);
    assert_eq!(composed["system"]["playMode"], "pan");
    assert_eq!(
        composed["runtimeConfig"]["activeBehavior"],
        full["runtimeConfig"]["activeBehavior"]
    );
    assert_eq!(
        composed["runtimeConfig"]["activeLayerIndex"],
        full["runtimeConfig"]["activeLayerIndex"]
    );
    assert_eq!(
        composed["runtimeConfig"]["playMode"],
        full["runtimeConfig"]["playMode"]
    );
    assert_eq!(composed["runtimeConfig"]["inputEventsWhilePaused"], true);
    assert_eq!(
        composed["runtimeConfig"]["layers"],
        normalized["runtimeConfig"]["layers"]
    );
    assert_eq!(
        composed["runtimeConfig"]["instruments"],
        normalized["runtimeConfig"]["instruments"]
    );
    assert_eq!(
        composed["runtimeConfig"]["sound"],
        normalized["runtimeConfig"]["sound"]
    );
    assert_eq!(composed["mappingConfig"], full["mappingConfig"]);
    assert_eq!(
        composed["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "sound.noteLengthMs"
    );
    assert_eq!(
        composed["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"]["action"],
        "midi.panic"
    );
    assert_eq!(
        composed["runtimeConfig"]["shiftAuxBindings"]["aux2"]["turnKey"],
        "gridBrightness"
    );
    assert_eq!(
        composed["runtimeConfig"]["shiftAuxBindings"]["aux2"]["pressAction"]["action"],
        "midi.panic"
    );

    assert_eq!(split_system_patch_documents(&composed).unwrap(), documents);
}

#[test]
fn generated_pi_and_desktop_defaults_split_and_compose_without_losing_owned_data() {
    assert_round_trip(DESKTOP_DEFAULT);
    assert_round_trip(PI_DEFAULT);
}

#[test]
fn composition_preserves_the_other_aux_side_when_one_document_clears_its_side() {
    let full = distinct_full_config(PI_DEFAULT);
    let documents = split_system_patch_documents(&full).unwrap();
    let mut cleared_system = documents.system.clone();
    cleared_system["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"] = Value::Null;

    let composed = compose_system_patch_documents(&cleared_system, &documents.patch).unwrap();

    assert_eq!(
        composed["runtimeConfig"]["auxBindings"]["aux1"]["turnKey"],
        "sound.noteLengthMs"
    );
    assert!(composed["runtimeConfig"]["auxBindings"]["aux1"]
        .get("pressAction")
        .is_none());

    let mut cleared_patch = documents.patch.clone();
    cleared_patch["runtimeConfig"]["auxBindings"]["aux1"] = Value::Null;

    let composed = compose_system_patch_documents(&documents.system, &cleared_patch).unwrap();

    assert_eq!(
        composed["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"]["action"],
        "midi.panic"
    );
    assert!(composed["runtimeConfig"]["auxBindings"]["aux1"]
        .get("turnKey")
        .is_none());
}

#[test]
fn present_null_or_empty_midi_ids_and_omitted_saved_state_round_trip() {
    let mut full = distinct_full_config(PI_DEFAULT);
    full["runtimeConfig"]["midi"]["outId"] = Value::Null;
    full["runtimeConfig"]["midi"]["inId"] = json!("");
    let build = full["runtimeConfig"]["layers"][0]["build"]
        .as_object_mut()
        .unwrap();
    build.remove("savedState");
    assert!(build.get("savedState").is_none());

    let normalized = normalize_full_config(&full).unwrap();
    let documents = split_system_patch_documents(&full).unwrap();
    assert!(documents.system["runtimeConfig"]["midi"]["outId"].is_null());
    assert_eq!(documents.system["runtimeConfig"]["midi"]["inId"], "");
    assert!(documents.patch["runtimeConfig"]["layers"][0]["build"]
        .get("savedState")
        .is_none());
    let composed = compose_system_patch_documents(&documents.system, &documents.patch).unwrap();
    assert_eq!(composed["runtimeConfig"], normalized["runtimeConfig"]);
    assert_eq!(split_system_patch_documents(&composed).unwrap(), documents);
}

#[test]
fn composition_rejects_same_aux_side_claims_from_both_documents() {
    let documents = split_system_patch_documents(&distinct_full_config(PI_DEFAULT)).unwrap();
    for (bank, slot, side, value) in [
        ("auxBindings", "aux1", "turnKey", json!("displayBrightness")),
        (
            "shiftAuxBindings",
            "aux1",
            "pressAction",
            json!({ "kind": "platform_effect", "action": "midi.panic" }),
        ),
    ] {
        let mut system = documents.system.clone();
        system["runtimeConfig"][bank][slot][side] = value;
        let error = compose_system_patch_documents(&system, &documents.patch).unwrap_err();
        assert!(error.contains(&format!("{bank}.{slot}.{side}")));
    }
}

#[test]
fn presence_checks_reject_missing_snapshot_fields_before_projection() {
    let full = distinct_full_config(PI_DEFAULT);
    for field in ["displayBrightness", "autoSaveDefault", "dsp"] {
        let mut incomplete_full = full.clone();
        incomplete_full["runtimeConfig"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(split_system_patch_documents(&incomplete_full).is_err());

        let documents = split_system_patch_documents(&full).unwrap();
        let mut incomplete_system = documents.system.clone();
        incomplete_system["runtimeConfig"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(compose_system_patch_documents(&incomplete_system, &documents.patch).is_err());
    }
}

#[test]
fn presence_checks_require_patch_transport_playfx_and_musical_sound_member() {
    let documents = split_system_patch_documents(&distinct_full_config(PI_DEFAULT)).unwrap();
    for field in ["transport", "playFx"] {
        let mut patch = documents.patch.clone();
        patch["runtimeConfig"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert!(compose_system_patch_documents(&documents.system, &patch).is_err());
    }
    let mut patch = documents.patch.clone();
    patch["runtimeConfig"]["sound"]
        .as_object_mut()
        .unwrap()
        .remove("voiceStealingMode");
    assert!(compose_system_patch_documents(&documents.system, &patch).is_err());
}

#[test]
fn presence_checks_require_both_aux_bank_containers_in_both_documents() {
    let documents = split_system_patch_documents(&distinct_full_config(PI_DEFAULT)).unwrap();
    for bank in ["auxBindings", "shiftAuxBindings"] {
        let mut system = documents.system.clone();
        system["runtimeConfig"]
            .as_object_mut()
            .unwrap()
            .remove(bank);
        assert!(compose_system_patch_documents(&system, &documents.patch).is_err());

        let mut patch = documents.patch.clone();
        patch["runtimeConfig"].as_object_mut().unwrap().remove(bank);
        assert!(compose_system_patch_documents(&documents.system, &patch).is_err());
    }
}

#[test]
fn split_and_composition_reject_wrong_envelopes_dropped_fields_and_invalid_samples() {
    let full = distinct_full_config(PI_DEFAULT);
    let documents = split_system_patch_documents(&full).unwrap();

    let mut wrong_system_kind = documents.system.clone();
    wrong_system_kind["kind"] = json!("octessera.patch");
    assert!(compose_system_patch_documents(&wrong_system_kind, &documents.patch).is_err());
    let mut wrong_system_version = documents.system.clone();
    wrong_system_version["schemaVersion"] = json!(2);
    assert!(compose_system_patch_documents(&wrong_system_version, &documents.patch).is_err());
    let mut wrong_patch_kind = documents.patch.clone();
    wrong_patch_kind["kind"] = json!("octessera.config");
    assert!(compose_system_patch_documents(&documents.system, &wrong_patch_kind).is_err());
    let mut wrong_patch_version = documents.patch.clone();
    wrong_patch_version["schemaVersion"] = json!(1);
    assert!(compose_system_patch_documents(&documents.system, &wrong_patch_version).is_err());
    let mut unknown_patch_field = documents.patch.clone();
    unknown_patch_field["runtimeConfig"]["futureField"] = json!(true);
    assert!(compose_system_patch_documents(&documents.system, &unknown_patch_field).is_err());
    let mut unknown_system_field = documents.system.clone();
    unknown_system_field["runtimeConfig"]["futureField"] = json!(true);
    assert!(compose_system_patch_documents(&unknown_system_field, &documents.patch).is_err());
    let mut patch_revision = documents.patch.clone();
    patch_revision["revision"] = json!(14);
    assert!(compose_system_patch_documents(&documents.system, &patch_revision).is_err());
    let mut missing_system_runtime = documents.system.clone();
    missing_system_runtime
        .as_object_mut()
        .unwrap()
        .remove("runtimeConfig");
    assert!(compose_system_patch_documents(&missing_system_runtime, &documents.patch).is_err());
    let mut missing_system_sound = documents.system.clone();
    missing_system_sound["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("sound");
    assert!(compose_system_patch_documents(&missing_system_sound, &documents.patch).is_err());
    let mut missing_master_volume = documents.system.clone();
    missing_master_volume["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("masterVolume");
    assert!(compose_system_patch_documents(&missing_master_volume, &documents.patch).is_err());
    let mut missing_usb_role = documents.system.clone();
    missing_usb_role["runtimeConfig"]["usb"]
        .as_object_mut()
        .unwrap()
        .remove("dataRole");
    assert!(compose_system_patch_documents(&missing_usb_role, &documents.patch).is_err());
    let mut missing_audio_output = documents.system.clone();
    missing_audio_output["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("audioOutputs");
    assert!(compose_system_patch_documents(&missing_audio_output, &documents.patch).is_err());
    let mut missing_hdmi_output = documents.system.clone();
    missing_hdmi_output["runtimeConfig"]["audioOutputs"]
        .as_object_mut()
        .unwrap()
        .remove("hdmi");
    assert!(compose_system_patch_documents(&missing_hdmi_output, &documents.patch).is_err());
    let mut missing_audio_buffer = documents.system.clone();
    missing_audio_buffer["runtimeConfig"]["sound"]
        .as_object_mut()
        .unwrap()
        .remove("audioOutputBufferFrames");
    assert!(compose_system_patch_documents(&missing_audio_buffer, &documents.patch).is_err());
    let mut missing_voice_stealing_mode = documents.patch.clone();
    missing_voice_stealing_mode["runtimeConfig"]["sound"]
        .as_object_mut()
        .unwrap()
        .remove("voiceStealingMode");
    assert!(
        compose_system_patch_documents(&documents.system, &missing_voice_stealing_mode).is_err()
    );
    let mut invalid_sample = documents.patch.clone();
    let slot = invalid_sample["runtimeConfig"]["instruments"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .flat_map(|instrument| instrument["sample"]["slots"].as_array_mut().unwrap())
        .find(|slot| slot["path"].as_str().is_some())
        .unwrap();
    slot["path"] = json!("samples/../outside.wav");
    assert!(compose_system_patch_documents(&documents.system, &invalid_sample).is_err());
}

#[test]
fn split_requires_an_owned_complete_v2_full_config_and_reconstructs_system_play_mode() {
    let full = distinct_full_config(DESKTOP_DEFAULT);
    let mut wrong_kind = full.clone();
    wrong_kind["kind"] = json!("octessera.patch");
    assert!(split_system_patch_documents(&wrong_kind).is_err());
    let mut wrong_version = full.clone();
    wrong_version["schemaVersion"] = json!(1);
    assert!(split_system_patch_documents(&wrong_version).is_err());
    let mut missing_master_volume = full.clone();
    missing_master_volume["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("masterVolume");
    assert!(split_system_patch_documents(&missing_master_volume).is_err());
    let mut missing_voice_stealing = full.clone();
    missing_voice_stealing["runtimeConfig"]["sound"]
        .as_object_mut()
        .unwrap()
        .remove("voiceStealingMode");
    assert!(split_system_patch_documents(&missing_voice_stealing).is_err());
    let mut unknown_field = full.clone();
    unknown_field["runtimeConfig"]["futureField"] = json!(true);
    assert!(split_system_patch_documents(&unknown_field).is_err());
    let mut unknown_root = full.clone();
    unknown_root["futureField"] = json!(true);
    assert!(split_system_patch_documents(&unknown_root).is_err());
    let mut missing_layers = full.clone();
    missing_layers["runtimeConfig"]
        .as_object_mut()
        .unwrap()
        .remove("layers");
    assert!(split_system_patch_documents(&missing_layers).is_err());
    let mut mismatched_play_mode = full;
    mismatched_play_mode["system"]["playMode"] = json!("fx");
    assert!(split_system_patch_documents(&mismatched_play_mode).is_err());
    let mut non_object_system = distinct_full_config(DESKTOP_DEFAULT);
    non_object_system["system"] = json!("pan");
    assert!(split_system_patch_documents(&non_object_system).is_err());
    let mut extra_system_field = distinct_full_config(DESKTOP_DEFAULT);
    extra_system_field["system"]["futureField"] = json!(true);
    assert!(split_system_patch_documents(&extra_system_field).is_err());
}
