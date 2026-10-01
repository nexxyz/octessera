use super::Value;

pub(super) fn validate_local_patch_sample_paths(payload: &Value) -> Result<(), String> {
    let Some(instruments) = payload
        .get("runtimeConfig")
        .and_then(|runtime| runtime.get("instruments"))
        .and_then(Value::as_array)
    else {
        return Ok(());
    };
    for (instrument_index, instrument) in instruments.iter().enumerate() {
        let Some(slots) = instrument
            .get("sample")
            .and_then(|sample| sample.get("slots"))
            .and_then(Value::as_array)
        else {
            continue;
        };
        for (slot_index, slot) in slots.iter().enumerate() {
            let Some(value) = slot.get("path") else {
                continue;
            };
            if value.is_null() {
                continue;
            }
            let path = format!(
                "$.runtimeConfig.instruments[{instrument_index}].sample.slots[{slot_index}].path"
            );
            let path_value = value
                .as_str()
                .ok_or_else(|| format!("{path} must be a safe relative WAV path"))?;
            if !is_safe_local_wav_path(path_value) {
                return Err(format!("{path} must be a safe relative WAV path"));
            }
        }
    }
    Ok(())
}

fn is_safe_local_wav_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains(':')
        && !path.chars().any(char::is_control)
        && !path.starts_with('/')
        && path.to_ascii_lowercase().ends_with(".wav")
        && !path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

#[cfg(test)]
#[path = "local_sample_path_tests.rs"]
mod tests;
