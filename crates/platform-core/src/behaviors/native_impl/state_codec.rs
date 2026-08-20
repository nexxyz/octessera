use serde::{Deserialize, Serialize};
use serde_json::Value;

pub fn serialize<T: Serialize>(state: &T) -> Result<Value, String> {
    serde_json::to_value(state).map_err(|error| error.to_string())
}

pub fn deserialize<T: for<'de> Deserialize<'de>>(data: Value) -> Result<T, String> {
    serde_json::from_value(data).map_err(|error| error.to_string())
}
