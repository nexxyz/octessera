use super::{Value, CONFIG_KIND, CONFIG_SCHEMA_VERSION};
use serde_json::Map;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ConfigDto {
    kind: String,
    schema_version: u64,
    revision: Option<u64>,
    runtime_config: Value,
    mapping_config: Option<Value>,
    system: Option<Value>,
    extensions: Map<String, Value>,
}

impl ConfigDto {
    pub(super) fn decode(payload: &Value) -> Result<Self, String> {
        let object = payload
            .as_object()
            .ok_or_else(|| "configuration payload must be an object".to_string())?;
        let kind = object
            .get("kind")
            .and_then(Value::as_str)
            .ok_or_else(|| "configuration envelope kind must be a string".to_string())?;
        if kind != CONFIG_KIND {
            return Err(format!("unsupported configuration envelope kind `{kind}`"));
        }
        let schema_version = object
            .get("schemaVersion")
            .and_then(Value::as_u64)
            .ok_or_else(|| "configuration schemaVersion must be an integer".to_string())?;
        if schema_version != CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "unsupported configuration schema version {schema_version}"
            ));
        }
        let revision = object
            .get("revision")
            .map(|value| {
                value
                    .as_u64()
                    .ok_or_else(|| "configuration revision must be an unsigned integer".to_string())
            })
            .transpose()?;
        let runtime_config = object
            .get("runtimeConfig")
            .cloned()
            .ok_or_else(|| "configuration payload is missing runtimeConfig".to_string())?;
        if !runtime_config.is_object() {
            return Err("runtimeConfig must be an object".into());
        }

        let mut extensions = Map::new();
        for (key, value) in object {
            if !matches!(
                key.as_str(),
                "kind"
                    | "schemaVersion"
                    | "revision"
                    | "runtimeConfig"
                    | "mappingConfig"
                    | "system"
            ) {
                extensions.insert(key.clone(), value.clone());
            }
        }

        Ok(Self {
            kind: kind.into(),
            schema_version,
            revision,
            runtime_config,
            mapping_config: object.get("mappingConfig").cloned(),
            system: object.get("system").cloned(),
            extensions,
        })
    }

    pub(super) fn application_view(&self, payload: &Value) -> Result<Self, String> {
        let object = payload
            .as_object()
            .ok_or_else(|| "configuration payload must be an object".to_string())?;
        let runtime_config = object
            .get("runtimeConfig")
            .cloned()
            .ok_or_else(|| "configuration payload is missing runtimeConfig".to_string())?;
        if !runtime_config.is_object() {
            return Err("runtimeConfig must be an object".into());
        }

        let mut extensions = Map::new();
        for (key, value) in object {
            if !matches!(
                key.as_str(),
                "kind"
                    | "schemaVersion"
                    | "revision"
                    | "runtimeConfig"
                    | "mappingConfig"
                    | "system"
            ) {
                extensions.insert(key.clone(), value.clone());
            }
        }

        Ok(Self {
            kind: self.kind.clone(),
            schema_version: self.schema_version,
            revision: self.revision,
            runtime_config,
            mapping_config: object.get("mappingConfig").cloned(),
            system: object.get("system").cloned(),
            extensions,
        })
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn kind(&self) -> &str {
        &self.kind
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn schema_version(&self) -> u64 {
        self.schema_version
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn revision(&self) -> Option<u64> {
        self.revision
    }

    pub(super) fn runtime_config(&self) -> &Value {
        &self.runtime_config
    }

    pub(super) fn mapping_config(&self) -> Option<&Value> {
        self.mapping_config.as_ref()
    }

    pub(super) fn system(&self) -> Option<&Value> {
        self.system.as_ref()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn extensions(&self) -> &Map<String, Value> {
        &self.extensions
    }
}
