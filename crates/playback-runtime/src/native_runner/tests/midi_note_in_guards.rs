use super::device_config_apply::{board_runner, save_setting_payload};
use super::midi_note_in::{audio, keyboard_runner, note_off, send_note, NOTES_IN};
use super::*;
use crate::RuntimePlatformRequest;

fn load_system(runner: &mut NativeRunner, payload: Value) {
    let request = RuntimePlatformRequest::new(
        RuntimePlatformEffect::StoreLoadSystem,
        "load-system".into(),
        None,
    );
    runner.register_platform_request(&request);
    runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadSystemResult {
                payload: Some(payload),
            }
            .with_identity(request.request_id, request.revision),
        })
        .unwrap();
}

#[test]
fn a_key_held_while_a_system_load_turns_notes_in_off_still_releases() {
    let notes_in_off = save_setting_payload(&mut board_runner(false), NOTES_IN, -1, "save");
    let mut runner = keyboard_runner("keys");
    send_note(&mut runner, [0x90, 60, 100]);

    load_system(&mut runner, notes_in_off);
    assert!(!runner.midi.notes_in);
    let messages = send_note(&mut runner, [0x80, 60, 0]);

    assert!(audio(&messages).contains(&note_off(0, 60)), "{messages:?}");
    let messages = send_note(&mut runner, [0x90, 60, 100]);
    assert!(audio(&messages).is_empty(), "new notes stay ignored");
}
