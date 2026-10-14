use crate::render_loop_queue::{NativeSceneCommand, NativeSceneCompletion};
use playback_runtime::{
    oled_frame::OledPresentationMetrics, NativeRunner, NativeRunnerConfig, PresentationScene,
};
use std::sync::mpsc;

pub(crate) fn test_runner() -> NativeRunner {
    let payload: serde_json::Value =
        serde_json::from_str(include_str!("../../../../config/generated/pi/default.json")).unwrap();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.skip_startup_splash();
    runner
}

pub(crate) fn test_scene() -> PresentationScene {
    test_runner().capture_display_scene().unwrap()
}

pub(crate) fn test_scene_command() -> (
    Box<NativeSceneCommand>,
    mpsc::Receiver<NativeSceneCompletion>,
) {
    let (completion, receiver) = mpsc::channel();
    let command = Box::new(NativeSceneCommand {
        scene: test_scene(),
        metrics: OledPresentationMetrics::default(),
        error: None,
        completion,
    });
    (command, receiver)
}
