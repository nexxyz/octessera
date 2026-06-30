use crate::protocol::SyncSource;

use platform_core::{
    default_mapping_config, AxisStrategy, GlobalSoundConfig, InterpretationEventProfile,
    InterpretationProfile, InterpretationStateProfile, NoteBehavior, TickStrategy, VelocityCurve,
};
use serde_json::Value;

#[derive(Clone, Debug)]
pub struct NativeRunnerConfig {
    pub behavior_id: String,
    pub behavior_config: Value,
    pub interpretation_profile: InterpretationProfile,
    pub mapping_config: platform_core::MappingConfig,
    pub global_sound: GlobalSoundConfig,
    pub note_behaviors: Vec<NoteBehavior>,
    pub sync_source: SyncSource,
    pub bpm: f64,
    pub sample_builtin_favourite_dirs: Vec<String>,
}

impl Default for NativeRunnerConfig {
    fn default() -> Self {
        Self {
            behavior_id: "life".into(),
            behavior_config: Value::Null,
            interpretation_profile: InterpretationProfile {
                id: "native_profile".into(),
                event: InterpretationEventProfile { enabled: true },
                state: InterpretationStateProfile {
                    enabled: true,
                    tick: TickStrategy::WholeGridTransitions,
                },
                x: AxisStrategy::ScaleStep { step: 1 },
                y: AxisStrategy::TimingOnly,
            },
            mapping_config: default_mapping_config(),
            global_sound: GlobalSoundConfig {
                velocity_scale_pct: 100,
                velocity_curve: VelocityCurve::Linear,
                note_length_ms: 120,
            },
            note_behaviors: vec![NoteBehavior::Oneshot; 16],
            sync_source: SyncSource::Internal,
            bpm: 120.0,
            sample_builtin_favourite_dirs: Vec::new(),
        }
    }
}
