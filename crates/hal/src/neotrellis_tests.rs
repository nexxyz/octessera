use super::*;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard};

fn grid_projection_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../../../resources/grid-projection-v1.json")).unwrap()
}

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

#[test]
fn exhaustive_fixture_covers_all_four_trellis_devices_and_keys() {
    let fixture = grid_projection_fixture();
    let cells = fixture["cells"].as_array().unwrap();
    assert_eq!(fixture["version"], 1);
    assert_eq!(fixture["width"], 8);
    assert_eq!(fixture["height"], 8);
    assert_eq!(cells.len(), 64);

    let addresses = ["0x2e", "0x2f", "0x30", "0x31"];
    let mut mappings = HashSet::new();
    for cell in cells {
        let logical = &cell["logical"];
        let trellis = &cell["neotrellis"];
        let device = trellis["device_index"].as_u64().unwrap() as usize;
        let key = trellis["key"].as_u64().unwrap() as u8;
        assert!(logical["x"].as_u64().unwrap() < 8);
        assert!(logical["y"].as_u64().unwrap() < 8);
        assert!(logical["index"].as_u64().unwrap() < 64);
        assert_eq!(trellis["address"].as_str(), Some(addresses[device]));
        assert_eq!(
            trellis_key_to_seesaw_key(key),
            trellis["seesaw_key"].as_u64().unwrap() as u8
        );
        assert_eq!(
            trellis_coordinate(device, key),
            Some((
                logical["x"].as_u64().unwrap() as usize,
                logical["y"].as_u64().unwrap() as usize,
            ))
        );
        assert!(mappings.insert((device, key)));
    }
    assert_eq!(mappings.len(), 64);
    for device in 0..4 {
        for key in 0..16_u8 {
            assert!(mappings.contains(&(device, key)));
        }
    }
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
    scan_count: u8,
    fifo: Vec<u8>,
    fail_write_at: Option<usize>,
    fail_reads: bool,
    write_calls: usize,
    read_calls: usize,
}

struct FakeTransport {
    state: Arc<Mutex<FakeState>>,
}

type FakeStates = [Arc<Mutex<FakeState>>; 4];
type FakeTransports = [Box<dyn SeesawTransport>; 4];

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
        if state.fail_reads {
            return Err("fake read failure".into());
        }
        let response = match (base, function) {
            (SEESAW_STATUS_BASE, SEESAW_HW_ID) => vec![state.hw_id],
            (SEESAW_KEYPAD_BASE, SEESAW_KEYPAD_COUNT) => vec![state.scan_count],
            (SEESAW_KEYPAD_BASE, SEESAW_KEYPAD_FIFO) => state.fifo.clone(),
            _ => vec![0; buffer.len()],
        };
        if response.len() != buffer.len() {
            return Err("fake response length mismatch".into());
        }
        buffer.copy_from_slice(&response);
        Ok(())
    }
}

fn fake_transports() -> (FakeTransports, FakeStates) {
    let states = std::array::from_fn(|_| {
        Arc::new(Mutex::new(FakeState {
            hw_id: 0x84,
            ..FakeState::default()
        }))
    });
    let transports = std::array::from_fn(|index| {
        Box::new(FakeTransport {
            state: Arc::clone(&states[index]),
        }) as Box<dyn SeesawTransport>
    });
    (transports, states)
}

fn expected_device_init(mode: SeesawInputMode) -> Vec<Transaction> {
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
    ];
    if mode == SeesawInputMode::Interrupt {
        transactions.push(Transaction::Write {
            base: SEESAW_KEYPAD_BASE,
            function: SEESAW_KEYPAD_INTENSET,
            data: vec![0x01],
        });
    }
    for key in 0..16_u8 {
        let seesaw_key = trellis_key_to_seesaw_key(key);
        for edge in [KEYPAD_EDGE_FALLING, KEYPAD_EDGE_RISING] {
            transactions.push(Transaction::Write {
                base: SEESAW_KEYPAD_BASE,
                function: SEESAW_KEYPAD_EVENT,
                data: vec![seesaw_key, 0x01 | (1 << (edge + 1))],
            });
        }
    }
    transactions.extend([
        Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_PIN,
            data: vec![TRELLIS_NEOPIXEL_PIN],
        },
        Transaction::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF_LENGTH,
            data: vec![0, 48],
        },
    ]);
    transactions
}

#[test]
fn initializes_each_device_in_exact_polling_order() {
    let (transports, states) = fake_transports();
    NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .unwrap();

    for state in states {
        assert_eq!(
            lock_state(&state).transactions,
            expected_device_init(SeesawInputMode::Polling)
        );
    }
}

#[test]
fn interrupt_initialization_adds_keypad_interrupt_setup() {
    let (transports, states) = fake_transports();
    NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Interrupt,
    )
    .unwrap();

    for state in states {
        assert_eq!(
            lock_state(&state).transactions,
            expected_device_init(SeesawInputMode::Interrupt)
        );
    }
}

