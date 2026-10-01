use super::*;
use serde_json::json;

#[test]
fn accepts_browser_relative_wav_paths_without_requiring_media_to_exist() {
    for path in [
        "userdata/User Kit/custom.wav",
        "sd-card/octessera/samples/kick.wav",
        "Drums/kick.wav",
        "missing/on/card.wav",
    ] {
        validate_local_patch_sample_paths(&json!({
            "runtimeConfig": {
                "instruments": [{ "sample": { "slots": [{ "path": path }] } }]
            }
        }))
        .unwrap();
    }
}

#[test]
fn rejects_absolute_traversal_malformed_and_non_wav_paths() {
    for path in [
        "",
        "/userdata/custom.wav",
        "C:/userdata/custom.wav",
        "C:\\userdata\\custom.wav",
        "//server/share/custom.wav",
        "userdata/../custom.wav",
        "userdata/./custom.wav",
        "userdata//custom.wav",
        "userdata/custom.aiff",
        "userdata:stream/custom.wav",
        "userdata\\custom.wav",
    ] {
        assert!(
            validate_local_patch_sample_paths(&json!({
                "runtimeConfig": {
                    "instruments": [{ "sample": { "slots": [{ "path": path }] } }]
                }
            }))
            .is_err(),
            "accepted unsafe local sample path {path:?}"
        );
    }
}
