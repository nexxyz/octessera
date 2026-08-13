use super::*;
use std::sync::{Arc, Mutex, MutexGuard};

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
        debouncer.update([true, false, false, false], start + NEOKEY_DEBOUNCE,),
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum Transaction {
    Write {
        base: u8,
        function: u8,
        data: Vec<u8>,
    },
    Read {
        base: u8,
        function: u8,
        length: usize,
    },
}

#[derive(Default)]
struct FakeState {
    transactions: Vec<Transaction>,
    hw_id: u8,
    raw_state: u32,
    fail_write_at: Option<usize>,
    fail_read_at: Option<usize>,
    fail_reads: bool,
    write_calls: usize,
    read_calls: usize,
}

struct FakeTransport {
    state: Arc<Mutex<FakeState>>,
}

fn lock_state(state: &Arc<Mutex<FakeState>>) -> MutexGuard<'_, FakeState> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl SeesawTransport for FakeTransport {
    fn write(&mut self, base: u8, function: u8, data: &[u8]) -> Result<(), String> {
        let mut state = lock_state(&self.state);
        state.write_calls += 1;
        state.transactions.push(Transaction::Write {
            base,
            function,
            data: data.to_vec(),
        });
        if state.fail_write_at == Some(state.write_calls) {
            return Err("fake write failure".into());
        }
        Ok(())
    }

    fn read(&mut self, base: u8, function: u8, buffer: &mut [u8]) -> Result<(), String> {
        let mut state = lock_state(&self.state);
        state.read_calls += 1;
        state.transactions.push(Transaction::Read {
            base,
            function,
            length: buffer.len(),
        });
        if state.fail_reads || state.fail_read_at == Some(state.read_calls) {
            return Err("fake read failure".into());
        }
        let response = match (base, function) {
            (SEESAW_STATUS_BASE, SEESAW_HW_ID) => vec![state.hw_id],
            (SEESAW_GPIO_BASE, SEESAW_GPIO_BULK) => state.raw_state.to_be_bytes().to_vec(),
            _ => vec![0; buffer.len()],
        };
        if response.len() != buffer.len() {
            return Err("fake response length mismatch".into());
        }
        buffer.copy_from_slice(&response);
        Ok(())
    }
}

fn fake_state() -> (FakeTransport, Arc<Mutex<FakeState>>) {
    let state = Arc::new(Mutex::new(FakeState {
        hw_id: 0x84,
        ..FakeState::default()
    }));
    (
        FakeTransport {
            state: Arc::clone(&state),
        },
        state,
    )
}

fn expected_init(mode: SeesawInputMode) -> Vec<Transaction> {
    let mask = NEOKEY_BUTTON_MASK.to_be_bytes().to_vec();
    let mut transactions = vec![
        Transaction::Write {
            base: SEESAW_STATUS_BASE,
            function: SEESAW_SW_RESET,
            data: vec![0xFF],
        },
        Transaction::Read {
            base: SEESAW_STATUS_BASE,
            function: SEESAW_HW_ID,
            length: 1,
        },
        Transaction::Write {
            base: SEESAW_GPIO_BASE,
            function: SEESAW_GPIO_DIRCLR_BULK,
            data: mask.clone(),
        },
        Transaction::Write {
            base: SEESAW_GPIO_BASE,
            function: SEESAW_GPIO_PULLENSET,
            data: mask.clone(),
        },
        Transaction::Write {
            base: SEESAW_GPIO_BASE,
            function: SEESAW_GPIO_BULK_SET,
            data: mask.clone(),
        },
    ];
    if mode == SeesawInputMode::Interrupt {
        transactions.extend([
            Transaction::Write {
                base: SEESAW_GPIO_BASE,
                function: SEESAW_GPIO_INTENSET,
                data: mask,
            },
            Transaction::Read {
                base: SEESAW_GPIO_BASE,
                function: SEESAW_GPIO_INTFLAG,
                length: 4,
            },
        ]);
    }
    transactions.extend([
        Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_PIN,
            data: vec![NEOKEY_NEOPIXEL_PIN],
        },
        Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF_LENGTH,
            data: NEOKEY_LED_BYTES.to_be_bytes().to_vec(),
        },
    ]);
    transactions
}

