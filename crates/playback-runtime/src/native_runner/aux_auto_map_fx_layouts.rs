use super::aux_auto_map::{
    aux_auto_map, AuxAutoMap, AuxBindingSource, ResolvedAuxPress, ResolvedAuxSlot, ResolvedAuxTurn,
};
use super::play_fx_config::play_fx_type;
use super::*;

impl NativeRunner {
    pub(super) fn fx_slot_auto_map(&self, key: &str) -> Option<AuxAutoMap> {
        let (slot_type, base) = self.fx_slot_auto_map_context(key)?;
        let params = fx_default_params(slot_type);
        let has = |name: &str| params.get(name).is_some();
        let key_for = |name: &str| format!("{base}.{name}");
        Some(match slot_type {
            "reverb" => aux_auto_map([
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
                has("decay").then(|| self.turn_slot(key_for("decay"), "Decay")),
                has("damp").then(|| self.turn_slot(key_for("damp"), "Damp")),
            ]),
            "delay" => aux_auto_map([
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
                has("timeMs").then(|| self.turn_slot(key_for("timeMs"), "Time")),
                has("spreadPct").then(|| self.turn_slot(key_for("spreadPct"), "Spread")),
            ]),
            "tremolo" | "auto_pan" => aux_auto_map([
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthPct").then(|| self.turn_slot(key_for("depthPct"), "Depth")),
            ]),
            "vibrato" => aux_auto_map([
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthMs").then(|| self.turn_slot(key_for("depthMs"), "Depth")),
                has("baseMs").then(|| self.turn_slot(key_for("baseMs"), "Base")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
            ]),
            "chorus" => aux_auto_map([
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
                has("depthMs").then(|| self.turn_slot(key_for("depthMs"), "Depth")),
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
                has("baseMs").then(|| self.turn_slot(key_for("baseMs"), "Base")),
            ]),
            "flanger" => aux_auto_map([
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthMs").then(|| self.turn_slot(key_for("depthMs"), "Depth")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
                has("baseMs").then(|| self.turn_slot(key_for("baseMs"), "Base")),
            ]),
            "wah" | "filter_lfo" => aux_auto_map([
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthPct").then(|| self.turn_slot(key_for("depthPct"), "Depth")),
                has("centerHz").then(|| self.turn_slot(key_for("centerHz"), "Center")),
                has("q").then(|| self.turn_slot(key_for("q"), "Q")),
            ]),
            "duck" => aux_auto_map([
                has("amountPct").then(|| self.turn_slot(key_for("amountPct"), "Amt")),
                has("releaseMs").then(|| self.turn_slot(key_for("releaseMs"), "Rel")),
                has("threshold").then(|| self.turn_slot(key_for("threshold"), "Th")),
                has("attackMs").then(|| self.turn_slot(key_for("attackMs"), "Atk")),
            ]),
            "bitcrusher" => aux_auto_map([
                has("bits").then(|| self.turn_slot(key_for("bits"), "Bits")),
                has("rateDiv").then(|| self.turn_slot(key_for("rateDiv"), "Div")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            "saturator" => aux_auto_map([
                has("drive").then(|| self.turn_slot(key_for("drive"), "Drive")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            "distortion" => aux_auto_map([
                has("drive").then(|| self.turn_slot(key_for("drive"), "Drive")),
                has("clip").then(|| self.turn_slot(key_for("clip"), "Clip")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            "glitch" => aux_auto_map([
                has("chancePct").then(|| self.turn_slot(key_for("chancePct"), "Chance")),
                has("sliceMs").then(|| self.turn_slot(key_for("sliceMs"), "Slice")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            "compressor" => aux_auto_map([
                has("thresholdDb").then(|| self.turn_slot(key_for("thresholdDb"), "Thresh")),
                has("ratio").then(|| self.turn_slot(key_for("ratio"), "Ratio")),
                has("makeupDb").then(|| self.turn_slot(key_for("makeupDb"), "Makeup")),
                has("attackMs").then(|| self.turn_slot(key_for("attackMs"), "Atk")),
                has("releaseMs").then(|| self.turn_slot(key_for("releaseMs"), "Rel")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            "eq" => aux_auto_map([
                has("lowGainDb").then(|| self.turn_slot(key_for("lowGainDb"), "Low")),
                has("midGainDb").then(|| self.turn_slot(key_for("midGainDb"), "Mid")),
                has("highGainDb").then(|| self.turn_slot(key_for("highGainDb"), "High")),
                has("midFreqHz").then(|| self.turn_slot(key_for("midFreqHz"), "MidHz")),
                has("midQ").then(|| self.turn_slot(key_for("midQ"), "MidQ")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            "vinyl" => aux_auto_map([
                has("cracklePct").then(|| self.turn_slot(key_for("cracklePct"), "Crackle")),
                has("warpDepthPct").then(|| self.turn_slot(key_for("warpDepthPct"), "Warp")),
                has("saturationPct").then(|| self.turn_slot(key_for("saturationPct"), "Sat")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ]),
            _ => AuxAutoMap::default(),
        })
    }

    fn fx_slot_auto_map_context(&self, key: &str) -> Option<(&str, String)> {
        let layers = key.split('.').collect::<Vec<_>>();
        if layers.first() != Some(&"mixer") {
            return None;
        }
        if layers.get(1) == Some(&"buses") && layers.len() >= 6 {
            return self.fx_bus_slot_auto_map_context(&layers);
        }
        if layers.get(1) == Some(&"master") && layers.get(2) == Some(&"slots") && layers.len() >= 5
        {
            return self.global_fx_slot_auto_map_context(&layers);
        }
        None
    }

    fn fx_bus_slot_auto_map_context(&self, layers: &[&str]) -> Option<(&str, String)> {
        let bus_index = layers.get(2)?.parse::<usize>().ok()?;
        let slot_name = *layers.get(3)?;
        let slot_type = match slot_name {
            "slot1" => self.fx_buses.get(bus_index)?.slot1_type.as_str(),
            "slot2" => self.fx_buses.get(bus_index)?.slot2_type.as_str(),
            "slot3" => self.fx_buses.get(bus_index)?.slot3_type.as_str(),
            _ => return None,
        };
        Some((
            slot_type,
            format!("mixer.buses.{bus_index}.{slot_name}.params"),
        ))
    }

    fn global_fx_slot_auto_map_context(&self, layers: &[&str]) -> Option<(&str, String)> {
        let slot_index = layers.get(3)?.parse::<usize>().ok()?;
        let slot_type = self.global_fx_slots.get(slot_index)?.as_str();
        Some((slot_type, format!("mixer.master.slots.{slot_index}.params")))
    }

    /// Play FX: Aux 1 click always maps the selected effect onto the grid.
    pub(super) fn play_fx_auto_map(&self) -> AuxAutoMap {
        let fx_type = play_fx_type(&self.play_fx_selected);
        let turn =
            |name: &str, label: &str| Some(self.turn_slot(format!("play.fx.params.{name}"), label));
        let mut slots = match fx_type {
            "stutter" => aux_auto_map([turn("rateHz", "Rate"), turn("depthPct", "Depth")]),
            "freeze" => aux_auto_map([turn("releaseMs", "Release"), turn("mixPct", "Mix")]),
            "filter_sweep" => aux_auto_map([
                turn("cutoffPct", "Cutoff"),
                turn("resonancePct", "Res"),
                turn("sweepInMs", "In"),
                turn("sweepOutMs", "Out"),
            ]),
            "pitch_shift" => aux_auto_map([
                turn("semitones", "Semi"),
                turn("cents", "Cents"),
                turn("mixPct", "Mix"),
                turn("slideInMs", "In"),
                turn("slideOutMs", "Out"),
            ]),
            _ => AuxAutoMap::default(),
        };
        let first = slots[0].take();
        slots[0] = Some(ResolvedAuxSlot {
            turn_source: if first.as_ref().is_some_and(|slot| slot.turn.is_some()) {
                AuxBindingSource::Auto
            } else {
                AuxBindingSource::None
            },
            turn: first.and_then(|slot| slot.turn),
            press: Some(ResolvedAuxPress {
                action: NativeMenuAction::PlatformEffect("play.fx.map".into()),
                label: "Map".into(),
            }),
            press_source: AuxBindingSource::Auto,
        });
        slots
    }

    pub(super) fn env_auto_map(&self, prefix: &str) -> AuxAutoMap {
        aux_auto_map([
            Some(self.turn_slot(format!("{prefix}.attackMs"), "Atk")),
            Some(self.turn_slot(format!("{prefix}.decayMs"), "Dec")),
            Some(self.turn_slot(format!("{prefix}.releaseMs"), "Rel")),
            Some(self.turn_slot(format!("{prefix}.sustainPct"), "Sus")),
        ])
    }

    pub(super) fn osc_auto_map(&self, prefix: &str) -> AuxAutoMap {
        aux_auto_map([
            Some(self.turn_slot(format!("{prefix}.waveform"), "Wave")),
            Some(self.turn_slot(format!("{prefix}.levelPct"), "Level")),
            Some(self.turn_slot(format!("{prefix}.detuneCents"), "Detune")),
            Some(self.turn_slot(format!("{prefix}.pulseWidthPct"), "PW")),
            Some(self.turn_slot(format!("{prefix}.octave"), "Octave")),
        ])
    }

    pub(super) fn turn_slot(&self, key: String, label: &str) -> ResolvedAuxSlot {
        ResolvedAuxSlot {
            turn: Some(ResolvedAuxTurn {
                key,
                label: label.into(),
            }),
            press: None,
            turn_source: AuxBindingSource::Auto,
            press_source: AuxBindingSource::None,
        }
    }

    pub(super) fn turn_press_slot(
        &self,
        key: String,
        turn_label: &str,
        action: NativeMenuAction,
        press_label: &str,
    ) -> ResolvedAuxSlot {
        ResolvedAuxSlot {
            turn: Some(ResolvedAuxTurn {
                key,
                label: turn_label.into(),
            }),
            press: Some(ResolvedAuxPress {
                action,
                label: press_label.into(),
            }),
            turn_source: AuxBindingSource::Auto,
            press_source: AuxBindingSource::Auto,
        }
    }

    pub(super) fn press_slot(&self, action: NativeMenuAction, label: &str) -> ResolvedAuxSlot {
        ResolvedAuxSlot {
            turn: None,
            press: Some(ResolvedAuxPress {
                action,
                label: label.into(),
            }),
            turn_source: AuxBindingSource::None,
            press_source: AuxBindingSource::Auto,
        }
    }
}
