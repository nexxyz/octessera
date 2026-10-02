use super::{platform_request, test_adapter};
use playback_runtime::{HostAdapter, RuntimeAudioCommand, RuntimePlatformEffect};

fn duck_slot(fx_type: &str, params: serde_json::Value) -> RuntimePlatformEffect {
    RuntimePlatformEffect::AudioCommand {
        command: RuntimeAudioCommand::SetFxBusSlot {
            bus_index: 2,
            slot_index: 0,
            generation: 17,
            fx_type: fx_type.into(),
            params: serde_json::from_value(params).unwrap(),
        },
    }
}

#[test]
fn desktop_accepts_duck_slot_with_source_and_tap_strings() {
    let (mut adapter, _rx) = test_adapter();
    adapter
        .handle_platform_effect(&platform_request(duck_slot(
            "duck",
            serde_json::json!({ "amountPct": 60, "attackMs": 8, "releaseMs": 160, "source": "I1", "sourceTap": "pre", "threshold": 0.08 }),
        )))
        .unwrap();
}

#[test]
fn desktop_rejects_invalid_duck_strings_and_strings_on_other_fx() {
    let (mut adapter, _rx) = test_adapter();
    for (fx_type, params) in [
        ("duck", serde_json::json!({ "source": "B9" })),
        ("duck", serde_json::json!({ "sourceTap": "sideways" })),
        ("delay", serde_json::json!({ "source": "I1" })),
    ] {
        assert!(
            adapter
                .handle_platform_effect(&platform_request(duck_slot(fx_type, params)))
                .is_err(),
            "{fx_type}"
        );
    }
}
