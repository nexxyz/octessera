use serde_json::Value;

const RETIRED_RUNTIME_KEYS: [&str; 0] = [];

/// Loading drops runtime keys listed in `RETIRED_RUNTIME_KEYS`. Until a beta there are no
/// concrete entries: saved-key changes normalize the existing stores instead.
pub(crate) fn without_retired_keys(document: &Value) -> Value {
    without_runtime_keys(document, &RETIRED_RUNTIME_KEYS)
}

fn without_runtime_keys(document: &Value, keys: &[&str]) -> Value {
    let mut document = document.clone();
    if let Some(runtime) = document
        .get_mut("runtimeConfig")
        .and_then(Value::as_object_mut)
    {
        for key in keys {
            runtime.remove(*key);
        }
    }
    document
}

#[cfg(test)]
mod tests {
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
        let cleaned = without_runtime_keys(&old_system, &["retiredTestKey"]);
        assert_eq!(cleaned, documents.system);
        assert!(compose_local_system_patch_documents(&cleaned, &documents.patch).is_ok());
    }
}
