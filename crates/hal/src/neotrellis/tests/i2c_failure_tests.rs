use super::*;

#[test]
fn retries_a_transient_write_failure_without_sleeping() {
    let (transports, states) = fake_transports();
    lock_state(&states[0]).fail_write_at = Some(1);
    NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();

    let reset_count = lock_state(&states[0])
        .transactions
        .iter()
        .filter(|transaction| {
            matches!(
                transaction,
                Transaction::Write {
                    base: SEESAW_STATUS_BASE,
                    function: SEESAW_SW_RESET,
                    ..
                }
            )
        })
        .count();
    assert_eq!(reset_count, 2);
}

#[test]
fn propagates_a_persistent_read_failure_after_retries() {
    let (transports, states) = fake_transports();
    lock_state(&states[1]).fail_reads = true;
    let error = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports)
        .err()
        .expect("read failure should fail initialization");

    assert!(error.contains("Trellis HW ID read failed: fake read failure"));
    assert_eq!(lock_state(&states[1]).read_calls, 3);
}

#[test]
fn a_failed_board_keeps_the_events_already_read_from_earlier_boards() {
    let (transports, states) = fake_transports();
    {
        let mut state = lock_state(&states[0]);
        state.scan_count = 2;
        state.fifo = vec![KEYPAD_EDGE_RISING, (1 << 2) | KEYPAD_EDGE_FALLING];
    }
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    lock_state(&states[2]).fail_reads = true;

    let mut events = Vec::new();
    let error = trellis.scan_keys(&mut events).unwrap_err();

    assert!(error.contains("Trellis scan count failed: fake read failure"));
    assert_eq!(events, vec![(0, 7, true), (1, 7, false)]);
    assert_eq!(lock_state(&states[3]).read_calls, 1);
}

fn expected_device_setup_then_count_read() -> Vec<Transaction> {
    let mut transactions = expected_device_init().split_off(1);
    transactions.push(Transaction::Read {
        base: SEESAW_KEYPAD_BASE,
        function: SEESAW_KEYPAD_COUNT,
        length: 1,
    });
    transactions
}

#[test]
fn a_board_that_answers_again_after_a_failed_scan_gets_its_setup_back() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    lock_state(&states[1]).fail_reads = true;
    assert!(trellis.scan_keys(&mut Vec::new()).is_err());
    assert!(!trellis.take_led_refresh());
    lock_state(&states[1]).fail_reads = false;
    for state in &states {
        lock_state(state).transactions.clear();
    }

    trellis.scan_keys(&mut Vec::new()).unwrap();

    assert_eq!(
        lock_state(&states[1]).transactions,
        expected_device_setup_then_count_read()
    );
    for device in [0, 2, 3] {
        assert_eq!(lock_state(&states[device]).transactions.len(), 1);
    }
    assert!(trellis.take_led_refresh());
    assert!(!trellis.take_led_refresh());
    lock_state(&states[1]).transactions.clear();
    trellis.scan_keys(&mut Vec::new()).unwrap();
    assert_eq!(lock_state(&states[1]).transactions.len(), 1);
}

#[test]
fn a_failed_led_write_sets_that_board_up_again_on_the_next_scan() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    {
        let mut state = lock_state(&states[2]);
        state.fail_write_at = Some(state.write_calls + 1);
    }
    assert!(trellis.write_led_frame(&[[0; 3]; 64]).is_err());
    for state in &states {
        lock_state(state).transactions.clear();
    }

    trellis.scan_keys(&mut Vec::new()).unwrap();

    assert_eq!(
        lock_state(&states[2]).transactions,
        expected_device_setup_then_count_read()
    );
    assert_eq!(lock_state(&states[0]).transactions.len(), 1);
    assert!(trellis.take_led_refresh());
}
