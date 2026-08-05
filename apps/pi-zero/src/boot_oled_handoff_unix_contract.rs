use super::super::*;

pub(crate) fn parse_status(bytes: &[u8]) -> Result<HandoffStatus, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("malformed OLED status.json: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "OLED status.json must be an object".to_string())?;
    let phase = object
        .get("phase")
        .and_then(|value| value.as_str())
        .and_then(HandoffPhase::parse)
        .ok_or_else(|| "OLED status.json has an invalid phase".to_string())?;
    let request_id = object
        .get("requestId")
        .map(|value| {
            value
                .as_str()
                .filter(|value| super::valid_request_id(value))
                .map(str::to_owned)
                .ok_or_else(|| "OLED status.json has an invalid requestId".to_string())
        })
        .transpose()?;
    let request_required = !matches!(phase, HandoffPhase::Animating);
    if !request_required && request_id.is_some() {
        return Err("OLED animating status must omit requestId".into());
    }
    if request_required && request_id.is_none() {
        return Err(format!(
            "OLED status phase {} requires requestId",
            phase.as_str()
        ));
    }
    let expected = if request_required {
        [
            "schema",
            "phase",
            "bootId",
            "pid",
            "cycleCount",
            "requestId",
        ]
        .as_slice()
    } else {
        ["schema", "phase", "bootId", "pid", "cycleCount"].as_slice()
    };
    exact_keys(object, expected, "status.json")?;
    if object.get("schema").and_then(|value| value.as_u64()) != Some(u64::from(HANDOFF_SCHEMA)) {
        return Err("OLED status.json has an invalid schema".into());
    }
    let boot_id = object
        .get("bootId")
        .and_then(|value| value.as_str())
        .filter(|value| super::valid_boot_id(value))
        .map(str::to_owned)
        .ok_or_else(|| "OLED status.json has an invalid bootId".to_string())?;
    let pid = object
        .get("pid")
        .and_then(|value| value.as_u64())
        .filter(|value| (1..=u64::from(u32::MAX)).contains(value))
        .map(|value| value as u32)
        .ok_or_else(|| "OLED status.json has an invalid pid".to_string())?;
    let cycle_count = object
        .get("cycleCount")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| "OLED status.json has an invalid cycleCount".to_string())?;
    Ok(HandoffStatus {
        phase,
        boot_id,
        pid,
        cycle_count,
        request_id,
    })
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn parse_status_for_test(value: &serde_json::Value) -> Result<HandoffStatus, String> {
    parse_status(&serde_json::to_vec(value).map_err(|error| error.to_string())?)
}

pub(crate) fn parse_stop(bytes: &[u8]) -> Result<StopRequest, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("malformed OLED stop.request: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| "OLED stop.request must be an object".to_string())?;
    exact_keys(
        object,
        &["schema", "bootId", "pid", "requestId"],
        "stop.request",
    )?;
    if object.get("schema").and_then(|value| value.as_u64()) != Some(u64::from(HANDOFF_SCHEMA)) {
        return Err("OLED stop.request has an invalid schema".into());
    }
    let boot_id = object
        .get("bootId")
        .and_then(|value| value.as_str())
        .filter(|value| super::valid_boot_id(value))
        .map(str::to_owned)
        .ok_or_else(|| "OLED stop.request has an invalid bootId".to_string())?;
    let pid = object
        .get("pid")
        .and_then(|value| value.as_u64())
        .filter(|value| (1..=u64::from(u32::MAX)).contains(value))
        .map(|value| value as u32)
        .ok_or_else(|| "OLED stop.request has an invalid pid".to_string())?;
    let request_id = object
        .get("requestId")
        .and_then(|value| value.as_str())
        .filter(|value| super::valid_request_id(value))
        .map(str::to_owned)
        .ok_or_else(|| "OLED stop.request has an invalid requestId".to_string())?;
    Ok(StopRequest {
        boot_id,
        pid,
        request_id,
    })
}

fn exact_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    expected: &[&str],
    name: &str,
) -> Result<(), String> {
    if object.len() != expected.len() || object.keys().any(|key| !expected.contains(&key.as_str()))
    {
        Err(format!("OLED {name} has unknown or missing keys"))
    } else {
        Ok(())
    }
}

pub(crate) fn valid_request_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

pub(crate) fn valid_boot_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()
            }
        })
}
