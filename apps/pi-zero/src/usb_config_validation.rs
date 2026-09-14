use playback_runtime::AudioOutputSet;

pub(crate) fn validate_pi_audio_outputs_payload(payload: &serde_json::Value) -> Result<(), String> {
    let root = payload.get("runtimeConfig").unwrap_or(payload);
    let Some(root) = root.as_object() else {
        return Ok(());
    };
    let Some(audio_outputs) = root.get("audioOutputs") else {
        return Ok(());
    };
    if audio_outputs
        .get("dac")
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        return Err("Jack Audio is always on".into());
    }
    if !AudioOutputSet::decode(audio_outputs)
        .map_err(|error| error.to_string())?
        .dac()
    {
        return Err("Jack Audio is always on".into());
    }
    Ok(())
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn validate_raspberry_usb_payload(payload: &serde_json::Value) -> Result<(), String> {
    crate::usb_config::parse_usb_runtime_config(payload)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
