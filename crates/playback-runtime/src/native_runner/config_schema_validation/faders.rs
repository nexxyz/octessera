use super::canonical::{array_field, object_field, object_value, unsigned_field};
use super::mapping_bindings::validate_binding_field;
use super::Value;
use serde_json::Map;

const PATH: &str = "runtimeConfig.faders";

pub(super) fn validate_faders(runtime: &Map<String, Value>) -> Result<(), String> {
    let faders = object_field(runtime, "faders", "runtimeConfig")?
        .ok_or_else(|| format!("{PATH} must be present"))?;
    let count = super::super::play_pages::faders::FADER_COUNT;
    let sliders = array_field(faders, "sliders", PATH, count)?
        .filter(|sliders| sliders.len() == count)
        .ok_or_else(|| format!("{PATH}.sliders must contain exactly eight sliders"))?;
    let top = (platform_core::GRID_HEIGHT - 1) as u64;
    for (index, value) in sliders.iter().enumerate() {
        let path = format!("{PATH}.sliders[{index}]");
        let slider = object_value(value, &path)?;
        for (target, position) in [("target", "position"), ("shiftTarget", "shiftPosition")] {
            validate_binding_field(slider, target, &path)?;
            unsigned_field(slider, position, &path, 0, top)?;
        }
    }
    Ok(())
}

/// The slider targets in claim order, for the exclusive-target check.
pub(super) fn fader_slider_targets(
    runtime: &Map<String, Value>,
) -> impl Iterator<Item = (String, &Value)> {
    runtime
        .get("faders")
        .and_then(|faders| faders.get("sliders"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .flat_map(|(index, slider)| {
            ["target", "shiftTarget"]
                .into_iter()
                .filter_map(move |field| {
                    slider
                        .get(field)
                        .map(|binding| (format!("{PATH}.sliders[{index}].{field}"), binding))
                })
        })
}
