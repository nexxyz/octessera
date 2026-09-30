use super::*;
use crate::native_menu::NativeMenuValue;

#[path = "aux_numeric_encoder_tests.rs"]
mod aux;
#[path = "main_numeric_encoder_tests.rs"]
mod main;

fn device_input(runner: &mut NativeRunner, input: Value) {
    runner
        .send(HostMessage::DeviceInput {
            input,
            request_snapshot: None,
        })
        .unwrap();
}

fn button(runner: &mut NativeRunner, kind: &str, pressed: bool) {
    device_input(runner, json!({ "type": kind, "pressed": pressed }));
}

fn turn(runner: &mut NativeRunner, id: &str, delta: i8) {
    device_input(
        runner,
        json!({ "type": "encoder_turn", "id": id, "delta": delta }),
    );
}

fn press(runner: &mut NativeRunner, id: &str) {
    device_input(runner, json!({ "type": "encoder_press", "id": id }));
}

fn runner() -> NativeRunner {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.skip_startup_splash();
    runner
}

fn aux_binding(key: &str) -> NativeAuxBinding {
    NativeAuxBinding {
        turn_key: Some(key.into()),
        press_action: None,
    }
}
