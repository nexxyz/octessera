use super::*;

/// The board's pixels go out again after its setup, one 24-byte chunk per part, then a latch.
fn expected_led_resend(grb: &[u8]) -> Vec<Vec<Transaction>> {
    vec![
        vec![Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF,
            data: [&[0, 0][..], &grb[..24]].concat(),
        }],
        vec![Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF,
            data: [&[0, 24][..], &grb[24..]].concat(),
        }],
        vec![Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_SHOW,
            data: Vec::new(),
        }],
    ]
}

fn refresh_parts(
    trellis: &mut NeoTrellis,
    state: &Arc<Mutex<FakeState>>,
    device: usize,
) -> Vec<Vec<Transaction>> {
    let mut parts = Vec::new();
    for part in 0.. {
        lock_state(state).transactions.clear();
        let more = trellis.reapply_setup_part(device, part).unwrap();
        parts.push(std::mem::take(&mut lock_state(state).transactions));
        if !more {
            break;
        }
    }
    parts
}

#[test]
fn the_refresh_sends_one_boards_setup_two_writes_at_a_time_then_its_pixels() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();

    let parts = refresh_parts(&mut trellis, &states[2], 2);

    assert_eq!(parts.len(), 13);
    assert!(parts[..9].iter().all(|part| part.len() == 2));
    assert_eq!(parts[9].len(), 1);
    assert_eq!(parts[..10].concat(), expected_device_init().split_off(2));
    assert_eq!(parts[10..], expected_led_resend(&[0; 48]));
    for device in [0, 1, 3] {
        assert_eq!(
            lock_state(&states[device]).transactions,
            expected_device_init()
        );
    }
    assert!(!trellis.take_led_refresh());
}

#[test]
fn the_refresh_resends_the_last_frame_of_that_board_only() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    let frame = std::array::from_fn(|index| [index as u8, 100 + index as u8, 200]);
    trellis.write_led_frame(&frame).unwrap();
    for state in &states {
        lock_state(state).transactions.clear();
    }

    let parts = refresh_parts(&mut trellis, &states[1], 1);

    let mut grb = Vec::new();
    for y in 0..4 {
        for x in 4..8 {
            grb.extend_from_slice(&grb_color(frame[y * 8 + x]));
        }
    }
    assert_eq!(parts[10..], expected_led_resend(&grb));
    for device in [0, 2, 3] {
        assert!(lock_state(&states[device]).transactions.is_empty());
    }
}

#[test]
fn a_failed_pixel_part_sets_that_board_up_again_on_the_next_scan() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    {
        let mut state = lock_state(&states[0]);
        state.fail_write_at = Some(state.write_calls + 1);
    }

    assert!(trellis.reapply_setup_part(0, 11).is_err());
    trellis.scan_keys(&mut Vec::new()).unwrap();

    assert!(trellis.take_led_refresh());
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
    let setup = expected_device_init().split_off(2);
    for part in setup
        .chunks(2)
        .map(<[_]>::to_vec)
        .chain(expected_led_resend(&[0; 48]))
    {
        expected.extend(part);
        expected.push(count_read.clone());
    }
    expected.pop();
    assert_eq!(lock_state(&states[0]).transactions, expected);
    assert!(!trellis.take_led_refresh());
}
