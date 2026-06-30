use super::*;

pub(crate) fn looper_runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig {
        behavior_id: "looper".into(),
        behavior_config: json!({ "mode": "overdub", "lengthSteps": 2 }),
        note_behaviors: vec![platform_core::NoteBehavior::Hold; 16],
        ..NativeRunnerConfig::default()
    })
    .unwrap();
    runner.transport = RuntimeTransportState::Playing;
    runner
}

pub(crate) fn looper_mode_and_step(runner: &NativeRunner) -> (String, usize) {
    match runner.engine.state() {
        platform_core::NativeBehaviorState::Looper(state) => (state.mode.clone(), state.step_index),
        _ => panic!("expected looper state"),
    }
}

pub(crate) fn has_note_off(messages: &[RunnerMessage]) -> bool {
    messages.iter().any(|message| match message {
        RunnerMessage::MusicalEvents { events } => events
            .iter()
            .any(|event| matches!(event, platform_core::MusicalEvent::NoteOff { .. })),
        _ => false,
    })
}

pub(crate) fn pulse_step(runner: &mut NativeRunner) {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 12,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(false),
        })
        .unwrap();
}

mod part_1;
mod part_2;
pub(super) use part_2::*;
