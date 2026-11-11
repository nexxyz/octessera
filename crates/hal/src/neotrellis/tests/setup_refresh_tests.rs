use super::*;

#[test]
fn the_refresh_sends_one_boards_setup_two_writes_at_a_time() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    let mut parts = Vec::new();
    for part in 0.. {
        lock_state(&states[2]).transactions.clear();
        let more = trellis.reapply_setup_part(2, part).unwrap();
        parts.push(std::mem::take(&mut lock_state(&states[2]).transactions));
        if !more {
            break;
        }
    }

    assert_eq!(parts.len(), 10);
    assert!(parts[..9].iter().all(|part| part.len() == 2));
    assert_eq!(parts[9].len(), 1);
    assert_eq!(parts.concat(), expected_device_init().split_off(2));
    for device in [0, 1, 3] {
        assert_eq!(
            lock_state(&states[device]).transactions,
            expected_device_init()
        );
    }
    assert!(!trellis.take_led_refresh());
}

#[test]
fn a_failed_refresh_part_sets_that_board_up_again_on_the_next_scan() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    {
        let mut state = lock_state(&states[3]);
        state.fail_write_at = Some(state.write_calls + 1);
    }
    assert!(trellis.reapply_setup_part(3, 4).is_err());
    lock_state(&states[3]).transactions.clear();

    trellis.scan_keys(&mut Vec::new()).unwrap();

    let mut expected = expected_device_init().split_off(1);
    expected.push(Transaction::Read {
        base: SEESAW_KEYPAD_BASE,
        function: SEESAW_KEYPAD_COUNT,
        length: 1,
    });
    assert_eq!(lock_state(&states[3]).transactions, expected);
    assert!(trellis.take_led_refresh());
}

#[test]
fn scans_between_refresh_parts_stay_plain_key_reads() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    lock_state(&states[0]).transactions.clear();
    let count_read = Transaction::Read {
        base: SEESAW_KEYPAD_BASE,
        function: SEESAW_KEYPAD_COUNT,
        length: 1,
    };

    let mut part = 0;
    while trellis.reapply_setup_part(0, part).unwrap() {
        trellis.scan_keys(&mut Vec::new()).unwrap();
        part += 1;
    }

    let mut expected = Vec::new();
    for chunk in expected_device_init().split_off(2).chunks(2) {
        expected.extend_from_slice(chunk);
        expected.push(count_read.clone());
    }
    expected.pop();
    assert_eq!(lock_state(&states[0]).transactions, expected);
    assert!(!trellis.take_led_refresh());
}
