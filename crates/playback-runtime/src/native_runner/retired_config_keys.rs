use serde_json::Value;

const RETIRED_RUNTIME_KEYS: [&str; 5] = [
    "noteLengthMs",
    "velocityScalePct",
    "velocityCurve",
    "voiceStealingMode",
    "bpm",
];

/// Saves written before the settings were folded still carry these duplicates; the kept copies
/// (`sound.*`, `transport.bpm`, `midi`) hold the same values, so loading simply drops them. The
/// old `sound.audioOutputBufferFrames` setting never reached an audio stream and goes the same way.
pub(crate) fn without_retired_keys(document: &Value) -> Value {
    let mut document = document.clone();
    let Some(runtime) = document
        .get_mut("runtimeConfig")
        .and_then(Value::as_object_mut)
    else {
        return document;
    };
    for key in RETIRED_RUNTIME_KEYS {
        runtime.remove(key);
    }
    if let Some(sound) = runtime.get_mut("sound").and_then(Value::as_object_mut) {
        sound.remove("audioOutputBufferFrames");
    }
    for layer in runtime
        .get_mut("layers")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(build) = layer.get_mut("build").and_then(Value::as_object_mut) {
            build.remove("saveGridState");
        }
    }
    for instrument in runtime
        .get_mut("instruments")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(instrument) = instrument.as_object_mut() {
            instrument.remove("midiEngine");
        }
    }
    document
}

#[cfg(test)]
mod tests {
    use super::super::{compose_local_system_patch_documents, split_system_patch_documents};
    use serde_json::{json, Value};

    const PI_DEFAULT: &str = include_str!("../../../../config/generated/pi/default.json");

    fn with_old_duplicates(mut document: Value) -> Value {
        let runtime = &mut document["runtimeConfig"];
        runtime["noteLengthMs"] = runtime["sound"]["noteLengthMs"].clone();
        runtime["velocityScalePct"] = runtime["sound"]["velocityScalePct"].clone();
        runtime["velocityCurve"] = runtime["sound"]["velocityCurve"].clone();
        runtime["voiceStealingMode"] = runtime["sound"]["voiceStealingMode"].clone();
        runtime["bpm"] = json!(120.0);
        for layer in runtime["layers"].as_array_mut().unwrap() {
            layer["build"]["saveGridState"] = json!(true);
        }
        for instrument in runtime["instruments"].as_array_mut().unwrap() {
            instrument["midiEngine"] = json!({ "channel": 1, "velocity": 100, "durationMs": 120 });
        }
        document
    }

    #[test]
    fn saves_from_before_the_settings_fold_still_load() {
        let full: Value = serde_json::from_str(PI_DEFAULT).unwrap();
        let documents = split_system_patch_documents(&full).unwrap();
        let old_patch = with_old_duplicates(documents.patch.clone());

        let mut old_system = documents.system.clone();
        old_system["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = json!(256);

        let composed = compose_local_system_patch_documents(&old_system, &old_patch).unwrap();
        let fresh =
            compose_local_system_patch_documents(&documents.system, &documents.patch).unwrap();
        assert_eq!(composed, fresh);

        let mut old_full = with_old_duplicates(full);
        old_full["runtimeConfig"]["sound"]["audioOutputBufferFrames"] = json!(256);
        assert_eq!(split_system_patch_documents(&old_full).unwrap(), documents);
    }
}
