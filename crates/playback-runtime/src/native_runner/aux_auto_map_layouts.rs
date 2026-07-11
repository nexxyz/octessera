use super::aux_auto_map::ResolvedAuxSlot;
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
        if !self.aux_auto_map_enabled || path.contains("2: Pulses") {
            return [None, None, None, None];
        }

        if path.contains("4: Sparks")
            && (selected_key.is_some_and(|key| key.starts_with("sparks.fx.params."))
                || selected_action
                    .map(|action| {
                        matches!(action, NativeMenuAction::PlatformEffect(effect) if effect == "sparks.fx.map")
                    })
                    .unwrap_or(false))
        {
            return self.sparks_fx_auto_map();
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
            if action == "sparks.fx.map" {
                return self.sparks_fx_auto_map();
            }
        }

        [None, None, None, None]
    }

    fn is_behavior_auto_map_key(&self, key: &str) -> bool {
        key == "algorithmStep"
            || key.starts_with(&format!(
                "layers.{}.worlds.behaviorConfig.",
                self.active_layer_index
            ))
    }

    fn behavior_auto_map(&self) -> [Option<ResolvedAuxSlot>; 4] {
        let layer_prefix = format!("layers.{}.worlds.behaviorConfig", self.active_layer_index);
        match self.behavior.id() {
            "life" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.randomCellsPerTick"), "Count")),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.randomTickInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnRandom".into()),
                    "Spawn",
                )),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.gliderSpawnInterval"),
                    "Glider",
                    NativeMenuAction::BehaviorAction("spawnGlider".into()),
                    "Glider",
                )),
            ]),
            "brain" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.randomSeedCells"), "Count")),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.seedInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("seedRandom".into()),
                    "Seed",
                )),
                Some(self.turn_slot(format!("{layer_prefix}.fireThreshold"), "Thresh")),
            ]),
            "ant" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.maxAnts"), "Count")),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.autoSpawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnAnt".into()),
                    "Spawn",
                )),
                None,
            ]),
            "bounce" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.maxBalls"), "Count")),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.spawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("addBall".into()),
                    "Add",
                )),
                None,
            ]),
            "pulse" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.lifespan"), "Life")),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.autoPulseInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("spawnPulse".into()),
                    "Spawn",
                )),
                Some(self.turn_slot(format!("{layer_prefix}.pulseShape"), "Shape")),
            ]),
            "raindrops" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.splashRadius"), "Splash")),
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.autoDropInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("dropNow".into()),
                    "Drop",
                )),
                None,
            ]),
            "dla" => self.with_step_rate([
                Some(self.turn_press_slot(
                    format!("{layer_prefix}.spawnInterval"),
                    "Interval",
                    NativeMenuAction::BehaviorAction("seedCluster".into()),
                    "Seed",
                )),
                None,
                None,
            ]),
            "keys" => self.with_step_rate([
                Some(self.turn_slot(format!("{layer_prefix}.quantize"), "Quantize")),
                None,
                None,
            ]),
            "none" => [None, None, None, None],
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
}
