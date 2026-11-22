use super::config_documents::SYSTEM_DOCUMENT_KIND;
use super::{CONFIG_KIND, DEFAULT_ONE_SHOT_LENGTH_MS};
use serde_json::{Map, Value};

pub(super) const SYSTEM_DOCUMENTS: &[&str] = &[SYSTEM_DOCUMENT_KIND, CONFIG_KIND];
pub(super) const PATCH_DOCUMENTS: &[&str] = &[super::PATCH_KIND, CONFIG_KIND];

struct AddedKey {
    kinds: &'static [&'static str],
    path: &'static [&'static str],
    default: fn() -> Value,
}

const ADDED_RUNTIME_KEYS: [AddedKey; 5] = [
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
    AddedKey {
        kinds: PATCH_DOCUMENTS,
        path: &["layers", "*", "link", "oneShotLengthMs"],
        default: || Value::from(DEFAULT_ONE_SHOT_LENGTH_MS),
    },
];

/// Loading fills each runtime key in `ADDED_RUNTIME_KEYS` with its default when a document of
/// one of its kinds was saved before the key existed, so an update carries old settings over;
/// a `*` step stands for every item of an array. An entry stays for good. Removing, renaming,
/// or re-ranging a saved key still means normalizing the existing stores, unless the key only
/// needs dropping (`retired_config_keys`).
/// Patches saved before every instrument carried a tile stamp with a `level` get the
/// defaults the same way.
pub(crate) fn with_added_keys(document: &Value) -> Value {
    let mut upgraded = with_runtime_keys(document, &ADDED_RUNTIME_KEYS);
    if is_kind(document, &[super::PATCH_KIND, CONFIG_KIND]) {
        super::config_schema::normalize_missing_tile_stamps(&mut upgraded);
    }
    upgraded
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
        if let Some(runtime) = upgraded.get_mut("runtimeConfig") {
            for_each_object(runtime, parents, &mut |object| {
                object.entry(name.to_string()).or_insert_with(key.default);
            });
        }
    }
    upgraded
}

/// Visits every object `path` leads to from `value`, where a `*` step enters each array item.
pub(super) fn for_each_object(
    value: &mut Value,
    path: &[&str],
    visit: &mut dyn FnMut(&mut Map<String, Value>),
) {
    match path.split_first() {
        None => {
            if let Some(object) = value.as_object_mut() {
                visit(object);
            }
        }
        Some((&"*", rest)) => {
            for item in value.as_array_mut().into_iter().flatten() {
                for_each_object(item, rest, visit);
            }
        }
        Some((step, rest)) => {
            if let Some(child) = value.get_mut(*step) {
                for_each_object(child, rest, visit);
            }
        }
    }
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

    /// The value at `path`, reading a `*` step at the array's first item.
    fn value_at<'a>(document: &'a Value, path: &[&str]) -> Option<&'a Value> {
        path.iter()
            .try_fold(&document["runtimeConfig"], |value, key| match *key {
                "*" => value.get(0),
                key => value.get(key),
            })
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
    fn a_patch_with_a_distortion_clip_above_full_scale_is_refused() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut patch = documents.patch.clone();
        let distortion = |clip: f64| json!({ "type": "distortion", "params": { "drive": 2.5, "clip": clip, "mixPct": 100 } });
        patch["runtimeConfig"]["mixer"]["master"]["slots"][1] = distortion(1.0);
        assert!(compose_local_system_patch_documents(&documents.system, &patch).is_ok());
        patch["runtimeConfig"]["mixer"]["master"]["slots"][1] = distortion(2.0);
        assert!(compose_local_system_patch_documents(&documents.system, &patch).is_err());
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
