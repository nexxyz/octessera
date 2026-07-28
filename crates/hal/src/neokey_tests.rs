use super::*;

#[test]
fn suppresses_short_press_pulse() {
    let mut debouncer = NeoKeyDebouncer::default();
    let start = Instant::now();

    assert_eq!(debouncer.update([false; 4], start), [false; 4]);
    assert_eq!(
        debouncer.update(
            [false, false, false, true],
            start + Duration::from_millis(4)
        ),
        [false; 4]
    );
    assert_eq!(
        debouncer.update([false; 4], start + Duration::from_millis(25)),
        [false; 4]
    );
}

#[test]
fn accepts_press_after_debounce_window() {
    let mut debouncer = NeoKeyDebouncer::default();
    let start = Instant::now();

    assert_eq!(
        debouncer.update([true, false, false, false], start),
        [false; 4]
    );
    assert_eq!(
        debouncer.update(
            [true, false, false, false],
            start + NEOKEY_DEBOUNCE - Duration::from_millis(1),
        ),
        [false; 4]
    );
    assert_eq!(
        debouncer.update([true, false, false, false], start + NEOKEY_DEBOUNCE),
        [true, false, false, false]
    );
}

#[test]
fn debounces_release_too() {
    let mut debouncer = NeoKeyDebouncer::default();
    let start = Instant::now();

    debouncer.update([false, true, false, false], start);
    assert_eq!(
        debouncer.update([false, true, false, false], start + NEOKEY_DEBOUNCE),
        [false, true, false, false]
    );
    assert_eq!(
        debouncer.update(
            [false, false, false, false],
            start + NEOKEY_DEBOUNCE + Duration::from_millis(10),
        ),
        [false, true, false, false]
    );
    assert_eq!(
        debouncer.update(
            [false, false, false, false],
            start + NEOKEY_DEBOUNCE + Duration::from_millis(40),
        ),
        [false; 4]
    );
}

#[test]
fn chatter_resets_debounce_window() {
    let mut debouncer = NeoKeyDebouncer::default();
    let start = Instant::now();

    debouncer.update([true, false, false, false], start);
    debouncer.update([false; 4], start + Duration::from_millis(10));
    assert_eq!(
        debouncer.update(
            [true, false, false, false],
            start + Duration::from_millis(20)
        ),
        [false; 4]
    );
    assert_eq!(
        debouncer.update(
            [true, false, false, false],
            start + Duration::from_millis(43)
        ),
        [false; 4]
    );
    assert_eq!(
        debouncer.update(
            [true, false, false, false],
            start + Duration::from_millis(44)
        ),
        [true, false, false, false]
    );
}

#[test]
fn buttons_debounce_independently() {
    let mut debouncer = NeoKeyDebouncer::default();
    let start = Instant::now();

    debouncer.update([true, false, false, false], start);
    debouncer.update(
        [true, true, false, false],
        start + Duration::from_millis(10),
    );

    assert_eq!(
        debouncer.update([true, true, false, false], start + NEOKEY_DEBOUNCE),
        [true, false, false, false]
    );
    assert_eq!(
        debouncer.update(
            [true, true, false, false],
            start + Duration::from_millis(10) + NEOKEY_DEBOUNCE,
        ),
        [true, true, false, false]
    );
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
#[test]
fn initialization_plan_matches_polling_and_interrupt_transport_contracts() {
    let polling = neokey_init_plan(SeesawInputMode::Polling);
    let interrupt = neokey_init_plan(SeesawInputMode::Interrupt);
    let has_write = |plan: &[NeoKeyInitCommand], base: u8, function: u8| {
        plan.iter().any(|command| {
            matches!(
                command,
                NeoKeyInitCommand::Write {
                    base: command_base,
                    function: command_function,
                } if *command_base == base && *command_function == function
            )
        })
    };
    let has_read = |plan: &[NeoKeyInitCommand], base: u8, function: u8| {
        plan.iter().any(|command| {
            matches!(
                command,
                NeoKeyInitCommand::Read {
                    base: command_base,
                    function: command_function,
                    ..
                } if *command_base == base && *command_function == function
            )
        })
    };

    assert!(!has_write(&polling, SEESAW_GPIO_BASE, SEESAW_GPIO_INTENSET));
    assert!(!has_read(&polling, SEESAW_GPIO_BASE, SEESAW_GPIO_INTFLAG));
    assert!(has_write(
        &interrupt,
        SEESAW_GPIO_BASE,
        SEESAW_GPIO_INTENSET
    ));
    assert!(has_read(&interrupt, SEESAW_GPIO_BASE, SEESAW_GPIO_INTFLAG));
}
