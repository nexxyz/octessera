use super::oled_runtime_fixtures::{present, snapshot, status};
use super::support::{FakeHost, FakeRunner};
use crate::{PlaybackRuntime, RunnerMessage, RuntimeConfig, RuntimePresentationMetrics};
use serde_json::json;

#[test]
fn invalid_first_presentation_is_status_only_and_recovery_is_positive_revisioned() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let output = runtime
        .ingest_runner_messages_with_output(
            vec![
                RunnerMessage::Snapshot {
                    snapshot: json!({"nonOledState": {"available": true}}),
                },
                RunnerMessage::RuntimeStatus { status: status() },
            ],
            &mut host,
        )
        .unwrap();
    assert!(output
        .messages
        .iter()
        .all(|message| !matches!(message, RunnerMessage::Snapshot { .. })));
    assert_eq!(runtime.oled_frame_revision(), 0);

    let recovered = present(&mut runtime, snapshot("recovered"));
    assert!(recovered.iter().all(|message| match message {
        RunnerMessage::Snapshot { snapshot } => snapshot
            .get("oledFrameRevision")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|revision| revision > 0),
        _ => true,
    }));

    let after_valid_absent = present(&mut runtime, json!({"nonOledState": {"still": true}}));
    assert!(matches!(
        after_valid_absent.as_slice(),
        [
            RunnerMessage::Snapshot { snapshot },
            RunnerMessage::RuntimeStatus { status }
        ] if snapshot["oledFrameRevision"].as_u64().is_some_and(|revision| revision > 0)
            && status.error.as_ref().is_some_and(|error| {
                error.recovery == crate::RuntimeRecovery::RetainLastGood
            })
    ));
}

#[test]
fn every_emitted_snapshot_has_a_positive_revision_across_runtime_paths() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let mut host = FakeHost::default();
    let mut runner = FakeRunner::default();

    let first = present(&mut runtime, snapshot("startup"));
    assert_positive_snapshot_revisions(&first);

    let malformed_first = {
        let mut fresh = PlaybackRuntime::new(RuntimeConfig::default());
        present(&mut fresh, json!({"missingPresentation": true}))
    };
    assert_positive_snapshot_revisions(&malformed_first);

    let after_valid_absent = present(&mut runtime, json!({"nonOledState": true}));
    assert_positive_snapshot_revisions(&after_valid_absent);
    let after_valid_malformed = present(
        &mut runtime,
        json!({"display": {"off": false}, "settings": {"displayBrightness": "bad"}}),
    );
    assert_positive_snapshot_revisions(&after_valid_malformed);

    let platform = runtime
        .dispatch_runner_messages(
            vec![RunnerMessage::PlatformEffects {
                effects: vec![crate::RuntimePlatformEffect::StoreListPresets],
            }],
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_positive_snapshot_revisions(&platform.messages);

    let metrics = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        audio_load_ratio: 0.9,
        voice_steal: true,
        ..Default::default()
    });
    assert_positive_snapshot_revisions(&metrics.messages);

    let recovery = runtime
        .recover_from_facts(
            crate::RuntimeErrorFacts::new(
                crate::RuntimeErrorDomain::Storage,
                crate::RuntimeErrorCode::OperationFailed,
                crate::RuntimeOperation::Store,
                Some("retry".into()),
            ),
            &mut runner,
            &mut host,
        )
        .unwrap();
    assert_positive_snapshot_revisions(&recovery.messages);
}

fn assert_positive_snapshot_revisions(messages: &[RunnerMessage]) {
    assert!(messages.iter().all(|message| match message {
        RunnerMessage::Snapshot { snapshot } => snapshot
            .get("oledFrameRevision")
            .and_then(serde_json::Value::as_u64)
            .is_some_and(|revision| revision > 0),
        _ => true,
    }));
}

#[test]
fn unchanged_oled_input_skips_render_but_full_led_and_hdmi_snapshots_still_publish() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let first = present(&mut runtime, snapshot("MENU"));
    assert_positive_snapshot_revisions(&first);
    let pixels = runtime.last_oled_frame().unwrap().to_vec();
    let revision = runtime.oled_frame_revision();
    let renders = runtime.test_oled_render_count();
    assert_eq!(renders, 1);

    let mut revision_only = snapshot("MENU");
    revision_only["oledFrameRevision"] = json!(999);
    let same = present(&mut runtime, revision_only);
    assert!(
        matches!(same.as_slice(), [RunnerMessage::Snapshot { snapshot }, RunnerMessage::RuntimeStatus { .. }]
        if snapshot["oledFrameRevision"] == revision)
    );
    assert_eq!(runtime.test_oled_render_count(), renders);

    let mut changed = snapshot("MENU");
    changed["leds"] = json!({"rgb": [1, 2, 3]});
    changed["hdmiFrame"] = json!({"pixels": [4, 5, 6]});
    changed["nested"] = json!({"oledFrameRevision": 99});
    let output = present(&mut runtime, changed.clone());
    assert!(matches!(output.as_slice(), [
        RunnerMessage::Snapshot { snapshot }, RunnerMessage::RuntimeStatus { .. }
    ] if snapshot["leds"] == changed["leds"]
        && snapshot["hdmiFrame"] == changed["hdmiFrame"]
        && snapshot["nested"] == changed["nested"]
        && snapshot["oledFrameRevision"] == revision));
    assert_eq!(runtime.last_oled_frame(), Some(pixels.as_slice()));
    assert_eq!(runtime.oled_frame_revision(), revision);
    assert_eq!(runtime.test_oled_render_count(), renders);

    let explicit = present(&mut runtime, changed);
    assert!(
        matches!(explicit.as_slice(), [RunnerMessage::Snapshot { snapshot }, RunnerMessage::RuntimeStatus { .. }]
        if snapshot["oledFrameRevision"] == revision)
    );
    assert_eq!(runtime.test_oled_render_count(), renders);
}

