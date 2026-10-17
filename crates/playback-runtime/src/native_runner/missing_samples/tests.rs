use super::super::{NativeRunnerConfig, NativeSampleAvailability};
use super::*;
use crate::{CoreRunner, HostMessage, RuntimeStoreResult};

fn runner_with_two_samples() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[1].kind = "sampler".into();
    runner.instruments[1].sample_paths[0] = Some("samples/Drum/kick/Kick2.wav".into());
    runner.instruments[1].sample_paths[1] = Some("userdata/User Kit/gone.wav".into());
    runner
}

fn report_missing(runner: &mut NativeRunner, paths: &[&str]) {
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::SamplesMissing {
                paths: paths.iter().map(|path| path.to_string()).collect(),
            },
        })
        .unwrap();
}

#[test]
fn reported_missing_sample_marks_only_its_slot_and_lists_it_in_a_popup() {
    let mut runner = runner_with_two_samples();
    report_missing(&mut runner, &["userdata/User Kit/gone.wav"]);

    assert_eq!(
        runner.sample_availability[1][1],
        NativeSampleAvailability::Unavailable
    );
    assert_ne!(
        runner.sample_availability[1][0],
        NativeSampleAvailability::Unavailable
    );
    let popup = runner
        .display
        .help_popup
        .as_ref()
        .expect("missing samples popup");
    assert_eq!(popup.title, "Samples missing");
    assert!(
        popup.lines.join(" ").contains("gone.wav"),
        "{:?}",
        popup.lines
    );
    assert!(runner.display.runtime_error_presentation.is_none());
}

#[test]
fn already_reported_missing_samples_do_not_reopen_the_popup() {
    let mut runner = runner_with_two_samples();
    report_missing(&mut runner, &["userdata/User Kit/gone.wav"]);
    runner.display.help_popup = None;

    report_missing(&mut runner, &["userdata/User Kit/gone.wav"]);

    assert!(runner.display.help_popup.is_none());
}

#[test]
fn unrelated_missing_paths_are_ignored() {
    let mut runner = runner_with_two_samples();
    report_missing(&mut runner, &["samples/Drum/kick/elsewhere.wav"]);

    assert!(runner.display.help_popup.is_none());
    assert!(runner.sample_availability[1]
        .iter()
        .all(|availability| *availability != NativeSampleAvailability::Unavailable));
}
