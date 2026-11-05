use super::*;
use std::sync::mpsc::Receiver;
use std::time::Duration;

const FAR: Duration = Duration::from_secs(3600);
const MARKER: [u8; 1] = [0xFE];

fn channel_sender() -> (TimedMidiSender, Receiver<Vec<u8>>) {
    let (tx, rx) = mpsc::channel();
    let mut sender = TimedMidiSender::new("midi-test", || {});
    sender
        .set_sink(Some(Box::new(move |bytes| {
            tx.send(bytes.to_vec()).map_err(|error| error.to_string())
        })))
        .unwrap();
    (sender, rx)
}

fn received_until_marker(rx: &Receiver<Vec<u8>>) -> Vec<Vec<u8>> {
    let mut received = Vec::new();
    loop {
        let bytes = rx.recv_timeout(Duration::from_secs(5)).unwrap();
        if bytes == MARKER {
            return received;
        }
        received.push(bytes);
    }
}

#[test]
fn schedule_releases_messages_in_time_then_arrival_order() {
    let origin = Instant::now();
    let mut schedule = MidiSchedule::default();
    schedule.push(origin + Duration::from_millis(2), vec![2]);
    schedule.push(origin + Duration::from_millis(1), vec![1]);
    schedule.push(origin + Duration::from_millis(2), vec![3]);

    assert_eq!(schedule.pop_due(origin), None);
    assert_eq!(schedule.next_due(), Some(origin + Duration::from_millis(1)));
    let later = origin + Duration::from_millis(2);
    let released: Vec<_> = std::iter::from_fn(|| schedule.pop_due(later)).collect();
    assert_eq!(released, vec![vec![1], vec![2], vec![3]]);
}

#[test]
fn schedule_flush_keeps_only_note_offs_in_time_order() {
    let origin = Instant::now();
    let mut schedule = MidiSchedule::default();
    schedule.push(origin + Duration::from_millis(3), vec![0x80, 62, 0]);
    schedule.push(origin + Duration::from_millis(1), vec![0x90, 64, 100]);
    schedule.push(origin + Duration::from_millis(2), vec![0x91, 60, 0]);
    schedule.push(origin + Duration::from_millis(2), vec![0xF8]);
    schedule.push(origin + Duration::from_millis(2), vec![0xB0, 1, 64]);

    assert_eq!(
        schedule.take_note_offs(),
        vec![vec![0x91, 60, 0], vec![0x80, 62, 0]]
    );
    assert_eq!(schedule.next_due(), None);
}

#[test]
fn untimed_messages_send_immediately_in_order() {
    let (mut sender, rx) = channel_sender();
    sender
        .send(Some(Instant::now() + FAR), &[0x90, 60, 100])
        .unwrap();
    sender.send(None, &[0x90, 61, 100]).unwrap();
    sender.send(None, &[0x80, 61, 0]).unwrap();
    sender.send(None, &MARKER).unwrap();

    assert_eq!(
        received_until_marker(&rx),
        vec![vec![0x90, 61, 100], vec![0x80, 61, 0]]
    );
}

#[test]
fn flush_sends_pending_note_offs_and_drops_other_future_messages() {
    let (mut sender, rx) = channel_sender();
    let later = Instant::now() + FAR;
    sender.send(Some(later), &[0x90, 60, 100]).unwrap();
    sender.send(Some(later), &[0xF8]).unwrap();
    sender.send(Some(later), &[0x80, 59, 0]).unwrap();
    sender.flush().unwrap();
    sender.send(None, &MARKER).unwrap();

    assert_eq!(received_until_marker(&rx), vec![vec![0x80, 59, 0]]);
}

#[test]
fn panic_clears_the_queue_and_sends_panic_messages() {
    let (mut sender, rx) = channel_sender();
    sender
        .send(Some(Instant::now() + FAR), &[0x80, 59, 0])
        .unwrap();
    sender.panic().unwrap();
    sender.flush().unwrap();
    sender.send(None, &MARKER).unwrap();

    assert_eq!(
        received_until_marker(&rx),
        midi_panic_messages().collect::<Vec<_>>()
    );
}

#[test]
fn an_unplugged_output_is_not_an_error() {
    let mut sender = TimedMidiSender::new("midi-test", || {});
    sender
        .set_sink(Some(Box::new(|_| Err("port gone".into()))))
        .unwrap();

    assert_eq!(sender.send(None, &[0x90, 60, 100]), Ok(()));
    assert_eq!(sender.send(None, &[0x80, 60, 0]), Ok(()));
    assert_eq!(sender.panic(), Ok(()));
}

#[test]
fn sender_without_an_output_ignores_messages() {
    let mut sender = TimedMidiSender::new("midi-test", || {});

    assert_eq!(sender.send(None, &[0x90, 60, 100]), Ok(()));
    assert_eq!(sender.panic(), Ok(()));
    assert!(sender.worker.is_none());
}

fn sleeping(sleep: Sleep) -> Shared {
    Shared {
        inbox: Mutex::new(Inbox {
            commands: Vec::new(),
            sleep,
            stopped: false,
        }),
        wake: Condvar::new(),
    }
}

fn woken(shared: &Shared) -> bool {
    matches!(shared.lock().sleep, Sleep::Awake)
}

#[test]
fn a_message_due_no_earlier_than_the_sleep_does_not_wake_the_sender() {
    let deadline = Instant::now() + FAR;
    let shared = sleeping(Sleep::Until(deadline));
    let send = |at| Command::Send {
        at: Some(at),
        bytes: vec![0xF8],
    };

    shared.post(send(deadline), Some(deadline)).unwrap();
    shared
        .post(send(deadline + FAR), Some(deadline + FAR))
        .unwrap();
    assert!(!woken(&shared));
    assert_eq!(shared.lock().commands.len(), 2);

    let earlier = deadline - Duration::from_millis(1);
    shared.post(send(earlier), Some(earlier)).unwrap();
    assert!(woken(&shared));
}

#[test]
fn untimed_messages_and_commands_wake_a_sleeping_sender() {
    let deadline = Instant::now() + FAR;
    for command in [
        Command::Send {
            at: None,
            bytes: vec![0x90, 60, 100],
        },
        Command::Flush,
    ] {
        let shared = sleeping(Sleep::Until(deadline));
        shared.post(command, None).unwrap();
        assert!(woken(&shared));
    }
    let idle = sleeping(Sleep::UntilMessage);
    idle.post(Command::Flush, Some(deadline)).unwrap();
    assert!(woken(&idle));
}

#[test]
fn a_stopped_sender_refuses_messages() {
    let shared = sleeping(Sleep::Awake);
    shared.stop();
    assert_eq!(
        shared.post(Command::Flush, None),
        Err(SENDER_STOPPED.to_string())
    );
}

#[test]
fn timed_messages_behind_the_next_deadline_still_go_out_in_order() {
    let (mut sender, rx) = channel_sender();
    let soon = Instant::now() + Duration::from_millis(2);
    sender.send(Some(soon), &[0x90, 60, 100]).unwrap();
    sender
        .send(Some(soon + Duration::from_millis(1)), &[0x80, 60, 0])
        .unwrap();
    sender
        .send(Some(soon + Duration::from_millis(2)), &MARKER)
        .unwrap();

    assert_eq!(
        received_until_marker(&rx),
        vec![vec![0x90, 60, 100], vec![0x80, 60, 0]]
    );
}
