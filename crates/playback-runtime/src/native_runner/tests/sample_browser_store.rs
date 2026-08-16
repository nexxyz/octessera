use super::*;
use crate::native_menu::NativeMenuItem;
use crate::{RuntimeErrorDomain, RuntimeErrorFacts, RuntimeOperation};

pub(crate) fn open_browser(dir: &str) -> NativeSampleBrowser {
    NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: dir.into(),
        entries: vec![],
    }
}

fn sampler_with_path(path: &str) -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.instruments[0].kind = "sampler".into();
    runner.instruments[0].sample_paths[0] = Some(path.into());
    runner.menu.rebuild(runner.menu_config());
    runner
}

fn item_label(item: &NativeMenuItem, key: &str) -> Option<String> {
    if item.key.as_deref() == Some(key) {
        return Some(item.label.clone());
    }
    item.children
        .iter()
        .find_map(|child| item_label(child, key))
}

fn sample_failure(path: &str) -> RuntimeStoreResult {
    RuntimeStoreResult::RuntimeFailure {
        error: RuntimeErrorFacts::new(
            RuntimeErrorDomain::Sample,
            RuntimeErrorCode::NotFound,
            RuntimeOperation::AudioCommand,
            Some(format!("sample not found: {path}")),
        ),
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
    runner.display.toast = None;
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
    assert!(runner.display.toast.is_none());
}

#[test]
pub(crate) fn missing_bundled_and_local_samples_show_na_without_rewriting_paths() {
    for path in [
        "samples/Drum/kick/missing-default.wav",
        "userdata/User Kit/missing-user.wav",
        "sd-card/octessera/samples/missing-sd.wav",
    ] {
        let mut runner = sampler_with_path(path);
        runner.apply_store_result(sample_failure(path)).unwrap();

        assert_eq!(runner.instruments[0].sample_paths[0].as_deref(), Some(path));
        assert_eq!(
            item_label(&runner.menu.root, &format!("sample.loaded:0:0:{path}")),
            Some(format!("N/A-{}", path.rsplit('/').next().unwrap()))
        );
        assert_eq!(
            runner.config_payload()["runtimeConfig"]["instruments"][0]["sample"]["slots"][0]
                ["path"],
            json!(path)
        );
    }
}

#[test]
pub(crate) fn missing_sample_directory_listing_exposes_original_path_for_replacement() {
    let path = "userdata/User Kit/missing.wav";
    let mut runner = sampler_with_path(path);
    runner.sample_browser = Some(open_browser("userdata/User Kit"));

    runner
        .apply_store_result(RuntimeStoreResult::SampleListResult {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "userdata/User Kit".into(),
            entries: vec![SampleEntry {
                name: "other.wav".into(),
                path: "userdata/User Kit/other.wav".into(),
                is_dir: false,
            }],
        })
        .unwrap();

    let browser = runner.sample_browser.as_ref().unwrap();
    assert!(browser.entries.iter().any(|entry| {
        entry.name == "N/A-missing.wav" && entry.path == "userdata/User Kit/missing.wav"
    }));
    assert_eq!(
        item_label(
            &runner.menu.root,
            "sample.pick.0.0.userdata/User Kit/missing.wav",
        ),
        Some("N/A-missing.wav".into())
    );
}

#[test]
pub(crate) fn missing_sample_directory_error_still_shows_na_browser_row() {
    let path = "samples/Drum/missing-folder/missing.wav";
    let mut runner = sampler_with_path(path);
    runner.sample_browser = Some(open_browser("samples/Drum/missing-folder"));

    runner
        .apply_store_result(RuntimeStoreResult::SampleListError {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "samples/Drum/missing-folder".into(),
            message: "directory not found".into(),
        })
        .unwrap();

    assert_eq!(
        runner.sample_browser.as_ref().unwrap().entries,
        vec![SampleEntry {
            name: "N/A-missing.wav".into(),
            path: path.into(),
            is_dir: false,
        }]
    );
}

#[test]
pub(crate) fn available_sample_listing_keeps_normal_display() {
    let path = "sd-card/octessera/samples/kick.wav";
    let mut runner = sampler_with_path(path);
    runner.sample_browser = Some(open_browser("sd-card/octessera/samples"));

    runner
        .apply_store_result(RuntimeStoreResult::SampleListResult {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "sd-card/octessera/samples".into(),
            entries: vec![SampleEntry {
                name: "kick.wav".into(),
                path: path.into(),
                is_dir: false,
            }],
        })
        .unwrap();

    assert_eq!(
        item_label(
            &runner.menu.root,
            "sample.loaded:0:0:sd-card/octessera/samples/kick.wav",
        ),
        Some("kick.wav".into())
    );
    assert_eq!(
        runner.sample_availability[0][0],
        NativeSampleAvailability::Available
    );
}

#[test]
pub(crate) fn replacement_and_assignment_preserve_sample_path_contract() {
    let missing = "userdata/User Kit/missing.wav";
    let replacement = "userdata/User Kit/replacement.wav";
    let mut runner = sampler_with_path(missing);
    runner.apply_store_result(sample_failure(missing)).unwrap();
    runner
        .handle_sample_browser_action(&format!("sample.pick:0:0:{replacement}"))
        .unwrap();
    assert_eq!(
        runner.instruments[0].sample_paths[0].as_deref(),
        Some(replacement)
    );
    assert!(runner.instruments[0].sample_assignments.is_empty());

    runner.assign_sample_cell(0, 0, 3, 4);
    assert_eq!(
        runner.instruments[0].sample_paths[0].as_deref(),
        Some(replacement)
    );
    assert_eq!(runner.instruments[0].sample_assignments[0].sample_slot, 0);
}

#[test]
pub(crate) fn unavailable_status_survives_a_same_path_config_transaction() {
    let path = "samples/Drum/kick/missing.wav";
    let mut runner = sampler_with_path(path);
    runner.apply_store_result(sample_failure(path)).unwrap();
    let payload = runner.config_payload();

    runner.apply_config_payload(payload).unwrap();

    assert_eq!(
        runner.sample_availability[0][0],
        NativeSampleAvailability::Unavailable
    );
    assert_eq!(
        item_label(&runner.menu.root, &format!("sample.loaded:0:0:{path}")),
        Some("N/A-missing.wav".into())
    );
}
