use super::{Value, CONFIG_KIND, CONFIG_SCHEMA_VERSION};
use serde_json::Map;

#[path = "config_schema_validation_canonical.rs"]
mod canonical;
#[path = "config_schema_validation_device_io.rs"]
mod device_io;
#[path = "config_schema_validation_instruments.rs"]
mod instruments;
#[path = "config_schema_validation_layers.rs"]
mod layers;
#[path = "config_schema_validation_mapping_bindings.rs"]
mod mapping_bindings;
#[path = "config_schema_validation_mixer_fx.rs"]
mod mixer_fx;
#[path = "config_schema_validation_modulation.rs"]
mod modulation;
#[path = "config_schema_validation_orchestration.rs"]
mod orchestration;
#[path = "config_schema_validation_scalar.rs"]
mod scalar;

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
