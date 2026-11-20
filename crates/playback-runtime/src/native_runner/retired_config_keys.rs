use super::added_config_keys::is_kind;
use super::config_documents::SYSTEM_DOCUMENT_KIND;
use serde_json::Value;

const SYSTEM_DOCUMENT: &[&str] = &[SYSTEM_DOCUMENT_KIND];

type RetiredKey = (&'static [&'static str], &'static str, fn(&Value) -> bool);

const RETIRED_RUNTIME_KEYS: [RetiredKey; 2] = [
    (SYSTEM_DOCUMENT, "auxBindings", holds_no_binding),
    (SYSTEM_DOCUMENT, "shiftAuxBindings", holds_no_binding),
];

/// Loading drops each runtime key in `RETIRED_RUNTIME_KEYS` from documents of the listed kinds
/// when its saved value passes the entry's check: a System saved by 0.8.14 still carries empty
/// Aux banks that only the patch owns now, while a System with real bindings stays rejected. Use
/// an entry only for a value that can go; anything a save still needs means normalizing the
/// existing stores.
pub(crate) fn without_retired_keys(document: &Value) -> Value {
    without_runtime_keys(document, &RETIRED_RUNTIME_KEYS)
}

fn holds_no_binding(bank: &Value) -> bool {
    bank.as_object()
        .is_some_and(|bank| bank.values().all(Value::is_null))
}

fn without_runtime_keys(document: &Value, keys: &[RetiredKey]) -> Value {
    let mut cleaned = document.clone();
    if let Some(runtime) = cleaned
        .get_mut("runtimeConfig")
        .and_then(Value::as_object_mut)
    {
        for (_, key, retire) in keys.iter().filter(|(kinds, ..)| is_kind(document, kinds)) {
            if runtime.get(*key).is_some_and(retire) {
                runtime.remove(*key);
            }
        }
    }
    cleaned
}

#[cfg(test)]
mod tests {
    use super::super::added_config_keys::SYSTEM_DOCUMENTS;
    use super::super::{compose_local_system_patch_documents, split_system_patch_documents};
    use super::without_runtime_keys;
    use serde_json::{json, Value};

    const PI_DEFAULT: &str = include_str!("../../../../config/generated/pi/default.json");

    #[test]
    fn a_retired_runtime_key_is_dropped_before_the_strict_checks() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let mut old_system = documents.system.clone();
        old_system["runtimeConfig"]["retiredTestKey"] = json!(true);

        assert!(compose_local_system_patch_documents(&old_system, &documents.patch).is_err());
        let keys: [super::RetiredKey; 1] = [(SYSTEM_DOCUMENTS, "retiredTestKey", |_| true)];
        let cleaned = without_runtime_keys(&old_system, &keys);
        assert_eq!(cleaned, documents.system);
        assert!(compose_local_system_patch_documents(&cleaned, &documents.patch).is_ok());
        assert_eq!(
            without_runtime_keys(&documents.patch, &keys),
            documents.patch
        );
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
