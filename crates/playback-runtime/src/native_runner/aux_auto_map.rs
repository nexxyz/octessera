use crate::native_menu::NativeMenuValue;
use std::time::Duration;

use super::*;

const AUX_OVERLAY_DELAY_MS: u64 = 1_500;

#[derive(Clone)]
pub(super) struct ResolvedAuxTurn {
    pub(super) key: String,
    pub(super) label: String,
}

#[derive(Clone)]
pub(super) struct ResolvedAuxPress {
    pub(super) action: NativeMenuAction,
    pub(super) label: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AuxBindingSource {
    Auto,
    Custom,
    None,
}

#[derive(Clone)]
pub(super) struct ResolvedAuxSlot {
    pub(super) turn: Option<ResolvedAuxTurn>,
    pub(super) press: Option<ResolvedAuxPress>,
    pub(super) turn_source: AuxBindingSource,
    pub(super) press_source: AuxBindingSource,
}

impl NativeRunner {
    pub(super) fn aux_mapping_overlay(&self) -> Option<(String, Vec<String>)> {
        if !self.ui.fn_held || self.ui.shift_held {
            return None;
        }
        if self.fn_hold_started_at.is_none_or(|started| {
            Instant::now().duration_since(started) < Duration::from_millis(AUX_OVERLAY_DELAY_MS)
        }) {
            return None;
        }
        let slots = (0..platform_core::AUX_ENCODER_COUNT)
            .map(|index| self.effective_aux_slot(index))
            .collect::<Vec<_>>();
        if slots
            .iter()
            .all(|slot| slot.turn.is_none() && slot.press.is_none())
        {
            return None;
        }
        let has_auto = slots.iter().any(|slot| {
            slot.turn_source == AuxBindingSource::Auto
                || slot.press_source == AuxBindingSource::Auto
        });
        let has_custom = slots.iter().any(|slot| {
            slot.turn_source == AuxBindingSource::Custom
                || slot.press_source == AuxBindingSource::Custom
        });
        let title = if has_auto {
            "AUTO MAP"
        } else if has_custom {
            "CUSTOM MAP"
        } else {
            "AUX MAP"
        };
        let mut lines = vec![self.aux_overlay_context_label()];
        lines.extend(slots.iter().enumerate().map(|(index, slot)| {
            let mut parts = Vec::new();
            if let Some(turn) = &slot.turn {
                parts.push(turn.label.clone());
            }
            if let Some(press) = &slot.press {
                parts.push(format!("!{}", press.label));
            }
            let body = if parts.is_empty() {
                "-".into()
            } else {
                parts.join("/")
            };
            format!("A{} {body}", index + 1)
        }));
        Some((title.into(), lines))
    }

    pub(super) fn effective_aux_slot(&self, index: usize) -> ResolvedAuxSlot {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        let auto =
            self.resolve_aux_auto_map(&path, selected_key.as_deref(), selected_action.as_ref());
        let custom = self.resolve_custom_aux_slot(index);
        let auto_slot = auto.get(index).and_then(|slot| slot.as_ref());
        let custom_slot = custom.as_ref();
        let turn = custom_slot
            .and_then(|slot| slot.turn.clone())
            .or_else(|| auto_slot.and_then(|slot| slot.turn.clone()));
        let press = custom_slot
            .and_then(|slot| slot.press.clone())
            .or_else(|| auto_slot.and_then(|slot| slot.press.clone()));
        let turn_source = if custom_slot.and_then(|slot| slot.turn.as_ref()).is_some() {
            AuxBindingSource::Custom
        } else if auto_slot.and_then(|slot| slot.turn.as_ref()).is_some() {
            AuxBindingSource::Auto
        } else {
            AuxBindingSource::None
        };
        let press_source = if custom_slot.and_then(|slot| slot.press.as_ref()).is_some() {
            AuxBindingSource::Custom
        } else if auto_slot.and_then(|slot| slot.press.as_ref()).is_some() {
            AuxBindingSource::Auto
        } else {
            AuxBindingSource::None
        };
        ResolvedAuxSlot {
            turn,
            press,
            turn_source,
            press_source,
        }
    }

    pub(super) fn auto_map_prefix_for_line(
        &self,
        key: Option<&str>,
        action: Option<&NativeMenuAction>,
    ) -> Option<String> {
        let path = self.menu.current_focus_path();
        let auto = self.resolve_aux_auto_map(&path, key, action);
        for (index, slot) in auto.iter().enumerate() {
            let Some(slot) = slot else {
                continue;
            };
            if let Some(key) = key {
                if slot.turn.as_ref().map(|turn| turn.key.as_str()) == Some(key) {
                    return Some(format!("{}-", index + 1));
                }
            }
            if let Some(action) = action {
                if slot
                    .press
                    .as_ref()
                    .map(|press| self.aux_actions_match(&press.action, action))
                    .unwrap_or(false)
                {
                    return Some(format!("{}!", index + 1));
                }
            }
        }
        None
    }

