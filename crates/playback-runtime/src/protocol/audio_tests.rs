use super::*;
use serde_json::json;

#[test]
fn device_config_reboot_wire_names_deserialize_with_canonical_and_legacy_alias() {
    let payload = json!({
        "runtimeConfig": {
            "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
            "usb": { "midiOutEnabled": false }
        }
    });
    for wire_name in ["apply_device_config_reboot", "usb_apply_reboot"] {
        let request: RuntimePlatformRequest = serde_json::from_value(json!({
            "effect": { "type": wire_name, "payload": payload },
            "requestId": "request-1"
        }))
        .unwrap();
        assert!(matches!(
            request.effect,
            RuntimePlatformEffect::ApplyDeviceConfigReboot { .. }
        ));
    }
}

#[test]
fn device_config_reboot_wire_name_serializes_canonically() {
    assert_eq!(
        serde_json::to_value(RuntimePlatformEffect::ApplyDeviceConfigReboot {
            payload: json!({
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "midiOutEnabled": false }
                }
            }),
        })
        .unwrap(),
        json!({
            "type": "apply_device_config_reboot",
            "payload": {
                "runtimeConfig": {
                    "audioOutputs": { "dac": true, "usb": false, "hdmi": false },
                    "usb": { "midiOutEnabled": false }
                }
            }
        })
    );
}
