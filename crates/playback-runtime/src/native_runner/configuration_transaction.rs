use crate::protocol::RuntimeAudioCommand;

use super::{NativeRunner, Value};

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ConfigurationAggregate {
    payload: Value,
    audio_config: Value,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ConfigurationRuntimePlan {
    NoRuntimeChange,
    DynamicAudioCommands(Vec<RuntimeAudioCommand>),
    FullRevisionedConfiguration { revision: u64 },
}

impl ConfigurationAggregate {
    pub(super) fn from_runner(runner: &NativeRunner) -> Self {
        Self {
            payload: runner.config_payload(),
            audio_config: audio_config_payload(runner),
        }
    }

    pub(super) fn resolve_plan(
        &self,
        next: &Self,
        current_audio_revision: u64,
    ) -> ConfigurationRuntimePlan {
        if self == next || self.audio_config == next.audio_config {
            ConfigurationRuntimePlan::NoRuntimeChange
        } else {
            ConfigurationRuntimePlan::FullRevisionedConfiguration {
                revision: current_audio_revision.saturating_add(1),
            }
        }
    }
}

impl ConfigurationRuntimePlan {
    pub(super) fn dynamic_audio_command(command: RuntimeAudioCommand) -> Self {
        Self::DynamicAudioCommands(vec![command])
    }

    pub(super) fn full_revision(&self) -> Option<u64> {
        match self {
            Self::FullRevisionedConfiguration { revision } => Some(*revision),
            Self::NoRuntimeChange | Self::DynamicAudioCommands(_) => None,
        }
    }
}

impl NativeRunner {
    pub(super) fn configuration_aggregate(&self) -> ConfigurationAggregate {
        ConfigurationAggregate::from_runner(self)
    }

    pub(super) fn enqueue_configuration_runtime_plan(&mut self, plan: ConfigurationRuntimePlan) {
        match plan {
            ConfigurationRuntimePlan::NoRuntimeChange => {}
            ConfigurationRuntimePlan::DynamicAudioCommands(commands) => {
                for command in commands {
                    self.outbox.push_audio_command(command);
                }
            }
            ConfigurationRuntimePlan::FullRevisionedConfiguration { revision } => {
                self.outbox
                    .push_audio_command(RuntimeAudioCommand::SetAudioConfig {
                        revision,
                        request_id: None,
                        config: audio_config_payload(self),
                    });
                self.outbox
                    .push_audio_command(RuntimeAudioCommand::SetDspConfig {
                        config: self.dsp_config,
                    });
            }
        }
    }

    pub(super) fn commit_configuration_runtime_plan(&mut self, plan: &ConfigurationRuntimePlan) {
        if let Some(revision) = plan.full_revision() {
            self.audio_config_revision = revision;
        }
    }

    pub(super) fn commit_full_configuration_runtime_plan(&mut self) {
        let plan = ConfigurationRuntimePlan::FullRevisionedConfiguration {
            revision: self.audio_config_revision.saturating_add(1),
        };
        self.commit_configuration_runtime_plan(&plan);
    }
}

fn audio_config_payload(runner: &NativeRunner) -> Value {
    let mut config = runner.audio_snapshot_payload();
    if let Value::Object(fields) = &mut config {
        fields.insert(
            "masterVolume".into(),
            serde_json::json!(runner.display.ui.master_volume),
        );
        fields.insert(
            "voiceStealingMode".into(),
            serde_json::json!(runner.voice_stealing_mode),
        );
        fields.insert("dsp".into(), serde_json::json!(runner.dsp_config));
    }
    config
}
