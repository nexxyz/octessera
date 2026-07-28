use super::*;

#[test]
fn rising_edge_is_press_and_falling_edge_is_release() {
    let seesaw_key = 10;
    assert_eq!(
        decode_trellis_key_event((seesaw_key << 2) | KEYPAD_EDGE_RISING),
        Some((6, true))
    );
    assert_eq!(
        decode_trellis_key_event((seesaw_key << 2) | KEYPAD_EDGE_FALLING),
        Some((6, false))
    );
}

#[test]
fn non_edge_events_are_ignored() {
    assert_eq!(decode_trellis_key_event(0), None);
    assert_eq!(decode_trellis_key_event(1), None);
}

#[test]
fn preserves_lower_left_coordinates_and_grb_output() {
    assert_eq!(trellis_coordinate(0, 0), Some((0, 7)));
    assert_eq!(trellis_coordinate(1, 0), Some((4, 7)));
    assert_eq!(trellis_coordinate(2, 0), Some((0, 3)));
    assert_eq!(trellis_coordinate(3, 15), Some((7, 0)));
    assert_eq!(trellis_coordinate(0, 16), None);
    assert_eq!(grb_color([0x12, 0x34, 0x56]), [0x34, 0x12, 0x56]);
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
#[test]
fn initialization_plan_matches_polling_and_interrupt_transport_contracts() {
    let polling = trellis_init_plan(SeesawInputMode::Polling);
    let interrupt = trellis_init_plan(SeesawInputMode::Interrupt);
    let has_write = |plan: &[TrellisInitCommand], function: u8| {
        plan.iter().any(|command| {
            matches!(
                command,
                TrellisInitCommand::Write {
                    base: SEESAW_KEYPAD_BASE,
                    function: command_function,
                    ..
                } if *command_function == function
            )
        })
    };

    assert!(!has_write(&polling, SEESAW_KEYPAD_INTENSET));
    assert!(has_write(&interrupt, SEESAW_KEYPAD_INTENSET));
    assert_eq!(
        polling
            .iter()
            .filter(|command| matches!(
                command,
                TrellisInitCommand::Write {
                    base: SEESAW_KEYPAD_BASE,
                    function: SEESAW_KEYPAD_EVENT,
                    ..
                }
            ))
            .count(),
        32
    );
    assert_eq!(polling, trellis_init_plan(SeesawInputMode::Polling));
    assert_eq!(
        trellis_reset_command(),
        TrellisInitCommand::Write {
            base: SEESAW_STATUS_BASE,
            function: SEESAW_SW_RESET,
            data: vec![0xFF],
        }
    );
}
