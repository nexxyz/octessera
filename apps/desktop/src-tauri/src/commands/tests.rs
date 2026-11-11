use super::ui_host_message;
use playback_runtime::HostMessage;
use serde_json::json;

#[test]
fn the_ui_may_send_device_input_and_the_snapshot_pulse() {
    assert!(matches!(
        ui_host_message(json!({ "type": "device_input", "input": { "type": "button_a" } })),
        Ok(HostMessage::DeviceInput { .. })
    ));
    assert!(matches!(
        ui_host_message(json!({
            "type": "transport_pulse_step",
            "pulses": 0,
            "source": "internal",
            "requestSnapshot": true,
        })),
        Ok(HostMessage::TransportPulseStep { .. })
    ));
}

#[test]
fn the_ui_may_not_send_host_results_or_midi() {
    for message in [
        json!({ "type": "runtime_result", "result": { "type": "save_system_result", "ok": true } }),
        json!({ "type": "midi_realtime_start" }),
        json!({ "type": "midi_control_change", "channel": 0, "controller": 1, "value": 2 }),
        json!({ "type": "transport_stop" }),
    ] {
        let error = ui_host_message(message.clone()).expect_err("must be refused");
        assert!(error.contains("not accepted"), "{message}: {error}");
    }
}

#[test]
fn malformed_ui_messages_are_refused() {
    assert!(ui_host_message(json!({ "type": "device_input" })).is_err());
}
