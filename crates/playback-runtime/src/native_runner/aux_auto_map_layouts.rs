use super::aux_auto_map::{AuxBindingSource, ResolvedAuxPress, ResolvedAuxSlot, ResolvedAuxTurn};
use super::aux_auto_map_instrument_layouts::{
    instrument_amp_auto_map, instrument_envelope_auto_map, instrument_filter_auto_map,
    instrument_mixer_auto_map, instrument_oscillator_auto_map, instrument_sample_auto_map,
};
use super::*;

impl NativeRunner {
    pub(super) fn resolve_aux_auto_map(
        &self,
        path: &str,
        selected_key: Option<&str>,
        selected_action: Option<&NativeMenuAction>,
    ) -> [Option<ResolvedAuxSlot>; 4] {
        if !self.aux_auto_map_enabled || path.contains("L2: Sense") {
            return [None, None, None, None];
        }

        if path.contains("L4: Dance")
            && (selected_key.is_some_and(|key| key.starts_with("dance.fx.params."))
                || selected_action
                    .map(|action| {
                        matches!(action, NativeMenuAction::PlatformEffect(effect) if effect == "dance.fx.map")
                    })
                    .unwrap_or(false))
        {
            return self.dance_fx_auto_map();
        }

        if let Some(key) = selected_key {
            if self.is_behavior_auto_map_key(key) {
                return self.behavior_auto_map();
            }
            if let Some(slot) = self.instrument_auto_map(key) {
                return slot;
            }
            if let Some(slot) = self.fx_slot_auto_map(key) {
                return slot;
            }
        }

        if matches!(selected_action, Some(NativeMenuAction::BehaviorAction(_))) {
            return self.behavior_auto_map();
        }

        if let Some(NativeMenuAction::PlatformEffect(action)) = selected_action {
            if let Some(slot) = self.sample_action_auto_map(action) {
                return slot;
            }
            if action == "dance.fx.map" {
                return self.dance_fx_auto_map();
            }
        }

        [None, None, None, None]
    }

    fn is_behavior_auto_map_key(&self, key: &str) -> bool {
        key == "algorithmStep"
            || key.starts_with(&format!(
                "parts.{}.l1.behaviorConfig.",
                self.active_part_index
            ))
    }

