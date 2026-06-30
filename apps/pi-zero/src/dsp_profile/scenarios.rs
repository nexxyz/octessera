use realtime_engine::synth::INSTRUMENT_SLOT_COUNT;
use rodio_engine_source::EngineEvent;

#[path = "fx_cases.rs"]
mod fx_cases;
#[path = "scenario_events.rs"]
mod scenario_events;

use fx_cases::bus_heavy_events;
use scenario_events::{
    baseline_events, fx_ramp_events, mixed_overload_events, mixed_ramp_events, momentary_events,
    sample_overload_events, sample_ramp_events, synth_overload_events, synth_ramp_events,
};

pub struct ScenarioSpec {
    pub name: String,
    pub events: Vec<EngineEvent>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMode {
    Full,
    Overload,
    Soak,
}

impl ProfileMode {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "full" => Some(Self::Full),
            "overload" | "steal" | "stealing" => Some(Self::Overload),
            "soak" => Some(Self::Soak),
            _ => None,
        }
    }
}

pub fn profile_scenarios(sample_rate: u32, mode: ProfileMode) -> Vec<ScenarioSpec> {
    match mode {
        ProfileMode::Full => full_scenarios(sample_rate),
        ProfileMode::Overload => overload_scenarios(sample_rate),
        ProfileMode::Soak => soak_scenarios(sample_rate),
    }
}

fn full_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    let mut scenarios = Vec::new();
    scenarios.push(ScenarioSpec {
        name: "baseline_idle".into(),
        events: baseline_events(),
    });

    for voices in [1, 2, 4, 8, 16, 32, 64] {
        scenarios.push(ScenarioSpec {
            name: format!("synth_ramp_{voices}"),
            events: synth_ramp_events(voices),
        });
    }

    for voices in [1, 2, 4, 8, 16, 32, 64] {
        scenarios.push(ScenarioSpec {
            name: format!("sample_ramp_{voices}"),
            events: sample_ramp_events(voices, sample_rate),
        });
    }

    for voices in [4, 8, 16, 32] {
        scenarios.push(ScenarioSpec {
            name: format!("mixed_ramp_{voices}_{voices}"),
            events: mixed_ramp_events(voices, sample_rate),
        });
    }

    for mode in 0..=4 {
        let name = match mode {
            0 => "fx_ramp_none".into(),
            1 => "fx_ramp_1_bus_delay".into(),
            2 => "fx_ramp_4_buses_1_slot".into(),
            3 => "fx_ramp_4_buses_2_slots".into(),
            _ => "fx_ramp_master_global".into(),
        };
        scenarios.push(ScenarioSpec {
            name,
            events: fx_ramp_events(mode, sample_rate),
        });
    }
    scenarios.push(ScenarioSpec {
        name: "bus_heavy_6_bus_fx_2_global".into(),
        events: bus_heavy_events(),
    });

    for mode in 1..=4 {
        let name = match mode {
            1 => "momentary_filter".into(),
            2 => "momentary_stutter".into(),
            3 => "momentary_pitch_shift".into(),
            _ => "momentary_combined".into(),
        };
        scenarios.push(ScenarioSpec {
            name,
            events: momentary_events(mode, sample_rate),
        });
    }

    scenarios
}

fn overload_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    vec![
        ScenarioSpec {
            name: "synth_one_slot_12_steal".into(),
            events: synth_overload_events(12, 1),
        },
        ScenarioSpec {
            name: "synth_cross_slot_96_steal".into(),
            events: synth_overload_events(96, INSTRUMENT_SLOT_COUNT),
        },
        ScenarioSpec {
            name: "sample_one_slot_12_steal".into(),
            events: sample_overload_events(12, 1, sample_rate),
        },
        ScenarioSpec {
            name: "sample_cross_slot_96_steal".into(),
            events: sample_overload_events(96, INSTRUMENT_SLOT_COUNT, sample_rate),
        },
        ScenarioSpec {
            name: "mixed_cross_slot_48_48_steal".into(),
            events: mixed_overload_events(48, sample_rate),
        },
    ]
}

fn soak_scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    vec![
        ScenarioSpec {
            name: "safe_soak_mixed_8_8".into(),
            events: mixed_ramp_events(8, sample_rate),
        },
        ScenarioSpec {
            name: "safe_soak_fx_16".into(),
            events: fx_ramp_events(2, sample_rate),
        },
        ScenarioSpec {
            name: "bus_heavy_6_bus_fx_2_global".into(),
            events: bus_heavy_events(),
        },
        ScenarioSpec {
            name: "risky_soak_momentary_combined".into(),
            events: momentary_events(4, sample_rate),
        },
    ]
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
    .map(|name| ScenarioSpec {
        name: name.into(),
        events: Vec::new(),
    })
    .collect()
}
