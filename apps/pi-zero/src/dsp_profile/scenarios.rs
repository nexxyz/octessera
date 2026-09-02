use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEvent;

#[path = "baseline_events.rs"]
mod baseline_events;
#[path = "baseline_scenarios.rs"]
mod baseline_scenarios;
#[path = "fx_cases.rs"]
mod fx_cases;
#[path = "scenario_events.rs"]
mod scenario_events;

use baseline_scenarios::scenarios as baseline_profile_scenarios;
use fx_cases::{bus_heavy_events, fx_limit_events};
use scenario_events::{
    baseline_events, fx_ramp_events, mixed_overload_events, mixed_ramp_events, momentary_events,
    sample_overload_events, sample_ramp_events, synth_overload_events, synth_ramp_events,
};

pub struct ScenarioSpec {
    pub name: String,
    pub events: Vec<EngineEvent>,
    expected: Option<ExpectedProfileState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpectedProfileState {
    pub active_synth_voices: usize,
    pub active_sample_voices: usize,
    pub active_preview_sample_voices: usize,
    pub active_momentary_fx: usize,
    pub active_bus_fx_slots: usize,
    pub active_global_fx_slots: usize,
    pub cumulative_voice_steals: u64,
    pub expected_voice_admission_drops_start: u64,
    pub expected_voice_admission_drops_end: u64,
}

impl ScenarioSpec {
    fn new(name: impl Into<String>, events: Vec<EngineEvent>) -> Self {
        Self {
            name: name.into(),
            events,
            expected: None,
        }
    }

    pub(super) fn with_expected(
        name: impl Into<String>,
        events: Vec<EngineEvent>,
        expected: ExpectedProfileState,
    ) -> Self {
        Self {
            name: name.into(),
            events,
            expected: Some(expected),
        }
    }

