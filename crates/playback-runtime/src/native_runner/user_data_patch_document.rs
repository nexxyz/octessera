use super::{
    portable_patch_payload_for_save, portable_patch_projection, prepare_config_payload,
    prepare_patch_payload, validate_config_payload, Value, CONFIG_KIND,
};

pub(crate) fn normalize_user_data_patch_payload(
    payload: Value,
    canonical_defaults: &Value,
) -> Result<Value, String> {
    let prepared = apply_user_data_patch_payload(payload, canonical_defaults)?;
    portable_patch_projection(&prepared)
}

pub(crate) fn apply_user_data_patch_payload(
    payload: Value,
    canonical_defaults: &Value,
) -> Result<Value, String> {
    let payload = if payload.get("kind").and_then(Value::as_str) == Some(CONFIG_KIND) {
        let prepared_full_config = prepare_config_payload(payload, canonical_defaults)?.payload;
        portable_patch_payload_for_save(&prepared_full_config)?
    } else {
        payload
    };
    Ok(prepare_patch_payload(payload, canonical_defaults)?.payload)
}

pub(crate) fn validate_user_data_config_payload(payload: &Value) -> Result<(), String> {
    validate_config_payload(payload)
}