    fn behavior_auto_map(&self) -> [Option<ResolvedAuxSlot>; 4] {
        let part_prefix = format!("parts.{}.l1.behaviorConfig", self.active_part_index);
        match self.behavior.id() {
            "life" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.randomCellsPerTick"), "Count")),
                Some(self.turn_press_slot(
                    format!("{part_prefix}.randomTickInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnRandom".into()),
                    "Spawn",
                )),
                None,
            ]),
            "brain" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.randomSeedCells"), "Count")),
                Some(self.turn_press_slot(
                    format!("{part_prefix}.seedInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("seedRandom".into()),
                    "Seed",
                )),
                Some(self.turn_slot(format!("{part_prefix}.fireThreshold"), "Thresh")),
            ]),
            "ant" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.maxAnts"), "Count")),
                Some(self.turn_press_slot(
                    format!("{part_prefix}.autoSpawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnAnt".into()),
                    "Spawn",
                )),
                None,
            ]),
            "bounce" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.maxBalls"), "Count")),
                Some(self.turn_press_slot(
                    format!("{part_prefix}.spawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("addBall".into()),
                    "Add",
                )),
                None,
            ]),
            "pulse" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.lifespan"), "Life")),
                Some(self.turn_press_slot(
                    format!("{part_prefix}.autoPulseInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnPulse".into()),
                    "Spawn",
                )),
                Some(self.turn_slot(format!("{part_prefix}.pulseShape"), "Shape")),
            ]),
            "raindrops" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.splashRadius"), "Splash")),
                Some(self.turn_press_slot(
                    format!("{part_prefix}.autoDropInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("dropNow".into()),
                    "Drop",
                )),
                None,
            ]),
            "dla" => self.with_step_rate([
                Some(self.turn_press_slot(
                    format!("{part_prefix}.spawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("seedCluster".into()),
                    "Seed",
                )),
                None,
                None,
            ]),
            "glider" => self.with_step_rate([
                Some(self.turn_press_slot(
                    format!("{part_prefix}.spawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnGlider".into()),
                    "Spawn",
                )),
                None,
                None,
            ]),
            "keys" => self.with_step_rate([
                Some(self.turn_slot(format!("{part_prefix}.quantize"), "Quantize")),
                None,
                None,
            ]),
            _ => [
                Some(self.turn_slot("algorithmStep".into(), "Step")),
                None,
                None,
                None,
            ],
        }
    }

    fn with_step_rate(
        &self,
        trailing: [Option<ResolvedAuxSlot>; 3],
    ) -> [Option<ResolvedAuxSlot>; 4] {
        [
            Some(self.turn_slot("algorithmStep".into(), "Step")),
            trailing[0].clone(),
            trailing[1].clone(),
            trailing[2].clone(),
        ]
    }

    fn instrument_auto_map(&self, key: &str) -> Option<[Option<ResolvedAuxSlot>; 4]> {
        let (index, field) = parse_instrument_binding_key(key)?;
        let prefix = format!("instruments.{index}");
        instrument_filter_auto_map(self, field, &prefix)
            .or_else(|| instrument_envelope_auto_map(self, field, &prefix))
            .or_else(|| instrument_oscillator_auto_map(self, field, &prefix))
            .or_else(|| instrument_amp_auto_map(self, field, &prefix))
            .or_else(|| instrument_sample_auto_map(self, index, field, &prefix))
            .or_else(|| instrument_mixer_auto_map(self, field, &prefix))
    }

    fn sample_action_auto_map(&self, action: &str) -> Option<[Option<ResolvedAuxSlot>; 4]> {
        let rest = action.strip_prefix("sample.assign:")?;
        let (instrument_slot, sample_slot, _) = parse_sample_action(rest).ok()?;
        let prefix = format!("instruments.{instrument_slot}.sample");
        Some([
            Some(self.turn_press_slot(
                format!("{prefix}.selectedSlot"),
                "Slot",
                NativeMenuAction::PlatformEffect(format!(
                    "sample.assign:{instrument_slot}:{}",
                    sample_slot.min(SAMPLE_SLOT_COUNT - 1)
                )),
                "Assign",
            )),
            Some(self.turn_slot(format!("{prefix}.baseVelocity"), "Base")),
            Some(self.turn_slot(format!("{prefix}.tuneSemis"), "Tune")),
            Some(self.turn_slot(format!("{prefix}.velocityLevelsEnabled"), "Levels")),
        ])
    }

    fn fx_slot_auto_map(&self, key: &str) -> Option<[Option<ResolvedAuxSlot>; 4]> {
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
        let parts = key.split('.').collect::<Vec<_>>();
        if parts.first() != Some(&"mixer") {
            return None;
        }
        if parts.get(1) == Some(&"buses") && parts.len() >= 6 {
            return self.fx_bus_slot_auto_map_context(&parts);
        }
        if parts.get(1) == Some(&"master") && parts.get(2) == Some(&"slots") && parts.len() >= 5 {
            return self.global_fx_slot_auto_map_context(&parts);
        }
        None
    }

    fn fx_bus_slot_auto_map_context(&self, parts: &[&str]) -> Option<(&str, String)> {
        let bus_index = parts.get(2)?.parse::<usize>().ok()?;
        let slot_name = *parts.get(3)?;
        let slot_type = match slot_name {
            "slot1" => self.fx_buses.get(bus_index)?.slot1_type.as_str(),
            "slot2" => self.fx_buses.get(bus_index)?.slot2_type.as_str(),
            _ => return None,
        };
        Some((
            slot_type,
            format!("mixer.buses.{bus_index}.{slot_name}.params"),
        ))
    }

    fn global_fx_slot_auto_map_context(&self, parts: &[&str]) -> Option<(&str, String)> {
        let slot_index = parts.get(3)?.parse::<usize>().ok()?;
        let slot_type = self.global_fx_slots.get(slot_index)?.as_str();
        Some((slot_type, format!("mixer.master.slots.{slot_index}.params")))
    }

    fn dance_fx_auto_map(&self) -> [Option<ResolvedAuxSlot>; 4] {
        let fx_type = dance_fx_type(&self.dance_fx_selected);
        let key_for = |name: &str| format!("dance.fx.params.{name}");
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
                    action: NativeMenuAction::PlatformEffect("dance.fx.map".into()),
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