    pub fn validate_snapshot(
        &self,
        phase: &str,
        snapshot: &realtime_engine::synth::SynthProfileSnapshot,
    ) -> Result<(), String> {
        let Some(expected) = self.expected else {
            return Ok(());
        };
        let actual = [
            (
                "synth voices",
                expected.active_synth_voices,
                snapshot.active_synth_voices,
            ),
            (
                "sample voices",
                expected.active_sample_voices,
                snapshot.active_sample_voices,
            ),
            (
                "preview sample voices",
                expected.active_preview_sample_voices,
                snapshot.active_preview_sample_voices,
            ),
            (
                "momentary FX",
                expected.active_momentary_fx,
                snapshot.active_momentary_fx,
            ),
            (
                "bus FX slots",
                expected.active_bus_fx_slots,
                snapshot.active_bus_fx_slots,
            ),
            (
                "global FX slots",
                expected.active_global_fx_slots,
                snapshot.active_global_fx_slots,
            ),
        ];
        if let Some((label, wanted, observed)) = actual
            .into_iter()
            .find(|(_, wanted, observed)| wanted != observed)
        {
            return Err(format!(
                "scenario {} {phase} state invalid: {label} expected {wanted}, observed {observed}",
                self.name
            ));
        }
        if snapshot.cumulative_voice_steals != expected.cumulative_voice_steals {
            return Err(format!(
                "scenario {} {phase} state invalid: voice steals expected {}, observed {}",
                self.name, expected.cumulative_voice_steals, snapshot.cumulative_voice_steals
            ));
        }
        let expected_admission_drops = if phase == "measurement" {
            expected.expected_voice_admission_drops_end
        } else {
            expected.expected_voice_admission_drops_start
        };
        if snapshot.cumulative_voice_admission_drops != expected_admission_drops {
            return Err(format!(
                "scenario {} {phase} state invalid: voice admission drops expected {}, observed {}",
                self.name, expected_admission_drops, snapshot.cumulative_voice_admission_drops
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMode {
    Baseline,
    Full,
    Overload,
    Soak,
    FxLimits,
}

impl ProfileMode {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "baseline" => Some(Self::Baseline),
            "full" => Some(Self::Full),
            "overload" | "steal" | "stealing" => Some(Self::Overload),
            "soak" => Some(Self::Soak),
            "fx-limits" | "fx_limits" | "fxlimits" => Some(Self::FxLimits),
            _ => None,
        }
    }
}

pub fn profile_scenarios(sample_rate: u32, mode: ProfileMode) -> Vec<ScenarioSpec> {
    match mode {
        ProfileMode::Baseline => baseline_profile_scenarios(sample_rate),
        ProfileMode::Full => full_scenarios(sample_rate),
        ProfileMode::Overload => overload_scenarios(sample_rate),
        ProfileMode::Soak => soak_scenarios(sample_rate),
        ProfileMode::FxLimits => fx_limit_scenarios(sample_rate),
    }
}

fn full_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    let mut scenarios = Vec::new();
    scenarios.push(ScenarioSpec::new("baseline_idle", baseline_events()));

    for voices in [1, 2, 4, 8, 16, 32, 64] {
        scenarios.push(ScenarioSpec::new(
            format!("synth_ramp_{voices}"),
            synth_ramp_events(voices),
        ));
    }

    for voices in [1, 2, 4, 8, 16, 32, 64] {
        scenarios.push(ScenarioSpec::new(
            format!("sample_ramp_{voices}"),
            sample_ramp_events(voices, sample_rate),
        ));
    }

    for voices in [4, 8, 16, 32] {
        scenarios.push(ScenarioSpec::new(
            format!("mixed_ramp_{voices}_{voices}"),
            mixed_ramp_events(voices, sample_rate),
        ));
    }

    for mode in 0..=4 {
        let name: String = match mode {
            0 => "fx_ramp_none".into(),
            1 => "fx_ramp_1_bus_delay".into(),
            2 => "fx_ramp_4_buses_1_slot".into(),
            3 => "fx_ramp_4_buses_2_slots".into(),
            _ => "fx_ramp_master_global".into(),
        };
        scenarios.push(ScenarioSpec::new(name, fx_ramp_events(mode, sample_rate)));
    }
    scenarios.push(ScenarioSpec::new(
        "bus_heavy_6_bus_fx_2_global",
        bus_heavy_events(),
    ));

    for mode in 1..=4 {
        let name: String = match mode {
            1 => "momentary_filter".into(),
            2 => "momentary_stutter".into(),
            3 => "momentary_pitch_shift".into(),
            _ => "momentary_combined".into(),
        };
        scenarios.push(ScenarioSpec::new(name, momentary_events(mode, sample_rate)));
    }

    scenarios
}

fn overload_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    vec![
        ScenarioSpec::new("synth_one_slot_12_steal", synth_overload_events(12, 1)),
        ScenarioSpec::new(
            "synth_cross_slot_96_steal",
            synth_overload_events(96, INSTRUMENT_SLOT_COUNT),
        ),
        ScenarioSpec::new(
            "sample_one_slot_12_steal",
            sample_overload_events(12, 1, sample_rate),
        ),
        ScenarioSpec::new(
            "sample_cross_slot_96_steal",
            sample_overload_events(96, INSTRUMENT_SLOT_COUNT, sample_rate),
        ),
        ScenarioSpec::new(
            "mixed_cross_slot_48_48_steal",
            mixed_overload_events(48, sample_rate),
        ),
    ]
}

fn soak_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    vec![
        ScenarioSpec::new("safe_soak_mixed_8_8", mixed_ramp_events(8, sample_rate)),
        ScenarioSpec::new("safe_soak_fx_16", fx_ramp_events(2, sample_rate)),
        ScenarioSpec::new("bus_heavy_6_bus_fx_2_global", bus_heavy_events()),
        ScenarioSpec::new(
            "risky_soak_momentary_combined",
            momentary_events(4, sample_rate),
        ),
    ]
}

fn fx_limit_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    let mut scenarios = Vec::new();
    for bus_slots in [0, 2, 4, 6, 8, 10, 12, 15, 18, 21, 24] {
        for momentary in [0, 1, 2] {
            let scope = if bus_slots > 12 {
                "synthetic"
            } else {
                "product"
            };
            scenarios.push(ScenarioSpec::new(
                format!("fx_limits_{scope}_8layers_2global_{bus_slots}bus_{momentary}momentary"),
                fx_limit_events(bus_slots, momentary, sample_rate),
            ));
        }
    }
    scenarios
}

pub fn runtime_step_scenarios() -> Vec<ScenarioSpec> {
    [
        "runtime_step_default",
        "snapshot_only_idle",
        "runtime_snapshot_no_menu_change",
        "menu_snapshot_only",
        "dense_scan_transform_events",
        "dense_scan_transform_snapshot",
        "menu_nav_no_snapshot",
        "menu_snapshot_nav_stress",
        "runtime_noteoff_queue_stress",
        "runtime_noteoff_snapshot_stress",
    ]
    .into_iter()
    .map(|name| ScenarioSpec::new(name, Vec::new()))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::{profile_scenarios, ProfileMode};

    #[test]
    fn baseline_mode_has_only_current_profile_scenarios() {
        let scenarios = profile_scenarios(44_100, ProfileMode::Baseline);

        assert_eq!(scenarios.len(), 13);
        assert_eq!(scenarios[0].name, "baseline_idle");
        assert_eq!(
            ProfileMode::from_str("baseline"),
            Some(ProfileMode::Baseline)
        );
    }
}
