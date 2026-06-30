use playback_runtime::NativeRunnerConfig;

pub(super) fn desktop_native_runner_config() -> NativeRunnerConfig {
    NativeRunnerConfig {
        behavior_id: "sequencer".into(),
        sample_builtin_favourite_dirs: vec!["userdata".into()],
        ..NativeRunnerConfig::default()
    }
}
