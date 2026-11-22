use super::added_config_keys::{for_each_object, is_kind, PATCH_DOCUMENTS};
use super::config_documents::SYSTEM_DOCUMENT_KIND;
use serde_json::Value;

const SYSTEM_DOCUMENT: &[&str] = &[SYSTEM_DOCUMENT_KIND];

type RetiredKey = (
    &'static [&'static str],
    &'static [&'static str],
    fn(&Value) -> bool,
);

const RETIRED_RUNTIME_KEYS: [RetiredKey; 4] = [
    (SYSTEM_DOCUMENT, &["auxBindings"], holds_no_binding),
    (SYSTEM_DOCUMENT, &["shiftAuxBindings"], holds_no_binding),
    (PATCH_DOCUMENTS, &["sound", "noteLengthMs"], |_| true),
    (
        PATCH_DOCUMENTS,
        &["instruments", "*", "midi", "durationMs"],
        |_| true,
    ),
];

/// Loading drops each runtime key in `RETIRED_RUNTIME_KEYS` from documents of the listed kinds
/// when its saved value passes the entry's check (a `*` step stands for every array item): a
/// System saved by 0.8.14 still carries empty Aux banks that only the patch owns now, while a
/// System with real bindings stays rejected, and a patch's global Note Length and per-slot MIDI
/// Duration went when each layer's Link got its One-Shot Len. Use an entry only for a value that
/// can go; anything a save still needs means normalizing the existing stores.
pub(crate) fn without_retired_keys(document: &Value) -> Value {
    without_runtime_keys(document, &RETIRED_RUNTIME_KEYS)
}

fn holds_no_binding(bank: &Value) -> bool {
    bank.as_object()
        .is_some_and(|bank| bank.values().all(Value::is_null))
}

fn without_runtime_keys(document: &Value, keys: &[RetiredKey]) -> Value {
    let mut cleaned = document.clone();
    if let Some(runtime) = cleaned.get_mut("runtimeConfig") {
        for (_, path, retire) in keys.iter().filter(|(kinds, ..)| is_kind(document, kinds)) {
            let Some((key, parents)) = path.split_last() else {
                continue;
            };
            for_each_object(runtime, parents, &mut |parent| {
                if parent.get(*key).is_some_and(retire) {
                    parent.remove(*key);
                }
            });
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::super::added_config_keys::SYSTEM_DOCUMENTS;
    use super::super::{compose_local_system_patch_documents, split_system_patch_documents};
    use super::without_runtime_keys;
    use crate::runtime::CoreRunner;
    use serde_json::{json, Value};

    const PI_DEFAULT: &str = include_str!("../../../../config/generated/pi/default.json");

    #[test]
    fn a_retired_runtime_key_is_dropped_before_the_strict_checks() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut old_system = documents.system.clone();
        old_system["runtimeConfig"]["retiredTestKey"] = json!(true);

        assert!(compose_local_system_patch_documents(&old_system, &documents.patch).is_err());
        let keys: [super::RetiredKey; 1] = [(SYSTEM_DOCUMENTS, &["retiredTestKey"], |_| true)];
        let cleaned = without_runtime_keys(&old_system, &keys);
        assert_eq!(cleaned, documents.system);
        assert!(compose_local_system_patch_documents(&cleaned, &documents.patch).is_ok());
        assert_eq!(
            without_runtime_keys(&documents.patch, &keys),
            documents.patch
        );
    }

    #[test]
    fn a_patch_saved_with_note_length_and_midi_duration_loads_with_one_shot_len() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut old_patch = documents.patch.clone();
        let runtime = &mut old_patch["runtimeConfig"];
        runtime["sound"]["noteLengthMs"] = json!(120);
        runtime["instruments"][4]["midi"]["durationMs"] = json!(200);
        for layer in runtime["layers"].as_array_mut().unwrap() {
            layer["link"]
                .as_object_mut()
                .unwrap()
                .remove("oneShotLengthMs");
        }

        let composed = compose_local_system_patch_documents(&documents.system, &old_patch)
            .expect("a patch from before One-Shot Len loads");
        let runtime = &composed["runtimeConfig"];
        assert!(runtime["sound"].get("noteLengthMs").is_none());
        assert!(runtime["instruments"][4]["midi"]
            .get("durationMs")
            .is_none());
        for layer in runtime["layers"].as_array().unwrap() {
            assert_eq!(layer["link"]["oneShotLengthMs"], json!(150));
        }
    }

    #[test]
    fn a_preset_and_a_backup_with_a_retired_key_load() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut old_patch = documents.patch.clone();
        let runtime = &mut old_patch["runtimeConfig"];
        runtime["sound"]["noteLengthMs"] = json!(120);
        for layer in runtime["layers"].as_array_mut().unwrap() {
            layer["link"]
                .as_object_mut()
                .unwrap()
                .remove("oneShotLengthMs");
        }

        let mut runner = crate::NativeRunner::new(crate::NativeRunnerConfig::default()).unwrap();
        runner
            .send(crate::HostMessage::RuntimeResult {
                result: crate::RuntimeStoreResult::LoadPresetResult {
                    name: "Old".into(),
                    payload: Some(old_patch.clone()),
                },
            })
            .unwrap();
        assert_eq!(
            runner.patch_store.current_preset_name.as_deref(),
            Some("Old")
        );

        let restored = crate::user_data_bundle::reconstruct_user_data_archive(
            &full,
            &documents.system,
            &documents.patch,
            &old_patch,
            &old_patch,
            &crate::UserPreferenceDelta::empty(),
        )
        .expect("a backup with a retired key restores");
        let runtime = &restored.current_patch["runtimeConfig"];
        assert!(runtime["sound"].get("noteLengthMs").is_none());
        for layer in runtime["layers"].as_array().unwrap() {
            assert_eq!(layer["link"]["oneShotLengthMs"], json!(150));
        }
    }

    #[test]
    fn a_system_aux_bank_is_dropped_but_a_patch_keeps_its_own() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut old_system = documents.system.clone();
        let empty = json!({ "aux1": null, "aux2": null, "aux3": null });
        old_system["runtimeConfig"]["auxBindings"] = empty.clone();
        old_system["runtimeConfig"]["shiftAuxBindings"] = empty;

        assert!(compose_local_system_patch_documents(&old_system, &documents.patch).is_ok());
        assert_eq!(
            super::without_retired_keys(&documents.patch),
            documents.patch
        );
    }
}