#[test]
fn validates_each_device_hw_id() {
    let (transports, states) = fake_transports();
    lock_state(&states[2]).hw_id = 0x12;
    let error = NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .err()
    .expect("invalid HW ID should fail");

    assert!(error.contains("Trellis HW ID invalid at 0x30: 0x12"));
}

#[test]
fn reads_fifo_edges_and_maps_coordinates() {
    let (transports, states) = fake_transports();
    {
        let mut state = lock_state(&states[0]);
        state.scan_count = 4;
        state.fifo = vec![
            KEYPAD_EDGE_RISING,
            (10 << 2) | KEYPAD_EDGE_FALLING,
            1,
            (32 << 2) | KEYPAD_EDGE_RISING,
        ];
    }
    let mut trellis = NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .unwrap();
    for state in &states {
        lock_state(state).transactions.clear();
    }

    assert_eq!(
        trellis.scan_keys().unwrap(),
        vec![(0, 7, true), (2, 6, false)]
    );
    assert_eq!(
        lock_state(&states[0]).transactions,
        vec![
            Transaction::Read {
                base: SEESAW_KEYPAD_BASE,
                function: SEESAW_KEYPAD_COUNT,
                length: 1,
            },
            Transaction::Read {
                base: SEESAW_KEYPAD_BASE,
                function: SEESAW_KEYPAD_FIFO,
                length: 4,
            },
        ]
    );
}

#[test]
fn writes_24_byte_grb_chunks_for_each_quadrant_then_shows() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .unwrap();
    for state in &states {
        lock_state(state).transactions.clear();
    }

    let frame = std::array::from_fn(|index| [index as u8, index as u8 + 1, index as u8 + 2]);
    trellis.write_led_frame(&frame).unwrap();

    for (device_index, state) in states.iter().enumerate() {
        let base_x = (device_index % 2) * 4;
        let base_y = (device_index / 2) * 4;
        let mut data = Vec::new();
        for y in base_y..(base_y + 4) {
            for x in base_x..(base_x + 4) {
                data.extend_from_slice(&grb_color(frame[y * 8 + x]));
            }
        }
        assert_eq!(
            lock_state(state).transactions,
            vec![
                Transaction::Write {
                    base: SEESAW_NEOPIXEL_BASE,
                    function: SEESAW_NEOPIXEL_BUF,
                    data: [&[0, 0][..], &data[..24]].concat(),
                },
                Transaction::Write {
                    base: SEESAW_NEOPIXEL_BASE,
                    function: SEESAW_NEOPIXEL_BUF,
                    data: [&[0, 24][..], &data[24..]].concat(),
                },
                Transaction::Write {
                    base: SEESAW_NEOPIXEL_BASE,
                    function: SEESAW_NEOPIXEL_SHOW,
                    data: Vec::new(),
                },
            ]
        );
    }
}

#[test]
fn projects_unique_sentinel_leds_using_fixture_device_keys() {
    let (transports, states) = fake_transports();
    let mut trellis = NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .unwrap();
    for state in &states {
        lock_state(state).transactions.clear();
    }

    let fixture = grid_projection_fixture();
    let cells = fixture["cells"].as_array().unwrap();
    let mut frame = [[0_u8; 3]; 64];
    for (sentinel_index, cell) in cells.iter().enumerate() {
        let display_index = cell["display"]["index"].as_u64().unwrap() as usize;
        frame[display_index] = [
            sentinel_index as u8 + 1,
            sentinel_index as u8 + 65,
            sentinel_index as u8 + 129,
        ];
    }
    trellis.write_led_frame(&frame).unwrap();

    for (device_index, state) in states.iter().enumerate() {
        let mut data = [0_u8; TRELLIS_PIXEL_BYTES_PER_DEVICE];
        let state = lock_state(state);
        for transaction in &state.transactions {
            let Transaction::Write {
                base: SEESAW_NEOPIXEL_BASE,
                function: SEESAW_NEOPIXEL_BUF,
                data: chunk,
            } = transaction
            else {
                continue;
            };
            let offset = u16::from_be_bytes([chunk[0], chunk[1]]) as usize;
            data[offset..offset + chunk.len() - 2].copy_from_slice(&chunk[2..]);
        }
        for cell in cells {
            let trellis = &cell["neotrellis"];
            if trellis["device_index"].as_u64().unwrap() as usize != device_index {
                continue;
            }
            let display_index = cell["display"]["index"].as_u64().unwrap() as usize;
            let key = trellis["key"].as_u64().unwrap() as usize;
            assert_eq!(
                &data[key * 3..key * 3 + 3],
                &grb_color(frame[display_index])
            );
        }
    }
}

#[test]
fn retries_a_transient_write_failure_without_sleeping() {
    let (transports, states) = fake_transports();
    lock_state(&states[0]).fail_write_at = Some(1);
    NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .unwrap();

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
    let error = NeoTrellis::from_transport(
        [0x2E, 0x2F, 0x30, 0x31],
        transports,
        SeesawInputMode::Polling,
    )
    .err()
    .expect("read failure should fail initialization");

    assert!(error.contains("Trellis HW ID read failed: fake read failure"));
    assert_eq!(lock_state(&states[1]).read_calls, 3);
}
