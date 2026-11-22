use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::super::{json, PlayConfigDto, Value};
use crate::timing_units::{note_unit_to_pulses, DEFAULT_NOTE_UNIT, NOTE_UNIT_OPTIONS};

/// One Play FX parameter: its saved key, menu label, default, range and encoder step.
pub(crate) struct PlayFxParam {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) default: i32,
    pub(crate) min: i32,
    pub(crate) max: i32,
    pub(crate) step: i32,
}

const fn param(
    key: &'static str,
    label: &'static str,
    default: i32,
    min: i32,
    max: i32,
    step: i32,
) -> PlayFxParam {
    PlayFxParam {
        key,
        label,
        default,
        min,
        max,
        step,
    }
}

/// Each Play FX type's parameters in menu row order. `Note` rows hold an index into
/// `NOTE_UNIT_OPTIONS`: 0 is 1/32T, 3 is 1/16, 5 is 1/8, 7 is 1/4 and 9 is 1/2.
const PLAY_FX_PARAMS: [(&str, &[PlayFxParam]); 11] = [
    (
        "stutter",
        &[
            param("rateHz", "Rate", 8, 1, 32, 1),
            param("depthPct", "Depth", 100, 0, 100, 1),
        ],
    ),
    (
        "freeze",
        &[
            param("releaseMs", "Release", 500, 10, 5000, 10),
            param("mixPct", "Mix", 100, 0, 100, 1),
        ],
    ),
    (
        "filter_sweep",
        &[
            param("cutoffPct", "Cutoff", 84, 0, 100, 1),
            param("resonancePct", "Res", 0, 0, 100, 1),
            param("sweepInMs", "Sweep In", 120, 10, 3000, 10),
            param("sweepOutMs", "Sweep Out", 180, 10, 3000, 10),
        ],
    ),
    (
        "pitch_shift",
        &[
            param("semitones", "Semitones", 0, -24, 24, 1),
            param("cents", "Cents", 0, -100, 100, 1),
            param("slideInMs", "Slide In", 120, 10, 3000, 10),
            param("slideOutMs", "Slide Out", 180, 10, 3000, 10),
            param("mixPct", "Mix", 100, 0, 100, 1),
        ],
    ),
    (
        "tape_stop",
        &[
            param("stopMs", "Stop", 700, 50, 2000, 10),
            param("startMs", "Start", 300, 10, 2000, 10),
        ],
    ),
    (
        "reverse",
        &[
            param("lengthMs", "Length", 250, 50, 1000, 10),
            param("mixPct", "Mix", 100, 0, 100, 1),
        ],
    ),
    (
        "trance_gate",
        &[
            param("rateNote", "Rate", 3, 0, 7, 1),
            param("depthPct", "Depth", 100, 0, 100, 1),
            param("smoothPct", "Smooth", 20, 0, 100, 1),
        ],
    ),
    (
        "dub_throw",
        &[
            param("delayNote", "Time", 7, 2, 9, 1),
            param("feedbackPct", "Feedback", 55, 0, 90, 1),
            param("mixPct", "Mix", 40, 0, 100, 1),
        ],
    ),
    (
        "wash",
        &[
            param("sizePct", "Size", 80, 0, 100, 1),
            param("mixPct", "Mix", 60, 0, 100, 1),
            param("releaseMs", "Release", 2500, 100, 8000, 10),
        ],
    ),
    (
        "hp_riser",
        &[
            param("cutoffPct", "Cutoff", 60, 0, 100, 1),
            param("resonancePct", "Res", 30, 0, 100, 1),
            param("sweepInMs", "Sweep In", 2000, 10, 8000, 10),
            param("sweepOutMs", "Sweep Out", 150, 10, 3000, 10),
        ],
    ),
    (
        "crush",
        &[
            param("bits", "Bits", 6, 1, 16, 1),
            param("rateDiv", "Downsample", 6, 1, 32, 1),
            param("mixPct", "Mix", 100, 0, 100, 1),
        ],
    ),
];

/// The note rows of the tempo-synced types and the audio keys that carry them as clock pulses.
const SYNCED_NOTE_PARAMS: [(&str, &str); 2] =
    [("rateNote", "ratePulses"), ("delayNote", "timePulses")];

/// The selectable Play FX types in menu order.
pub(crate) fn play_fx_types() -> impl Iterator<Item = &'static str> {
    std::iter::once("none").chain(PLAY_FX_PARAMS.iter().map(|(name, _)| *name))
}

