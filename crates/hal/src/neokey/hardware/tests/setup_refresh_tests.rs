use super::*;

fn refresh_parts(neokey: &mut NeoKey, state: &Arc<Mutex<FakeState>>) -> Vec<Vec<Transaction>> {
    let mut parts = Vec::new();
    for part in 0.. {
        lock_state(state).transactions.clear();
        let more = neokey.reapply_setup_part(part).unwrap();
        parts.push(std::mem::take(&mut lock_state(state).transactions));
        if !more {
            break;
        }
    }
    parts
}

#[test]
fn the_refresh_re_sends_the_setup_and_the_shown_colors_two_writes_at_a_time() {
    let (transport, state) = fake_state();
    let mut neokey = NeoKey::from_transport(Box::new(transport)).unwrap();
    neokey.set_led(1, 10, 20, 30).unwrap();
    neokey.set_led(3, 1, 2, 3).unwrap();

    let parts = refresh_parts(&mut neokey, &state);

    assert_eq!(parts.len(), 4);
    assert!(parts.iter().all(|part| part.len() == 2));
    let mut expected = expected_init()[2..8].to_vec();
    expected.extend([
        Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF,
            data: vec![0, 0, 0, 0, 0, 20, 10, 30, 0, 0, 0, 2, 1, 3],
        },
        Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_SHOW,
            data: vec![],
        },
    ]);
    assert_eq!(parts.concat(), expected);
    assert!(!neokey.take_led_refresh());
}

#[test]
fn a_failed_refresh_part_sets_the_board_up_again_on_the_next_scan() {
    let (transport, state) = fake_state();
    let mut neokey = NeoKey::from_transport(Box::new(transport)).unwrap();
    {
        let mut state = lock_state(&state);
        state.fail_write_at = Some(state.write_calls + 2);
    }
    assert!(neokey.reapply_setup_part(1).is_err());
    lock_state(&state).transactions.clear();

    neokey.scan_interrupts().unwrap();

    let mut expected = expected_init().split_off(1);
    expected.extend(interrupt_scan());
    assert_eq!(lock_state(&state).transactions, expected);
    assert!(neokey.take_led_refresh());
}
