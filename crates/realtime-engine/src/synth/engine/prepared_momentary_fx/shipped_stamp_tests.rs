use super::*;
use crate::synth::MomentaryFxTarget;

/// The runtime sends tempo-synced `Note` rows as clock pulses with the tempo they follow
/// (`play_fx_synced_audio_params` in playback-runtime).
const SYNCED_NOTE_KEYS: [(&str, &str); 2] =
    [("rateNote", "ratePulses"), ("delayNote", "timePulses")];

fn shipped_play_fx_stamps() -> Vec<(String, BTreeMap<String, Value>)> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../config/defaults/base.json"
    );
    let defaults: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let play_fx = &defaults["runtimeConfig"]["playFx"];
    let configs = play_fx["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|assignment| &assignment["config"])
        .chain(std::iter::once(&play_fx["selected"]));
    configs
        .filter_map(|config| {
            let fx_type = config["fxType"].as_str()?;
            (fx_type != "none").then(|| {
                let mut params: BTreeMap<String, Value> = config["params"]
                    .as_object()
                    .unwrap()
                    .clone()
                    .into_iter()
                    .collect();
                for (note_key, pulses_key) in SYNCED_NOTE_KEYS {
                    if params.remove(note_key).is_some() {
                        params.insert(pulses_key.into(), serde_json::json!(24));
                        params.insert("tempoBpm".into(), serde_json::json!(120.0));
                    }
                }
                (fx_type.to_string(), params)
            })
        })
        .collect()
}

#[test]
fn engine_accepts_every_shipped_play_fx_stamp() {
    let stamps = shipped_play_fx_stamps();
    let mut types: Vec<&str> = stamps.iter().map(|(fx_type, _)| fx_type.as_str()).collect();
    types.sort_unstable();
    types.dedup();
    assert_eq!(types.len(), 11, "shipped Play FX types: {types:?}");
    for (fx_type, params) in stamps {
        for (key, value) in &params {
            let single = BTreeMap::from([(key.clone(), value.clone())]);
            assert!(
                prepare_momentary_fx_update(1, fx_type.clone(), single, 48_000).is_some(),
                "{fx_type} does not accept {key}"
            );
        }
        assert!(
            prepare_momentary_fx_start(
                "stamp".into(),
                fx_type.clone(),
                params.clone(),
                MomentaryFxTarget::Global,
                48_000,
            )
            .is_some(),
            "{fx_type} start {params:?}"
        );
        assert!(
            prepare_momentary_fx_update(1, fx_type.clone(), params.clone(), 48_000).is_some(),
            "{fx_type} update {params:?}"
        );
    }
}
