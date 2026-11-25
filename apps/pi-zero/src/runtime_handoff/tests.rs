use super::*;

#[test]
fn items_arrive_in_send_order_from_every_sender() {
    let (tx, rx) = channel();
    let other = tx.clone();
    tx.send(1).unwrap();
    other.send(2).unwrap();
    tx.send(3).unwrap();
    assert_eq!(rx.try_iter().collect::<Vec<_>>(), vec![1, 2, 3]);
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
}

#[test]
fn try_recv_reports_empty_while_a_sender_holds_the_queue() {
    let (tx, rx) = channel();
    tx.send(1).unwrap();
    let held = tx.shared.lock();
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    drop(held);
    assert_eq!(rx.try_recv(), Ok(1));
}

fn wait_until_senders_block<T>(tx: &Sender<T>, count: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while tx.shared.lock().waiting_senders < count {
        assert!(Instant::now() < deadline, "senders did not block");
        std::thread::yield_now();
    }
}

#[test]
fn bounded_try_send_reports_full_and_send_waits_for_room() {
    let (tx, rx) = bounded(1);
    tx.try_send(1).unwrap();
    assert_eq!(tx.try_send(2), Err(TrySendError::Full(2)));
    let blocked = tx.clone();
    let sender = std::thread::spawn(move || blocked.send(2));
    wait_until_senders_block(&tx, 1);
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(1));
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(2));
    assert!(sender.join().unwrap().is_ok());
}

#[test]
fn every_blocked_sender_wakes_as_room_frees() {
    let (tx, rx) = bounded(2);
    tx.send(1).unwrap();
    tx.send(2).unwrap();
    let senders: Vec<_> = [3, 4]
        .map(|value| {
            let blocked = tx.clone();
            std::thread::spawn(move || blocked.send(value))
        })
        .into_iter()
        .collect();
    wait_until_senders_block(&tx, 2);
    let mut state = rx.shared.lock();
    assert_eq!(rx.shared.take(&mut state), Some(1));
    assert_eq!(rx.shared.take(&mut state), Some(2));
    drop(state);
    let mut received: Vec<_> = (0..2)
        .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .collect();
    received.sort_unstable();
    assert_eq!(received, vec![3, 4]);
    for sender in senders {
        assert!(sender.join().unwrap().is_ok());
    }
}

#[test]
fn recv_timeout_times_out_and_wakes_for_a_later_send() {
    let (tx, rx) = channel();
    assert_eq!(
        rx.recv_timeout(Duration::from_millis(10)),
        Err(RecvTimeoutError::Timeout)
    );
    let sender = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(10));
        tx.send(7).unwrap();
    });
    assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(7));
    sender.join().unwrap();
}

#[test]
fn dropping_every_sender_disconnects_after_the_queued_items() {
    let (tx, rx) = channel();
    let other = tx.clone();
    tx.send(1).unwrap();
    drop(tx);
    assert_eq!(rx.try_recv(), Ok(1));
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    let waiter = std::thread::spawn(move || rx.recv());
    drop(other);
    assert_eq!(waiter.join().unwrap(), Err(RecvError));
}

#[test]
fn dropping_the_receiver_disconnects_senders_and_frees_a_waiting_send() {
    let (tx, rx) = bounded(1);
    tx.send(1).unwrap();
    let blocked = tx.clone();
    let sender = std::thread::spawn(move || blocked.send(2));
    wait_until_senders_block(&tx, 1);
    drop(rx);
    assert_eq!(sender.join().unwrap(), Err(SendError(2)));
    assert_eq!(tx.try_send(3), Err(TrySendError::Disconnected(3)));
    assert_eq!(tx.send(4), Err(SendError(4)));
}
