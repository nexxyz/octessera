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
fn panic_reports_sink_failures() {
    let mut sender = TimedMidiSender::new("midi-test", || {});
    sender
        .set_sink(Some(Box::new(|_| Err("port gone".into()))))
        .unwrap();

    assert_eq!(sender.panic(), Err("port gone".into()));
}

#[test]
fn sender_without_an_output_ignores_messages() {
    let mut sender = TimedMidiSender::new("midi-test", || {});

    assert_eq!(sender.send(None, &[0x90, 60, 100]), Ok(()));
    assert_eq!(sender.panic(), Ok(()));
    assert!(sender.worker.is_none());
}
