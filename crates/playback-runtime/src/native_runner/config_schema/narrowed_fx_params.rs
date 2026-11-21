use super::Value;

/// Saved FX numbers whose range has since narrowed: `(type, param, new top)`. Distortion Clip
/// reached 2.0 once, and above 1.0 the clipped sound passed full scale. A save from before the
/// narrowing loads with the value at the new top instead of being refused.
const NARROWED_FX_PARAMS: [(&str, &str, f64); 1] = [("distortion", "clip", 1.0)];

pub(in crate::native_runner) fn clamp_narrowed_fx_params(payload: &mut Value) {
    let Some(mixer) = payload
        .get_mut("runtimeConfig")
        .and_then(|runtime| runtime.get_mut("mixer"))
    else {
        return;
    };
    if let Some(buses) = mixer.get_mut("buses").and_then(Value::as_array_mut) {
        for bus in buses {
            for key in ["slot1", "slot2", "slot3"] {
                if let Some(slot) = bus.get_mut(key) {
                    clamp_slot(slot);
                }
            }
        }
    }
    if let Some(slots) = mixer
        .get_mut("master")
        .and_then(|master| master.get_mut("slots"))
        .and_then(Value::as_array_mut)
    {
        slots.iter_mut().for_each(clamp_slot);
    }
}

fn clamp_slot(slot: &mut Value) {
    let Some(slot_type) = slot.get("type").and_then(Value::as_str).map(str::to_string) else {
        return;
    };
    let Some(params) = slot.get_mut("params").and_then(Value::as_object_mut) else {
        return;
    };
    for (_, key, top) in NARROWED_FX_PARAMS
        .iter()
        .filter(|(fx_type, ..)| *fx_type == slot_type)
    {
        if params
            .get(*key)
            .and_then(Value::as_f64)
            .is_some_and(|value| value > *top)
        {
            params.insert((*key).into(), Value::from(*top));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::clamp_narrowed_fx_params;
    use serde_json::json;

    #[test]
    fn an_old_distortion_clip_above_the_new_top_lands_on_it() {
        let distortion = |clip: f64| json!({ "type": "distortion", "params": { "clip": clip } });
        let mut payload = json!({ "runtimeConfig": { "mixer": {
            "buses": [{ "slot1": distortion(2.0), "slot2": distortion(0.4), "slot3": { "type": "none" } }],
            "master": { "slots": [distortion(1.5), { "type": "saturator", "params": { "clip": 3.0 } }] },
        } } });
        clamp_narrowed_fx_params(&mut payload);
        let mixer = &payload["runtimeConfig"]["mixer"];
        assert_eq!(mixer["buses"][0]["slot1"]["params"]["clip"], 1.0);
        assert_eq!(mixer["buses"][0]["slot2"]["params"]["clip"], 0.4);
        assert_eq!(mixer["master"]["slots"][0]["params"]["clip"], 1.0);
        assert_eq!(mixer["master"]["slots"][1]["params"]["clip"], 3.0);
    }
}