#[test]
fn independent_transport_and_selection_cues_each_invalidate_oled_input() {
    for (field, value) in [
        ("transportIcon", json!("play")),
        ("transportFlash", json!("beat")),
        ("eventDotOn", json!(true)),
        ("selectedRow", json!(0)),
    ] {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let mut baseline = snapshot("MENU");
        if field == "transportFlash" {
            baseline["transportIcon"] = json!("play");
        }
        let _ = present(&mut runtime, baseline.clone());
        let previous_pixels = runtime.last_oled_frame().unwrap().to_vec();
        let mut changed = baseline;
        changed[field] = value;
        let output = present(&mut runtime, changed);
        assert_eq!(runtime.test_oled_render_count(), 2, "{field}");
        assert!(
            matches!(output.as_slice(), [
            RunnerMessage::OledFrame { revision: 2, pixels, .. },
            RunnerMessage::Snapshot { snapshot }, RunnerMessage::RuntimeStatus { .. }
        ] if snapshot["oledFrameRevision"] == 2 && pixels != &previous_pixels),
            "{field}"
        );
    }
}

#[test]
fn individual_menu_and_brightness_edits_update_oled_pixels() {
    for field in ["title", "lines", "brightness"] {
        let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
        let _ = present(&mut runtime, snapshot("MENU"));
        let old_pixels = runtime.last_oled_frame().unwrap().to_vec();
        let mut changed = snapshot("MENU");
        match field {
            "title" => changed["display"]["title"] = json!("Help: Basic Help"),
            "lines" => changed["display"]["lines"] = json!(["Try this"]),
            "brightness" => changed["settings"]["displayBrightness"] = json!(35),
            _ => unreachable!(),
        }
        let output = present(&mut runtime, changed);
        assert_eq!(runtime.test_oled_render_count(), 2, "{field}");
        assert!(
            matches!(output.as_slice(), [
            RunnerMessage::OledFrame { revision: 2, pixels, .. },
            RunnerMessage::Snapshot { snapshot }, RunnerMessage::RuntimeStatus { .. }
        ] if snapshot["oledFrameRevision"] == 2 && pixels != &old_pixels),
            "{field}"
        );
    }
}

#[test]
fn visible_oled_inputs_rebuild_pixels_while_metrics_and_errors_remain_live() {
    let mut runtime = PlaybackRuntime::new(RuntimeConfig::default());
    let _ = present(&mut runtime, snapshot("MENU"));
    let initial_pixels = runtime.last_oled_frame().unwrap().to_vec();
    let mut next = snapshot("MENU");
    next["display"]["title"] = json!("Help: Basic Help");
    next["display"]["lines"] = json!(["Try this"]);
    next["selectedRow"] = json!(0);
    next["transportFlash"] = json!("beat");
    next["eventDotOn"] = json!(true);
    next["settings"]["displayBrightness"] = json!(35);
    let shown = present(&mut runtime, next);
    assert!(matches!(shown.as_slice(), [
        RunnerMessage::OledFrame { revision: 2, pixels, .. },
        RunnerMessage::Snapshot { snapshot }, RunnerMessage::RuntimeStatus { .. }
    ] if snapshot["oledFrameRevision"] == 2 && pixels != &initial_pixels));
    assert_eq!(runtime.test_oled_render_count(), 2);

    let metrics = runtime.update_presentation_metrics(RuntimePresentationMetrics {
        worker_utilization: Some(0.9),
        high_cpu_steady: true,
        ..Default::default()
    });
    assert_positive_snapshot_revisions(&metrics.messages);
    assert_eq!(runtime.test_oled_render_count(), 3);

    runtime.latch_facts(crate::RuntimeErrorFacts::new(
        crate::RuntimeErrorDomain::Storage,
        crate::RuntimeErrorCode::OperationFailed,
        crate::RuntimeOperation::Store,
        Some("disk full".into()),
    ));
    assert_eq!(runtime.test_oled_render_count(), 4);
    assert!(runtime
        .last_snapshot()
        .is_some_and(|snapshot| snapshot["runtimeError"]["message"] == "disk full"));
}