    fn resolve_custom_aux_slot(&self, index: usize) -> Option<ResolvedAuxSlot> {
        let binding = self.aux_bindings.get(index)?.as_ref()?;
        let turn = binding.turn_key.as_ref().map(|key| ResolvedAuxTurn {
            key: key.clone(),
            label: self.aux_binding_key_label(key),
        });
        let press = binding
            .press_action
            .as_ref()
            .map(|action| ResolvedAuxPress {
                action: action.clone(),
                label: self.aux_binding_action_label(action),
            });
        Some(ResolvedAuxSlot {
            turn_source: if turn.is_some() {
                AuxBindingSource::Custom
            } else {
                AuxBindingSource::None
            },
            press_source: if press.is_some() {
                AuxBindingSource::Custom
            } else {
                AuxBindingSource::None
            },
            turn,
            press,
        })
    }

    fn resolve_aux_auto_map(
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
        if field.starts_with("synth.filter.") {
            return Some([
                Some(self.turn_slot(format!("{prefix}.synth.filter.cutoffHz"), "Cutoff")),
                Some(self.turn_slot(format!("{prefix}.synth.filter.resonance"), "Res")),
                Some(self.turn_slot(format!("{prefix}.synth.filter.envAmountPct"), "Env")),
                Some(self.turn_slot(format!("{prefix}.synth.filter.keyTrackingPct"), "Key")),
            ]);
        }
        if field.starts_with("sample.filter.") {
            return Some([
                Some(self.turn_slot(format!("{prefix}.sample.filter.cutoffHz"), "Cutoff")),
                Some(self.turn_slot(format!("{prefix}.sample.filter.resonance"), "Res")),
                Some(self.turn_slot(format!("{prefix}.sample.filter.envAmountPct"), "Env")),
                Some(self.turn_slot(format!("{prefix}.sample.filter.keyTrackingPct"), "Key")),
            ]);
        }
        if field.starts_with("synth.ampEnv.") {
            return Some(self.env_auto_map(&format!("{prefix}.synth.ampEnv")));
        }
        if field.starts_with("synth.filterEnv.") {
            return Some(self.env_auto_map(&format!("{prefix}.synth.filterEnv")));
        }
        if field.starts_with("sample.ampEnv.") {
            return Some(self.env_auto_map(&format!("{prefix}.sample.ampEnv")));
        }
        if field.starts_with("sample.filterEnv.") {
            return Some(self.env_auto_map(&format!("{prefix}.sample.filterEnv")));
        }
        if field.starts_with("synth.osc1.") {
            return Some(self.osc_auto_map(&format!("{prefix}.synth.osc1")));
        }
        if field.starts_with("synth.osc2.") {
            return Some(self.osc_auto_map(&format!("{prefix}.synth.osc2")));
        }
        if field.starts_with("synth.amp.") {
            return Some([
                Some(self.turn_slot(format!("{prefix}.synth.amp.gainPct"), "Gain")),
                Some(self.turn_slot(format!("{prefix}.synth.amp.velocitySensitivityPct"), "Vel")),
                None,
                None,
            ]);
        }
        if field.starts_with("sample.amp.") {
            return Some([
                Some(self.turn_slot(format!("{prefix}.sample.amp.gainPct"), "Gain")),
                Some(self.turn_slot(format!("{prefix}.sample.amp.velocitySensitivityPct"), "Vel")),
                None,
                None,
            ]);
        }
        if field.starts_with("sample.") {
            let sample_slot = self
                .instruments
                .get(index)
                .map(|instrument| instrument.selected_sample_slot.min(SAMPLE_SLOT_COUNT - 1))
                .unwrap_or(0);
            return Some([
                Some(self.turn_press_slot(
                    format!("{prefix}.sample.selectedSlot"),
                    "Slot",
                    NativeMenuAction::PlatformEffect(format!(
                        "sample.assign:{index}:{sample_slot}"
                    )),
                    "Assign",
                )),
                Some(self.turn_slot(format!("{prefix}.sample.baseVelocity"), "Base")),
                Some(self.turn_slot(format!("{prefix}.sample.tuneSemis"), "Tune")),
                Some(self.turn_slot(format!("{prefix}.sample.velocityLevelsEnabled"), "Levels")),
            ]);
        }
        if field.starts_with("mixer.") {
            return Some([
                Some(self.turn_slot(format!("{prefix}.mixer.volume"), "Vol")),
                Some(self.turn_slot(format!("{prefix}.mixer.panPos"), "Pan")),
                Some(self.turn_slot(format!("{prefix}.mixer.route"), "Route")),
                None,
            ]);
        }
        None
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
        let parts = key.split('.').collect::<Vec<_>>();
        if parts.first() != Some(&"mixer") {
            return None;
        }
        let (slot_type, base) = if parts.get(1) == Some(&"buses") && parts.len() >= 6 {
            let bus_index = parts.get(2)?.parse::<usize>().ok()?;
            let slot_name = *parts.get(3)?;
            let slot_type = match slot_name {
                "slot1" => self.fx_buses.get(bus_index)?.slot1_type.as_str(),
                "slot2" => self.fx_buses.get(bus_index)?.slot2_type.as_str(),
                _ => return None,
            };
            (
                slot_type,
                format!("mixer.buses.{bus_index}.{slot_name}.params"),
            )
        } else if parts.get(1) == Some(&"master")
            && parts.get(2) == Some(&"slots")
            && parts.len() >= 5
        {
            let slot_index = parts.get(3)?.parse::<usize>().ok()?;
            let slot_type = self.global_fx_slots.get(slot_index)?.as_str();
            (slot_type, format!("mixer.master.slots.{slot_index}.params"))
        } else {
            return None;
        };
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

    fn env_auto_map(&self, prefix: &str) -> [Option<ResolvedAuxSlot>; 4] {
        [
            Some(self.turn_slot(format!("{prefix}.attackMs"), "Atk")),
            Some(self.turn_slot(format!("{prefix}.decayMs"), "Dec")),
            Some(self.turn_slot(format!("{prefix}.sustainPct"), "Sus")),
            Some(self.turn_slot(format!("{prefix}.releaseMs"), "Rel")),
        ]
    }

    fn osc_auto_map(&self, prefix: &str) -> [Option<ResolvedAuxSlot>; 4] {
        [
            Some(self.turn_slot(format!("{prefix}.waveform"), "Wave")),
            Some(self.turn_slot(format!("{prefix}.levelPct"), "Level")),
            Some(self.turn_slot(format!("{prefix}.detuneCents"), "Detune")),
            Some(self.turn_slot(format!("{prefix}.pulseWidthPct"), "PW")),
        ]
    }

    fn turn_slot(&self, key: String, label: &str) -> ResolvedAuxSlot {
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

    fn turn_press_slot(
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

    pub(super) fn aux_binding_key_label(&self, key: &str) -> String {
        self.menu
            .binding_spec_for_key(key)
            .and_then(|binding| binding.label)
            .unwrap_or_else(|| match key {
                "masterVolume" => "Master Vol".into(),
                _ => key.rsplit('.').next().unwrap_or(key).into(),
            })
    }

    pub(super) fn aux_binding_action_label(&self, action: &NativeMenuAction) -> String {
        match action {
            NativeMenuAction::BehaviorAction(action_type) => self
                .l1_menu_items()
                .into_iter()
                .find_map(|item| match item.value {
                    NativeMenuValue::Action(NativeMenuAction::BehaviorAction(ref current))
                        if current == action_type =>
                    {
                        Some(item.label)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| action_type.clone()),
            NativeMenuAction::PlatformEffect(action_type) if action_type == "dance.fx.map" => {
                "Map".into()
            }
            NativeMenuAction::PlatformEffect(action_type) if action_type == "midi.panic" => {
                "MIDI Panic".into()
            }
            NativeMenuAction::PlatformEffect(action_type)
                if action_type == "store.refresh" || action_type == "preset.refresh" =>
            {
                "Refresh".into()
            }
            NativeMenuAction::PlatformEffect(action_type)
                if action_type.starts_with("sample.assign:") =>
            {
                "Assign".into()
            }
            NativeMenuAction::ResetBehavior => "Reset".into(),
            _ => "Action".into(),
        }
    }

    pub(super) fn aux_actions_match(
        &self,
        left: &NativeMenuAction,
        right: &NativeMenuAction,
    ) -> bool {
        match (left, right) {
            (NativeMenuAction::BehaviorAction(lhs), NativeMenuAction::BehaviorAction(rhs)) => {
                lhs == rhs
            }
            (NativeMenuAction::PlatformEffect(lhs), NativeMenuAction::PlatformEffect(rhs)) => {
                lhs == rhs
            }
            (NativeMenuAction::ResetBehavior, NativeMenuAction::ResetBehavior) => true,
            _ => false,
        }
    }

    fn aux_overlay_context_label(&self) -> String {
        let (selected_key, selected_action) = self.menu.current_binding_target();
        let path = self.menu.current_focus_path();
        if let Some(key) = selected_key {
            if key.contains(".synth.filter.") {
                return "Synth Filter".into();
            }
            if key.contains(".sample.filter.") {
                return "Sample Filter".into();
            }
            if key.contains(".synth.ampEnv.") || key.contains(".sample.ampEnv.") {
                return "Amp Env".into();
            }
            if key.contains(".synth.filterEnv.") || key.contains(".sample.filterEnv.") {
                return "Filter Env".into();
            }
            if key.contains(".synth.osc1.") {
                return "Osc 1".into();
            }
            if key.contains(".synth.osc2.") {
                return "Osc 2".into();
            }
            if key.contains("mixer.buses.") {
                return "FX Bus".into();
            }
            if key.contains("mixer.master.slots.") {
                return "Global FX".into();
            }
            if key.starts_with("parts.") && key.contains(".behaviorConfig.") {
                return "Life".into();
            }
            if key.contains("dance.fx.params.") {
                return "Dance FX".into();
            }
        }
        if matches!(selected_action, Some(NativeMenuAction::PlatformEffect(action)) if action.starts_with("sample.assign:"))
        {
            return "Sample".into();
        }
        if path.contains("L1: Life") {
            return "Life".into();
        }
        if path.contains("L4: Dance") {
            return "Dance FX".into();
        }
        "Aux Map".into()
    }
}
