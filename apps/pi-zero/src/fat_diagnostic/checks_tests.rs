use super::orange_checks::{orange_audio_route_matches, orange_oled_handoff_from_readiness};
use super::{
    input_check, raspberry_audio_card_listed, setup_status_check_paths,
    status_file_oled_handoff_check, CheckContext,
};
use crate::board_profile::FAT_RASPBERRY_PI_ZERO_2W;
use crate::fat_diagnostic::model::{CheckOutcome, CheckStatus};
use serde_json::json;
use std::fs;
use std::time::Duration;

#[test]
fn input_check_is_operator_required_without_hardware_access() {
    let outcome = input_check(&CheckContext {
        board: FAT_RASPBERRY_PI_ZERO_2W,
        timeout: Duration::from_secs(1),
        executable: None,
    });
    assert_eq!(outcome.status, CheckStatus::OperatorRequired);
}

#[test]
fn absent_setup_status_is_operator_required_for_image_flash_customization() {
    let root = test_root("setup-status");
    let outcome = setup_status_check_paths(&root.join("public"));
    assert_eq!(outcome.status, CheckStatus::OperatorRequired);
    assert!(outcome.message.contains("image-flash customization"));
    assert!(!root.exists());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn setup_status_accepts_backend_envelope_phases_and_rejects_missing_type() {
    let root = test_root("setup-status-valid");
    let public = root.join("public");
    fs::create_dir_all(&public).unwrap();
    let current = public.join("current.json");
    for status in [
        json!({"type":"setup_portal_status","phase":"starting","disposition":"accepted","rebootRequired":false}),
        json!({"type":"setup_portal_status","phase":"portal_ready","portalSuffix":"abcd","rebootRequired":false}),
        json!({"type":"setup_portal_status","phase":"finalizing","rebootRequired":false}),
        json!({"type":"setup_portal_status","phase":"succeeded","rebootRequired":false}),
        json!({"type":"setup_portal_status","phase":"failed","errorCode":"operation_failed","rebootRequired":false}),
        json!({"type":"setup_portal_status","phase":"timed_out","errorCode":"unavailable","rebootRequired":false}),
    ] {
        fs::write(
            &current,
            serde_json::to_vec(&json!({"schema":1,"status":status})).unwrap(),
        )
        .unwrap();
        assert_eq!(setup_status_check_paths(&public).status, CheckStatus::Pass);
    }

    fs::write(
        &current,
        serde_json::to_vec(&json!({
            "schema": 1,
            "status": {"phase":"starting","disposition":"accepted","rebootRequired":false}
        }))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(setup_status_check_paths(&public).status, CheckStatus::Fail);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn orange_handoff_does_not_require_the_protected_status_file() {
    let root = test_root("protected-oled-handoff");
    let readiness = CheckOutcome {
        status: CheckStatus::Pass,
        message: "current readiness is valid".into(),
        artifact: "03-readiness.txt".into(),
        artifact_content: "marker=current\nsystemd=matching".into(),
    };

    assert_eq!(
        status_file_oled_handoff_check(&root).status,
        CheckStatus::Fail
    );
    let outcome = orange_oled_handoff_from_readiness(readiness);
    assert_eq!(outcome.status, CheckStatus::Pass);
    assert!(outcome.message.contains("matching service invocation"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn raspberry_status_file_handoff_still_requires_native_marker_and_lock() {
    let root = test_root("raspberry-oled-handoff");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("status.json"),
        br#"{"schema":1,"phase":"first_menu_rendered"}"#,
    )
    .unwrap();
    fs::write(root.join("oled.lock"), b"").unwrap();
    assert_eq!(
        status_file_oled_handoff_check(&root).status,
        CheckStatus::Pass
    );
    fs::remove_file(root.join("oled.lock")).unwrap();
    assert_eq!(
        status_file_oled_handoff_check(&root).status,
        CheckStatus::Fail
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn orange_audio_requires_the_exact_card_and_playback_pcm() {
    let cards =
        " 1 [octesseradac   ]: octessera-dac - octessera-dac\n                      octessera-dac\n";
    let pcm = "01-00: sunxi-audio : playback 1\n";
    assert!(orange_audio_route_matches(cards, pcm));
    for invalid in [
        (
            cards,
            "00-00: sunxi-audio : playback 1\n",
        ),
        (cards, "01-00: sunxi-audio : playback 0\n"),
        (cards, "01-01: sunxi-audio : playback 1\n"),
        (
            " 1 [othercard      ]: octessera-dac - octessera-dac\n",
            pcm,
        ),
        (
            " 1 [octesseradac   ]: octessera-dac - octessera-dac\n 2 [octesseradac   ]: octessera-dac - octessera-dac\n",
            pcm,
        ),
    ] {
        assert!(!orange_audio_route_matches(invalid.0, invalid.1));
    }
}

#[test]
fn raspberry_audio_keeps_case_insensitive_aplay_fragment_matching() {
    assert!(raspberry_audio_card_listed(
        "card 0: HIFIBERRY DAC\n",
        FAT_RASPBERRY_PI_ZERO_2W.audio_card_fragments
    ));
    assert!(!raspberry_audio_card_listed(
        "card 0: octessera-dac\n",
        FAT_RASPBERRY_PI_ZERO_2W.audio_card_fragments
    ));
}

fn test_root(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "octessera-fat-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
