use super::{Value, CONFIG_KIND, CONFIG_SCHEMA_VERSION};
use serde_json::Map;

mod canonical;
mod device_io;
mod instruments;
mod layers;
mod mapping_bindings;
mod mixer_fx;
mod modulation;
mod orchestration;
mod scalar;

pub use mixer_fx::is_valid_fx_string_param;
pub(super) use modulation::validate_canonical_lfo_bank_shape;

pub(super) fn validate_config_payload(payload: &Value) -> Result<(), String> {
    let object = payload
        .as_object()
        .ok_or_else(|| "configuration payload must be an object".to_string())?;
    if object.get("kind").and_then(Value::as_str) != Some(CONFIG_KIND)
        || object.get("schemaVersion").and_then(Value::as_u64) != Some(CONFIG_SCHEMA_VERSION)
    {
        return Err("prepared configuration has an invalid envelope".into());
    }
    validate_payload(object)
}

fn validate_payload(root: &Map<String, Value>) -> Result<(), String> {
    let runtime = canonical::object_field(root, "runtimeConfig", "configuration")?
        .ok_or_else(|| "configuration runtimeConfig must be an object".to_string())?;
    scalar::walk_scalars(&Value::Object(root.clone()), "configuration")?;
    orchestration::validate_runtime(runtime)?;
    if let Some(mapping) = root.get("mappingConfig") {
        mapping_bindings::validate_mapping_config(mapping)?;
    }
    orchestration::validate_system(root)
}

pub(super) fn validate_audio_outputs(runtime: &Map<String, Value>) -> Result<(), String> {
    device_io::validate_audio_outputs(runtime)
}
