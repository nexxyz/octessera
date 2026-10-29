use super::*;

pub(super) fn new_initial_with_dependencies(
    sink: AudioSink,
    initial: OpenedAudioSink,
    dependencies: OrangeRecoveryDependencies,
) -> Result<OrangeRecoveryController, String> {
    let controller = OrangeRecoveryController::new_with_dependencies(
        sink,
        initial.health.clone(),
        Some(initial),
        OrangeRecoveryPhase::Healthy,
        dependencies,
    );
    let engine_tx = controller
        .current
        .as_ref()
        .expect("initial audio stream")
        .engine_tx
        .as_ref()
        .expect("initial Jack engine event sender")
        .clone();
    controller.engine.attach(engine_tx)?;
    Ok(controller)
}
