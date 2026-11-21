use super::support::{set_runtime_playing, FakeHost};
use crate::{
    CoreRunner, HostMessage, MidiNote, MusicalEvent, PlaybackRuntime, RunnerMessage, RuntimeConfig,
    SyncSource,
};
use std::time::{Duration, Instant};

pub(crate) fn midi_out_runtime() -> PlaybackRuntime {
    PlaybackRuntime::new(RuntimeConfig {
        bpm: 120.0,
        sync_source: SyncSource::Internal,
        midi_clock_out_enabled: false,
        midi_out_enabled: true,
        midi_out_internal_channels: u16::MAX,
    })
}

pub(crate) fn dispatch_note(
    runtime: &mut PlaybackRuntime,
    runner: &mut impl CoreRunner,
    host: &mut FakeHost,
    bytes: [u8; 3],
) {
    let message: HostMessage = MidiNote::parse(&bytes).unwrap().into();
    runtime
        .dispatch_host_message(message, runner, host)
        .unwrap();
}

/// A sequencer stand-in: every pulse sends one MIDI note; it keeps the keyboard notes it gets.
#[derive(Default)]
struct SequencerRunner {
    keyboard: Vec<HostMessage>,
}

impl CoreRunner for SequencerRunner {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        if matches!(message, HostMessage::MidiNote { .. }) {
            self.keyboard.push(message);
            return Ok(Vec::new());
        }
        Ok(vec![RunnerMessage::MidiEvents {
            events: vec![MusicalEvent::NoteOn {
                channel: 2,
                note: 64,
                velocity: 90,
                duration_ms: Some(30),
            }],
        }])
    }

    fn send_system_store_result(
        &mut self,
        _message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<crate::RuntimeStoreResult>), String> {
        Ok((Vec::new(), None))
    }
}

#[test]
fn a_sequenced_note_echoed_back_is_dropped_but_other_notes_reach_the_runner() {
    let mut runtime = midi_out_runtime();
    let mut runner = SequencerRunner::default();
    let start = Instant::now();
    let mut host = FakeHost {
        clock: Some(start),
        ..FakeHost::default()
    };
    set_runtime_playing(&mut runtime, &mut host);
    host.midi_message_at.clear();
    runtime.advance(100, &mut runner, &mut host).unwrap();
    let sent = host
        .midi_messages
        .iter()
        .position(|bytes| bytes == &vec![0x92, 64, 90])
        .expect("the sequencer note goes out");
    let sent_at = host.midi_message_at[sent].expect("a sequenced note is stamped");
    runner.keyboard.clear();

    host.clock = Some(sent_at - Duration::from_millis(1));
    dispatch_note(&mut runtime, &mut runner, &mut host, [0x92, 64, 90]);
    assert_eq!(
        runner.keyboard.len(),
        1,
        "not on the wire yet, so not an echo"
    );

    host.clock = Some(sent_at + Duration::from_millis(5));
    dispatch_note(&mut runtime, &mut runner, &mut host, [0x92, 64, 90]);
    assert_eq!(runner.keyboard.len(), 1, "its echo is dropped");

    dispatch_note(&mut runtime, &mut runner, &mut host, [0x90, 60, 100]);
    assert_eq!(runner.keyboard.len(), 2, "an unrelated keyboard note plays");

    let last_sent_at = host
        .midi_messages
        .iter()
        .zip(&host.midi_message_at)
        .filter(|(bytes, _)| *bytes == &vec![0x92, 64, 90])
        .filter_map(|(_, at)| *at)
        .max()
        .unwrap();
    host.clock = Some(last_sent_at + Duration::from_millis(151));
    dispatch_note(&mut runtime, &mut runner, &mut host, [0x92, 64, 90]);
    assert_eq!(
        runner.keyboard.len(),
        3,
        "after the window it is a played note"
    );
}

#[test]
fn a_key_up_right_after_a_sequenced_note_off_on_the_same_note_reaches_the_runner() {
    let mut runtime = midi_out_runtime();
    let mut runner = SequencerRunner::default();
    let start = Instant::now();
    let mut host = FakeHost {
        clock: Some(start),
        ..FakeHost::default()
    };
    set_runtime_playing(&mut runtime, &mut host);
    host.midi_message_at.clear();
    runtime.advance(100, &mut runner, &mut host).unwrap();
    let off_at = host
        .midi_messages
        .iter()
        .zip(&host.midi_message_at)
        .find(|(bytes, _)| *bytes == &vec![0x82, 64, 0])
        .and_then(|(_, at)| *at)
        .expect("the sequenced note-off goes out");
    runner.keyboard.clear();

    host.clock = Some(off_at + Duration::from_millis(5));
    dispatch_note(&mut runtime, &mut runner, &mut host, [0x82, 64, 0]);
    assert_eq!(
        runner.keyboard,
        vec![HostMessage::MidiNote {
            channel: 2,
            note: 64,
            velocity: 0,
        }],
        "a note-off is never an echo, so the key-up releases the held note"
    );
}

#[test]
fn a_key_up_forgets_the_sent_note_so_the_next_identical_press_plays() {
    let mut runtime = midi_out_runtime();
    let mut runner = SequencerRunner::default();
    let start = Instant::now();
    let mut host = FakeHost {
        clock: Some(start),
        ..FakeHost::default()
    };
    set_runtime_playing(&mut runtime, &mut host);
    host.midi_message_at.clear();
    runtime.advance(100, &mut runner, &mut host).unwrap();
    let sent = host
        .midi_messages
        .iter()
        .position(|bytes| bytes == &vec![0x92, 64, 90])
        .expect("the sequencer note goes out");
    let sent_at = host.midi_message_at[sent].expect("a sequenced note is stamped");
    runner.keyboard.clear();

    host.clock = Some(sent_at + Duration::from_millis(2));
    dispatch_note(&mut runtime, &mut runner, &mut host, [0x82, 64, 0]);
    host.clock = Some(sent_at + Duration::from_millis(10));
    dispatch_note(&mut runtime, &mut runner, &mut host, [0x92, 64, 90]);

    assert_eq!(
        runner.keyboard.len(),
        2,
        "no echo came before the key-up, so the next press of that key is a played note"
    );
}
