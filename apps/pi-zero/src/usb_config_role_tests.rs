use super::*;

fn payload(role: &str, usb_audio: bool, midi: bool) -> serde_json::Value {
    serde_json::json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": usb_audio, "hdmi": false },
            "usb": { "dataRole": role, "midiOutEnabled": midi }
        }
    })
}

#[test]
fn data_role_is_required() {
    assert!(parse_usb_runtime_config(&serde_json::json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
            "usb": { "midiOutEnabled": false }
        }
    }))
    .is_err());
}

#[test]
fn host_data_role_requires_audio_and_midi_off() {
    assert_eq!(
        parse_usb_runtime_config(&payload("host", false, false))
            .unwrap()
            .data_role,
        UsbDataRole::Host
    );
    for invalid in [payload("host", true, false), payload("host", false, true)] {
        assert!(parse_usb_runtime_config(&invalid).is_err());
        assert!(crate::usb_config_validation::validate_raspberry_usb_payload(&invalid).is_err());
    }
}

#[test]
fn invalid_data_role_is_rejected_at_both_pi_boundaries() {
    let invalid = payload("sideways", false, false);
    assert!(parse_usb_runtime_config(&invalid).is_err());
    assert!(crate::usb_config_validation::validate_raspberry_usb_payload(&invalid).is_err());
}
