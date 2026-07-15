use super::aux_auto_map::{AuxBindingSource, ResolvedAuxPress, ResolvedAuxSlot, ResolvedAuxTurn};
use super::*;

impl NativeRunner {
    pub(super) fn fx_slot_auto_map(&self, key: &str) -> Option<[Option<ResolvedAuxSlot>; 4]> {
        let (slot_type, base) = self.fx_slot_auto_map_context(key)?;
        let params = fx_default_params(slot_type);
        let has = |name: &str| params.get(name).is_some();
        let key_for = |name: &str| format!("{base}.{name}");
        Some(match slot_type {
            "reverb" => [
                has("decay").then(|| self.turn_slot(key_for("decay"), "Decay")),
                has("damp").then(|| self.turn_slot(key_for("damp"), "Damp")),
                None,
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "delay" => [
                has("timeMs").then(|| self.turn_slot(key_for("timeMs"), "Time")),
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
                None,
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "tremolo" | "auto_pan" => [
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthPct").then(|| self.turn_slot(key_for("depthPct"), "Depth")),
                None,
                None,
            ],
            "vibrato" => [
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthMs").then(|| self.turn_slot(key_for("depthMs"), "Depth")),
                has("baseMs").then(|| self.turn_slot(key_for("baseMs"), "Base")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "chorus" => [
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthMs").then(|| self.turn_slot(key_for("depthMs"), "Depth")),
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "flanger" => [
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("feedback").then(|| self.turn_slot(key_for("feedback"), "FB")),
                has("depthMs").then(|| self.turn_slot(key_for("depthMs"), "Depth")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "wah" | "filter_lfo" => [
                has("rateHz").then(|| self.turn_slot(key_for("rateHz"), "Rate")),
                has("depthPct").then(|| self.turn_slot(key_for("depthPct"), "Depth")),
                has("centerHz").then(|| self.turn_slot(key_for("centerHz"), "Center")),
                has("q").then(|| self.turn_slot(key_for("q"), "Q")),
            ],
            "duck" => [
                has("attackMs").then(|| self.turn_slot(key_for("attackMs"), "Atk")),
                has("amountPct").then(|| self.turn_slot(key_for("amountPct"), "Amt")),
                has("threshold").then(|| self.turn_slot(key_for("threshold"), "Th")),
                has("releaseMs").then(|| self.turn_slot(key_for("releaseMs"), "Rel")),
            ],
            "bitcrusher" => [
                has("rateDiv").then(|| self.turn_slot(key_for("rateDiv"), "Div")),
                None,
                has("bits").then(|| self.turn_slot(key_for("bits"), "Bits")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "saturator" => [
                None,
                None,
                has("drive").then(|| self.turn_slot(key_for("drive"), "Drive")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "distortion" => [
                None,
                has("clip").then(|| self.turn_slot(key_for("clip"), "Clip")),
                has("drive").then(|| self.turn_slot(key_for("drive"), "Drive")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "glitch" => [
                has("sliceMs").then(|| self.turn_slot(key_for("sliceMs"), "Slice")),
                has("chancePct").then(|| self.turn_slot(key_for("chancePct"), "Chance")),
                None,
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "compressor" => [
                has("attackMs").then(|| self.turn_slot(key_for("attackMs"), "Atk")),
                has("ratio").then(|| self.turn_slot(key_for("ratio"), "Ratio")),
                has("thresholdDb").then(|| self.turn_slot(key_for("thresholdDb"), "Thresh")),
                has("makeupDb").then(|| self.turn_slot(key_for("makeupDb"), "Makeup")),
            ],
            "eq" => [
                has("midFreqHz").then(|| self.turn_slot(key_for("midFreqHz"), "MidHz")),
                has("midQ").then(|| self.turn_slot(key_for("midQ"), "MidQ")),
                has("midGainDb").then(|| self.turn_slot(key_for("midGainDb"), "Mid")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "vinyl" => [
                has("saturationPct").then(|| self.turn_slot(key_for("saturationPct"), "Sat")),
                has("cracklePct").then(|| self.turn_slot(key_for("cracklePct"), "Crackle")),
                has("warpDepthPct").then(|| self.turn_slot(key_for("warpDepthPct"), "Warp")),
                has("mixPct").then(|| self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            _ => [None, None, None, None],
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

    pub(super) fn sparks_fx_auto_map(&self) -> [Option<ResolvedAuxSlot>; 4] {
        let fx_type = sparks_fx_type(&self.sparks_fx_selected);
        let key_for = |name: &str| format!("sparks.fx.params.{name}");
        let slots = match fx_type {
            "stutter" => [
                Some(self.turn_slot(key_for("rateHz"), "Rate")),
                Some(self.turn_slot(key_for("depthPct"), "Depth")),
                None,
                None,
            ],
            "freeze" => [
                Some(self.turn_slot(key_for("releaseMs"), "Release")),
                None,
                None,
                Some(self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            "filter_sweep" => [
                Some(self.turn_slot(key_for("sweepInMs"), "In")),
                Some(self.turn_slot(key_for("resonancePct"), "Res")),
                Some(self.turn_slot(key_for("cutoffPct"), "Cutoff")),
                Some(self.turn_slot(key_for("sweepOutMs"), "Out")),
            ],
            "pitch_shift" => [
                None,
                Some(self.turn_slot(key_for("cents"), "Cents")),
                Some(self.turn_slot(key_for("semitones"), "Semi")),
                Some(self.turn_slot(key_for("mixPct"), "Mix")),
            ],
            _ => [None, None, None, None],
        };
        [
            Some(ResolvedAuxSlot {
                turn: slots[0].as_ref().and_then(|slot| slot.turn.clone()),
                press: Some(ResolvedAuxPress {
                    action: NativeMenuAction::PlatformEffect("sparks.fx.map".into()),
                    label: "Map".into(),
                }),
                turn_source: slots[0]
                    .as_ref()
                    .map(|slot| slot.turn_source)
                    .unwrap_or(AuxBindingSource::None),
                press_source: AuxBindingSource::Auto,
            }),
            slots[1].clone(),
            slots[2].clone(),
            slots[3].clone(),
        ]
    }

    pub(super) fn env_auto_map(&self, prefix: &str) -> [Option<ResolvedAuxSlot>; 4] {
        [
            Some(self.turn_slot(format!("{prefix}.attackMs"), "Atk")),
            Some(self.turn_slot(format!("{prefix}.decayMs"), "Dec")),
            Some(self.turn_slot(format!("{prefix}.sustainPct"), "Sus")),
            Some(self.turn_slot(format!("{prefix}.releaseMs"), "Rel")),
        ]
    }

    pub(super) fn osc_auto_map(&self, prefix: &str) -> [Option<ResolvedAuxSlot>; 4] {
        [
            Some(self.turn_slot(format!("{prefix}.waveform"), "Wave")),
            Some(self.turn_slot(format!("{prefix}.levelPct"), "Level")),
            Some(self.turn_slot(format!("{prefix}.detuneCents"), "Detune")),
            Some(self.turn_slot(format!("{prefix}.pulseWidthPct"), "PW")),
        ]
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
}
