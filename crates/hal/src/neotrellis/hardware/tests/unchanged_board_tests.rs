use super::*;

fn trellis_with_cleared_log() -> (NeoTrellis, FakeStates) {
    let (transports, states) = fake_transports();
    let trellis = NeoTrellis::from_transport([0x2E, 0x2F, 0x30, 0x31], transports).unwrap();
    clear(&states);
    (trellis, states)
}

fn clear(states: &FakeStates) {
    for state in states {
        lock_state(state).transactions.clear();
    }
}

fn led_writes(states: &FakeStates) -> [usize; 4] {
    std::array::from_fn(|index| lock_state(&states[index]).transactions.len())
}

fn fail_next_write(state: &Arc<Mutex<FakeState>>) {
    let mut state = lock_state(state);
    state.fail_write_at = Some(state.write_calls + 1);
}

#[test]
fn the_first_frame_latches_every_board_even_when_it_is_black() {
    let (mut trellis, states) = trellis_with_cleared_log();

    trellis.write_led_frame(&[[0; 3]; 64]).unwrap();

    assert_eq!(led_writes(&states), [3; 4]);
}

#[test]
fn an_unchanged_frame_sends_nothing() {
    let (mut trellis, states) = trellis_with_cleared_log();
    let frame = [[9, 8, 7]; 64];
    trellis.write_led_frame(&frame).unwrap();
    clear(&states);

    trellis.write_led_frame(&frame).unwrap();

    assert_eq!(led_writes(&states), [0; 4]);
}

#[test]
fn one_changed_tile_rewrites_and_latches_only_its_board() {
    let (mut trellis, states) = trellis_with_cleared_log();
    let mut frame = [[0; 3]; 64];
    trellis.write_led_frame(&frame).unwrap();
    clear(&states);

    frame[7 * 8 + 7] = [255, 0, 0];
    trellis.write_led_frame(&frame).unwrap();

    let changed = (0..4)
        .find(|&index| led_writes(&states)[index] > 0)
        .unwrap();
    let mut expected = [0; 4];
    expected[changed] = 3;
    assert_eq!(led_writes(&states), expected);
    assert!(matches!(
        lock_state(&states[changed]).transactions.last(),
        Some(Transaction::Write {
            function: SEESAW_NEOPIXEL_SHOW,
            ..
        })
    ));
}

#[test]
fn a_board_whose_latch_failed_is_rewritten_with_the_same_frame() {
    let (mut trellis, states) = trellis_with_cleared_log();
    let frame = [[1, 2, 3]; 64];
    {
        let mut state = lock_state(&states[1]);
        state.fail_write_at = Some(state.write_calls + 3);
    }
    assert!(trellis.write_led_frame(&frame).is_err());
    clear(&states);

    trellis.scan_keys(&mut Vec::new()).unwrap();
    clear(&states);
    trellis.write_led_frame(&frame).unwrap();

    let writes = led_writes(&states);
    assert_eq!(writes[0], 0);
    assert_eq!(writes[1], 3);
    assert_eq!(writes[2], 3);
    assert_eq!(writes[3], 3);
}

#[test]
fn a_board_that_got_its_setup_back_is_rewritten_with_the_same_frame() {
    let (mut trellis, states) = trellis_with_cleared_log();
    let frame = [[4, 5, 6]; 64];
    trellis.write_led_frame(&frame).unwrap();
    lock_state(&states[3]).fail_reads = true;
    assert!(trellis.scan_keys(&mut Vec::new()).is_err());
    lock_state(&states[3]).fail_reads = false;
    trellis.scan_keys(&mut Vec::new()).unwrap();
    assert!(trellis.take_led_refresh());
    clear(&states);

    trellis.write_led_frame(&frame).unwrap();

    assert_eq!(led_writes(&states), [0, 0, 0, 3]);
}

#[test]
fn a_failed_buffer_write_retries_that_board_in_full() {
    let (mut trellis, states) = trellis_with_cleared_log();
    trellis.write_led_frame(&[[0; 3]; 64]).unwrap();
    let frame = [[7, 7, 7]; 64];
    fail_next_write(&states[0]);
    assert!(trellis.write_led_frame(&frame).is_err());
    trellis.scan_keys(&mut Vec::new()).unwrap();
    clear(&states);

    trellis.write_led_frame(&frame).unwrap();

    assert_eq!(led_writes(&states), [3; 4]);
}
