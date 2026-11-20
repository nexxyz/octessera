use super::config_documents::SYSTEM_DOCUMENT_KIND;
use super::CONFIG_KIND;
use serde_json::Value;

pub(super) const SYSTEM_DOCUMENTS: &[&str] = &[SYSTEM_DOCUMENT_KIND, CONFIG_KIND];
#[cfg(test)]
pub(super) const PATCH_DOCUMENTS: &[&str] = &[super::PATCH_KIND, CONFIG_KIND];

struct AddedKey {
    kinds: &'static [&'static str],
    path: &'static [&'static str],
    default: fn() -> Value,
}

const ADDED_RUNTIME_KEYS: [AddedKey; 4] = [
    AddedKey {
        kinds: SYSTEM_DOCUMENTS,
        path: &["midi", "notesIn"],
        default: || Value::Bool(true),
    },
    AddedKey {
        kinds: SYSTEM_DOCUMENTS,
        path: &["midi", "sendInstruments"],
        default: || Value::String("midi".into()),
    },
    AddedKey {
        kinds: SYSTEM_DOCUMENTS,
        path: &["perfViewAtBoot"],
        default: || Value::Bool(false),
    },
    AddedKey {
        kinds: SYSTEM_DOCUMENTS,
        path: &["perfViewOnBack"],
        default: || Value::Bool(true),
    },
];

/// Loading fills each runtime key in `ADDED_RUNTIME_KEYS` with its default when a document of
/// one of its kinds was saved before the key existed, so an update carries old settings over.
/// An entry stays for good. Removing, renaming, or re-ranging a saved key still means
/// normalizing the existing stores, unless the key only needs dropping (`retired_config_keys`).
pub(crate) fn with_added_keys(document: &Value) -> Value {
    with_runtime_keys(document, &ADDED_RUNTIME_KEYS)
}

pub(super) fn is_kind(document: &Value, kinds: &[&str]) -> bool {
    document
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kinds.contains(&kind))
}

fn with_runtime_keys(document: &Value, keys: &[AddedKey]) -> Value {
    let mut upgraded = document.clone();
    for key in keys.iter().filter(|key| is_kind(document, key.kinds)) {
        let Some((name, parents)) = key.path.split_last() else {
            continue;
        };
        let parent = upgraded.get_mut("runtimeConfig").and_then(|runtime| {
            parents
                .iter()
                .try_fold(runtime, |value, parent| value.get_mut(*parent))
        });
        if let Some(object) = parent.and_then(Value::as_object_mut) {
            object.entry(name.to_string()).or_insert_with(key.default);
        }
    }
    upgraded
}

#[cfg(test)]
mod tests {
    use super::super::{compose_local_system_patch_documents, split_system_patch_documents};
    use super::{
        with_added_keys, with_runtime_keys, AddedKey, ADDED_RUNTIME_KEYS, PATCH_DOCUMENTS,
        SYSTEM_DOCUMENTS,
    };
    use serde_json::{json, Value};

    const PI_DEFAULT: &str = include_str!("../../../../config/generated/pi/default.json");
    const DESKTOP_DEFAULT: &str = include_str!("../../../../config/generated/desktop/default.json");

    fn value_at<'a>(document: &'a Value, path: &[&str]) -> Option<&'a Value> {
        path.iter()
            .try_fold(&document["runtimeConfig"], |value, key| value.get(*key))
    }

    #[test]
    fn every_added_key_defaults_to_the_shipped_value() {
        for shipped in [PI_DEFAULT, DESKTOP_DEFAULT] {
            let full: Value = serde_json::from_str(shipped).unwrap();
            for key in &ADDED_RUNTIME_KEYS {
                assert_eq!(value_at(&full, key.path), Some(&(key.default)()));
            }
        }
    }

    #[test]
    fn a_system_saved_before_an_added_key_loads_with_its_default() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut old_system = documents.system.clone();
        old_system["runtimeConfig"]["midi"]
            .as_object_mut()
            .unwrap()
            .remove("notesIn");

        assert_eq!(with_added_keys(&old_system), documents.system);
        let composed = compose_local_system_patch_documents(&old_system, &documents.patch)
            .expect("an old System without midi.notesIn loads");
        assert_eq!(composed["runtimeConfig"]["midi"]["notesIn"], json!(true));
    }

    #[test]
    fn an_added_key_keeps_a_saved_value_and_stays_in_its_own_document() {
        let keys = [
            AddedKey {
                kinds: SYSTEM_DOCUMENTS,
                path: &["midi", "notesIn"],
                default: || Value::Bool(true),
            },
            AddedKey {
                kinds: PATCH_DOCUMENTS,
                path: &["patchOnly"],
                default: || json!(1),
            },
        ];
        let system = json!({
            "kind": "octessera.system",
            "runtimeConfig": { "midi": { "notesIn": false } }
        });
        assert_eq!(with_runtime_keys(&system, &keys), system);
        let patch = json!({ "kind": "octessera.patch", "runtimeConfig": {} });
        assert_eq!(
            with_runtime_keys(&patch, &keys),
            json!({ "kind": "octessera.patch", "runtimeConfig": { "patchOnly": 1 } })
        );
    }
}
