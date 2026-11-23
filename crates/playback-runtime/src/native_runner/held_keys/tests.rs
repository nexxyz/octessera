use super::*;

const SYNTH: u8 = 0;
const HOLD: [NoteBehavior; 2] = [NoteBehavior::Hold, NoteBehavior::Hold];
const ONE_SHOT: [NoteBehavior; 2] = [NoteBehavior::Oneshot, NoteBehavior::Oneshot];

fn press(pitch: u8, note: u8, length_ms: Option<u32>) -> KeyEvent {
    KeyEvent::Press(KeyPress {
        slot: SYNTH,
        pitch,
        sound: Sound::Audio {
            channel: SYNTH,
            note,
            velocity: 100,
        },
        length_ms,
        cuts: true,
    })
}

fn release(pitch: u8) -> KeyEvent {
    KeyEvent::Release { slot: SYNTH, pitch }
}

fn play(keys: &mut HeldKeys, events: Vec<KeyEvent>, behaviors: &[NoteBehavior]) -> Vec<String> {
    let mut out = RoutedMusicalEvents::default();
    keys.play(events, behaviors, &mut out);
    out.audio
        .iter()
        .map(|event| match event {
            MusicalEvent::NoteOn { note, .. } => format!("+{note}"),
            MusicalEvent::NoteOff { note, .. } => format!("-{note}"),
            MusicalEvent::Cc { .. } => "c".into(),
        })
        .collect()
}

#[test]
fn a_held_key_struck_again_ends_its_note_before_it_sounds_again() {
    let mut keys = HeldKeys::default();
    assert_eq!(play(&mut keys, vec![press(60, 60, None)], &HOLD), ["+60"]);
    assert_eq!(
        play(&mut keys, vec![press(60, 60, None)], &HOLD),
        ["-60", "+60"]
    );
    assert_eq!(play(&mut keys, vec![release(60)], &HOLD), ["-60"]);
    assert!(play(&mut keys, vec![release(60)], &HOLD).is_empty());
}

#[test]
fn a_one_shot_key_struck_again_only_sounds_again() {
    let mut keys = HeldKeys::default();
    play(&mut keys, vec![press(60, 60, Some(150))], &ONE_SHOT);
    assert_eq!(
        play(&mut keys, vec![press(60, 60, Some(150))], &ONE_SHOT),
        ["+60"]
    );
}

#[test]
fn a_one_shot_key_lets_go_after_its_length_and_a_later_release_sends_nothing() {
    let mut keys = HeldKeys::default();
    play(&mut keys, vec![press(60, 60, Some(150))], &ONE_SHOT);
    keys.advance(6, 120.0);
    assert_eq!(play(&mut keys, vec![release(60)], &ONE_SHOT), ["-60"]);
    play(&mut keys, vec![press(60, 60, Some(150))], &ONE_SHOT);
    keys.advance(8, 120.0);
    assert!(play(&mut keys, vec![release(60)], &ONE_SHOT).is_empty());
}

#[test]
fn a_release_matches_the_key_as_played_whatever_it_sounds_as() {
    let mut keys = HeldKeys::default();
    play(&mut keys, vec![press(60, 62, None)], &HOLD);
    assert!(play(&mut keys, vec![release(62)], &HOLD).is_empty());
    assert_eq!(play(&mut keys, vec![release(60)], &HOLD), ["-62"]);
}

#[test]
fn two_keys_on_one_sound_share_it_until_either_releases() {
    let mut keys = HeldKeys::default();
    play(&mut keys, vec![press(60, 60, None)], &HOLD);
    assert_eq!(
        play(&mut keys, vec![press(58, 60, None)], &HOLD),
        ["-60", "+60"]
    );
    assert_eq!(play(&mut keys, vec![release(58)], &HOLD), ["-60"]);
    assert!(play(&mut keys, vec![release(60)], &HOLD).is_empty());
    assert!(keys.held(SYNTH).is_empty());
}

#[test]
fn a_release_of_all_slots_ends_every_key_once() {
    let mut keys = HeldKeys::default();
    play(
        &mut keys,
        vec![
            press(60, 60, None),
            press(64, 64, None),
            press(58, 60, None),
        ],
        &HOLD,
    );
    assert_eq!(
        play(&mut keys, vec![KeyEvent::ReleaseAll { slots: 1 }], &HOLD),
        ["-60", "-64"]
    );
    assert!(keys.held(SYNTH).is_empty());
}

#[test]
fn a_full_table_lets_go_of_its_first_key_for_a_new_one() {
    let mut keys = HeldKeys::default();
    let presses = (0..KEYS_PER_INSTRUMENT as u8)
        .map(|pitch| press(pitch, pitch, None))
        .collect();
    play(&mut keys, presses, &HOLD);
    assert_eq!(
        play(&mut keys, vec![press(100, 100, None)], &HOLD),
        ["-0", "+100"]
    );
    assert_eq!(keys.held(SYNTH).len(), KEYS_PER_INSTRUMENT);
}
