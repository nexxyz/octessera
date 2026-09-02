use super::*;

pub(super) fn new_initial_with_dependencies(
    sink: AudioSink,
    mode: OrangeRecoveryMode,
    initial: OpenedAudioSink,
    dependencies: OrangeRecoveryDependencies,
) -> Result<OrangeRecoveryController, String> {
    let controller = OrangeRecoveryController::new_with_dependencies(
        sink,
        mode,
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
        .clone();
    attach_sink_atomic(
        &controller.attach_gate,
        &controller.realtime_txs,
        &controller.replay_events,
        sink,
        engine_tx,
    )?;
    Ok(controller)
}
