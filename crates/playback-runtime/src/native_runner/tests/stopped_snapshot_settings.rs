use super::*;

fn snapshot_of(messages: &[RunnerMessage]) -> &Value {
    messages
        .iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .expect("snapshot")
}

fn stopped_deadline_snapshot(runner: &mut NativeRunner) -> Vec<RunnerMessage> {
    runner
        .send(HostMessage::TransportPulseStep {
            pulses: 0,
            source: SyncSource::Internal,
            at_ppqn_pulse: None,
            request_snapshot: Some(true),
        })
        .unwrap()
}

#[test]
fn a_stopped_deadline_snapshot_omits_audio_settings_that_did_not_change() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    assert_eq!(runner.transport.transport, RuntimeTransportState::Stopped);
    let first = stopped_deadline_snapshot(&mut runner);
    assert!(snapshot_of(&first)["settings"]["instruments"].is_array());

    let unchanged = stopped_deadline_snapshot(&mut runner);
    let settings = &snapshot_of(&unchanged)["settings"];
    assert!(settings.get("instruments").is_none());
    assert!(settings.get("mixer").is_none());
    assert!(settings.get("panPositions").is_none());
    assert_eq!(
        settings["displayBrightness"],
        runner.display.ui.display_brightness
    );

    runner.audio_config_revision += 1;
    let changed = stopped_deadline_snapshot(&mut runner);
    let settings = &snapshot_of(&changed)["settings"];
    assert_eq!(settings["instruments"].as_array().unwrap().len(), 8);
    assert_eq!(
        settings["audioConfigRevision"],
        runner.audio_config_revision
    );
}

#[test]
fn audio_settings_queued_without_a_snapshot_still_reach_the_next_one() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    stopped_deadline_snapshot(&mut runner);

    runner.audio_config_revision += 1;
    runner.queue_audio_config_if_changed();
    let next = stopped_deadline_snapshot(&mut runner);
    assert!(snapshot_of(&next)["settings"]["instruments"].is_array());
}