/// The note value a Play FX `Note` row shows.
pub(crate) fn play_fx_note_unit(index: i64) -> &'static str {
    usize::try_from(index)
        .ok()
        .and_then(|index| NOTE_UNIT_OPTIONS.get(index))
        .copied()
        .unwrap_or(DEFAULT_NOTE_UNIT)
}

/// The parameters of a Play FX type; `none` and unknown types have none.
pub(crate) fn play_fx_param_specs(fx_type: &str) -> &'static [PlayFxParam] {
    PLAY_FX_PARAMS
        .iter()
        .find(|(name, _)| *name == fx_type)
        .map_or(&[], |(_, params)| params)
}

/// A Play FX setup: the selected one, a tile stamp, or the one being stamped. Saved as
/// `{ "fxType", "targetKey", "params" }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::native_runner) struct PlayFxConfig {
    pub(in crate::native_runner) fx_type: String,
    pub(in crate::native_runner) target_key: String,
    pub(in crate::native_runner) params: BTreeMap<String, i64>,
}

impl Default for PlayFxConfig {
    fn default() -> Self {
        Self {
            fx_type: "none".into(),
            target_key: "master".into(),
            params: BTreeMap::new(),
        }
    }
}

impl PlayFxConfig {
    /// A saved setup with an unknown type or target replaced and exactly its type's
    /// parameters, each inside its range.
    pub(in crate::native_runner) fn from_dto(config: &PlayConfigDto) -> Self {
        Self::sanitized(
            config.fx_type.as_deref().unwrap_or("none"),
            config.target_key.as_deref().unwrap_or("master"),
            |key| {
                config
                    .params
                    .as_ref()
                    .and_then(|params| params.get(key))
                    .and_then(Value::as_i64)
            },
        )
    }

    pub(in crate::native_runner) fn sanitized(
        fx_type: &str,
        target_key: &str,
        param: impl Fn(&str) -> Option<i64>,
    ) -> Self {
        let fx_type = PLAY_FX_PARAMS
            .iter()
            .map(|(name, _)| *name)
            .find(|name| *name == fx_type)
            .unwrap_or("none");
        let target_key = Some(target_key)
            .filter(|key| is_play_fx_target(key))
            .unwrap_or("master");
        let params = play_fx_param_specs(fx_type)
            .iter()
            .map(|spec| {
                let value = param(spec.key).unwrap_or_else(|| i64::from(spec.default));
                (
                    spec.key.to_string(),
                    value.clamp(i64::from(spec.min), i64::from(spec.max)),
                )
            })
            .collect();
        Self {
            fx_type: fx_type.into(),
            target_key: target_key.into(),
            params,
        }
    }

    fn audio_param_values(&self) -> BTreeMap<String, Value> {
        self.params
            .iter()
            .map(|(key, value)| (key.clone(), Value::from(*value)))
            .collect()
    }

    /// The audio-command parameters: `Note` rows become clock pulses, and a tempo-synced type
    /// carries the tempo it follows. `None` for a type that does not follow it.
    pub(in crate::native_runner) fn synced_audio_params(
        &self,
        bpm: f64,
    ) -> Option<BTreeMap<String, Value>> {
        let mut params = self.audio_param_values();
        let mut synced = false;
        for (note_key, pulses_key) in SYNCED_NOTE_PARAMS {
            if let Some(index) = self.params.get(note_key) {
                params.remove(note_key);
                let pulses = note_unit_to_pulses(play_fx_note_unit(*index));
                params.insert(pulses_key.into(), json!(pulses));
                synced = true;
            }
        }
        synced.then(|| {
            params.insert("tempoBpm".into(), json!(bpm));
            params
        })
    }

    pub(super) fn audio_params(&self, bpm: f64) -> BTreeMap<String, Value> {
        self.synced_audio_params(bpm)
            .unwrap_or_else(|| self.audio_param_values())
    }
}

pub(crate) fn play_fx_targets() -> Vec<String> {
    let mut targets = vec!["master".to_string()];
    targets.extend((1..=platform_core::BUS_COUNT).map(|index| format!("fx_bus_{index}")));
    targets
        .extend((1..=platform_core::INSTRUMENT_COUNT).map(|index| format!("instrument_{index}")));
    targets
}

pub(in crate::native_runner) fn is_play_fx_target(key: &str) -> bool {
    play_fx_targets().iter().any(|target| target == key)
}