#[test]
fn initializes_in_exact_polling_order() {
    let (transport, state) = fake_state();
    NeoKey::from_transport(Box::new(transport), SeesawInputMode::Polling).unwrap();
    assert_eq!(
        lock_state(&state).transactions,
        expected_init(SeesawInputMode::Polling)
    );
}

#[test]
fn interrupt_initialization_adds_enable_and_clear_transactions() {
    let (transport, state) = fake_state();
    NeoKey::from_transport(Box::new(transport), SeesawInputMode::Interrupt).unwrap();
    assert_eq!(
        lock_state(&state).transactions,
        expected_init(SeesawInputMode::Interrupt)
    );
}

#[test]
fn validates_hw_id() {
    let (transport, state) = fake_state();
    lock_state(&state).hw_id = 0x12;
    let error = NeoKey::from_transport(Box::new(transport), SeesawInputMode::Polling)
        .err()
        .expect("invalid HW ID should fail");
    assert!(error.contains("NeoKey HW ID invalid: 0x12"));
}

#[test]
fn maps_active_low_button_bits() {
    assert_eq!(
        neokey_buttons_from_raw(0x000000A0),
        [true, false, true, false]
    );
}

#[test]
fn retains_injected_transport_for_scan_and_led_transactions() {
    let (transport, state) = fake_state();
    let mut neokey = NeoKey::from_transport(Box::new(transport), SeesawInputMode::Polling).unwrap();
    lock_state(&state).transactions.clear();

    assert_eq!(neokey.raw_button_state().unwrap(), 0);
    neokey.set_led(2, 0x11, 0x22, 0x33).unwrap();

    assert_eq!(
        lock_state(&state).transactions,
        [
            Transaction::Read {
                base: SEESAW_GPIO_BASE,
                function: SEESAW_GPIO_BULK,
                length: 4,
            },
            Transaction::Write {
                base: SEESAW_NEOPIXEL_BASE,
                function: SEESAW_NEOPIXEL_BUF,
                data: vec![0, 6, 0x22, 0x11, 0x33],
            },
            Transaction::Write {
                base: SEESAW_NEOPIXEL_BASE,
                function: SEESAW_NEOPIXEL_SHOW,
                data: Vec::new(),
            },
        ]
    );
}

#[test]
fn retries_a_transient_write_failure_without_sleeping() {
    let (transport, state) = fake_state();
    lock_state(&state).fail_write_at = Some(1);

    NeoKey::from_transport(Box::new(transport), SeesawInputMode::Polling).unwrap();

    let state = lock_state(&state);
    let transactions = &state.transactions;
    assert_eq!(
        transactions
            .iter()
            .filter(|transaction| matches!(
                transaction,
                Transaction::Write {
                    base: SEESAW_STATUS_BASE,
                    function: SEESAW_SW_RESET,
                    ..
                }
            ))
            .count(),
        2
    );
}

#[test]
fn retries_a_transient_read_failure_without_sleeping() {
    let (transport, state) = fake_state();
    lock_state(&state).fail_read_at = Some(1);

    NeoKey::from_transport(Box::new(transport), SeesawInputMode::Polling).unwrap();

    assert_eq!(lock_state(&state).read_calls, 2);
}

#[test]
fn propagates_a_persistent_read_failure_after_retries() {
    let (transport, state) = fake_state();
    lock_state(&state).fail_reads = true;
    let error = NeoKey::from_transport(Box::new(transport), SeesawInputMode::Polling)
        .err()
        .expect("read failure should fail initialization");

    assert!(error.contains("NeoKey HW ID read failed: fake read failure"));
    assert_eq!(lock_state(&state).read_calls, 3);
}
