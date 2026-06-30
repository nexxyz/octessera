use super::*;

pub(crate) fn open_browser(dir: &str) -> NativeSampleBrowser {
    NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: dir.into(),
        entries: vec![],
    }
}

#[test]
pub(crate) fn matching_sample_list_result_applies_to_open_browser() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sample_browser = Some(open_browser("Drums"));

    runner
        .apply_store_result(RuntimeStoreResult::SampleListResult {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "Drums".into(),
            entries: vec![SampleEntry {
                name: "kick.wav".into(),
                path: "Drums/kick.wav".into(),
                is_dir: false,
            }],
        })
        .unwrap();

    let browser = runner.sample_browser.as_ref().unwrap();
    assert_eq!(browser.entries.len(), 1);
    assert_eq!(browser.entries[0].path, "Drums/kick.wav");
}

#[test]
pub(crate) fn mismatched_sample_list_result_is_ignored() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sample_browser = Some(open_browser("Drums"));
    let before = runner.sample_browser.clone();
    let before_menu = runner.menu.snapshot();

    runner
        .apply_store_result(RuntimeStoreResult::SampleListResult {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "Bass".into(),
            entries: vec![SampleEntry {
                name: "bass.wav".into(),
                path: "Bass/bass.wav".into(),
                is_dir: false,
            }],
        })
        .unwrap();

    assert_eq!(runner.sample_browser, before);
    assert_eq!(runner.menu.snapshot(), before_menu);
}

#[test]
pub(crate) fn mismatched_sample_list_error_is_ignored_without_toast() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.sample_browser = Some(open_browser("Drums"));
    runner.toast = None;
    let before = runner.sample_browser.clone();
    let before_menu = runner.menu.snapshot();

    runner
        .apply_store_result(RuntimeStoreResult::SampleListError {
            instrument_slot: 1,
            sample_slot: 0,
            dir: "Drums".into(),
            message: "stale error".into(),
        })
        .unwrap();

    assert_eq!(runner.sample_browser, before);
    assert_eq!(runner.menu.snapshot(), before_menu);
    assert!(runner.toast.is_none());
}
