use super::{
    device_addresses, is_valid_hw_id, parse_args, require_active_test_confirmation, run_diagnostic,
    I2cOperations, Options, HW_ID, I2C_PATH, STATUS_BASE, SW_RESET, VALID_HW_IDS,
};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq)]
enum ExpectedOperation {
    Write {
        address: u16,
        data: Vec<u8>,
    },
    WriteRead {
        address: u16,
        command: Vec<u8>,
        response: Vec<u8>,
    },
}

#[derive(Default)]
struct MockI2c {
    expected: VecDeque<ExpectedOperation>,
}

impl MockI2c {
    fn expect_write(&mut self, address: u16, data: &[u8]) {
        self.expected.push_back(ExpectedOperation::Write {
            address,
            data: data.to_vec(),
        });
    }

    fn expect_read(&mut self, address: u16, command: &[u8], response: &[u8]) {
        self.expected.push_back(ExpectedOperation::WriteRead {
            address,
            command: command.to_vec(),
            response: response.to_vec(),
        });
    }

    fn assert_complete(&self) {
        assert!(
            self.expected.is_empty(),
            "mock I2C protocol had unconsumed operations: {:?}",
            self.expected
        );
    }
}

impl I2cOperations for MockI2c {
    fn write(&mut self, address: u16, data: &[u8]) -> Result<(), String> {
        match self.expected.pop_front() {
            Some(ExpectedOperation::Write {
                address: expected_address,
                data: expected_data,
            }) if expected_address == address && expected_data == data => Ok(()),
            unexpected => Err(format!(
                "unexpected I2C write at {address:#04x}: {unexpected:?}"
            )),
        }
    }

    fn write_read(
        &mut self,
        address: u16,
        command: &[u8],
        response: &mut [u8],
    ) -> Result<(), String> {
        match self.expected.pop_front() {
            Some(ExpectedOperation::WriteRead {
                address: expected_address,
                command: expected_command,
                response: expected_response,
            }) if expected_address == address
                && expected_command == command
                && expected_response.len() == response.len() =>
            {
                response.copy_from_slice(&expected_response);
                Ok(())
            }
            unexpected => Err(format!(
                "unexpected I2C read at {address:#04x}: {unexpected:?}"
            )),
        }
    }
}

#[test]
fn active_mode_requires_explicit_confirmation() {
    assert_eq!(
        parse_args(Vec::<String>::new()).unwrap(),
        Options {
            confirm_active_test: false,
            print_build_metadata: false,
        }
    );
    assert!(require_active_test_confirmation(Options {
        confirm_active_test: false,
        print_build_metadata: false,
    })
    .is_err());
    assert!(require_active_test_confirmation(Options {
        confirm_active_test: true,
        print_build_metadata: false,
    })
    .is_ok());
}

#[test]
fn metadata_mode_is_hardware_free_and_exclusive() {
    assert_eq!(
        parse_args(["--print-build-metadata"]).unwrap(),
        Options {
            confirm_active_test: false,
            print_build_metadata: true,
        }
    );
    assert!(parse_args(["--confirm-active-test", "--print-build-metadata"]).is_err());
    assert!(parse_args(["--confirm-active-test", "--confirm-active-test"]).is_err());
}

#[test]
fn diagnostic_uses_only_the_orange_bus_and_exact_reset_id_sequence() {
    assert_eq!(I2C_PATH, "/dev/i2c-2");
    assert_eq!(device_addresses(), [0x2E, 0x2F, 0x30, 0x31, 0x3F]);

    let mut mock = MockI2c::default();
    for address in device_addresses() {
        mock.expect_write(address, &[STATUS_BASE, SW_RESET, 0xFF]);
    }
    for (address, id) in device_addresses()
        .into_iter()
        .zip([0x55, 0x84, 0x85, 0x86, 0x87])
    {
        mock.expect_read(address, &[STATUS_BASE, HW_ID], &[id]);
    }

    assert_eq!(
        run_diagnostic(
            &mut mock,
            Duration::ZERO,
            Instant::now() + Duration::from_secs(1),
            || false,
        )
        .unwrap(),
        vec![
            (0x2E, 0x55),
            (0x2F, 0x84),
            (0x30, 0x85),
            (0x31, 0x86),
            (0x3F, 0x87)
        ]
    );
    mock.assert_complete();
}

#[test]
fn checks_safety_before_and_after_each_transaction() {
    let mut mock = MockI2c::default();
    for address in device_addresses() {
        mock.expect_write(address, &[STATUS_BASE, SW_RESET, 0xFF]);
    }
    for address in device_addresses() {
        mock.expect_read(address, &[STATUS_BASE, HW_ID], &[0x55]);
    }
    let mut safety_checks = 0;
    run_diagnostic(
        &mut mock,
        Duration::ZERO,
        Instant::now() + Duration::from_secs(1),
        || {
            safety_checks += 1;
            false
        },
    )
    .unwrap();
    assert_eq!(safety_checks, 22);
    mock.assert_complete();
}

#[test]
fn only_known_seesaw_hardware_ids_are_accepted() {
    for id in VALID_HW_IDS {
        assert!(is_valid_hw_id(id), "known HW ID {id:#04x} was rejected");
    }
    for id in [0x00, 0x54, 0x83, 0x8A, 0xFF] {
        assert!(!is_valid_hw_id(id), "invalid HW ID {id:#04x} was accepted");
    }
}

#[test]
fn invalid_hardware_id_stops_after_the_exact_read() {
    let mut mock = MockI2c::default();
    for address in device_addresses() {
        mock.expect_write(address, &[STATUS_BASE, SW_RESET, 0xFF]);
    }
    mock.expect_read(0x2E, &[STATUS_BASE, HW_ID], &[0x00]);

    let error = run_diagnostic(
        &mut mock,
        Duration::ZERO,
        Instant::now() + Duration::from_secs(1),
        || false,
    )
    .unwrap_err();
    assert!(error.contains("NeoTrellis HW ID invalid at 0x2e"));
    mock.assert_complete();
}

#[test]
fn expired_budget_is_rejected_before_the_first_transaction() {
    let mut mock = MockI2c::default();
    let error = run_diagnostic(&mut mock, Duration::ZERO, Instant::now(), || false).unwrap_err();
    assert!(error.contains("cooperative budget expired"));
    mock.assert_complete();
}

#[test]
fn reset_delay_is_admitted_against_the_remaining_budget() {
    let mut mock = MockI2c::default();
    for address in device_addresses() {
        mock.expect_write(address, &[STATUS_BASE, SW_RESET, 0xFF]);
    }
    let error = run_diagnostic(
        &mut mock,
        Duration::from_secs(2),
        Instant::now() + Duration::from_secs(1),
        || false,
    )
    .unwrap_err();
    assert!(error.contains("reset delay would exceed"));
    mock.assert_complete();
}

#[test]
fn interruption_is_rejected_before_starting_i2c() {
    let mut mock = MockI2c::default();
    let error = run_diagnostic(
        &mut mock,
        Duration::ZERO,
        Instant::now() + Duration::from_secs(1),
        || true,
    )
    .unwrap_err();
    assert!(error.contains("interrupted"));
    mock.assert_complete();
}

#[test]
fn interruption_is_observed_after_a_synchronous_transaction() {
    let mut mock = MockI2c::default();
    mock.expect_write(0x2E, &[STATUS_BASE, SW_RESET, 0xFF]);
    let mut checks = 0;
    let error = run_diagnostic(
        &mut mock,
        Duration::ZERO,
        Instant::now() + Duration::from_secs(1),
        || {
            checks += 1;
            checks >= 2
        },
    )
    .unwrap_err();
    assert!(error.contains("interrupted"));
    assert!(error.contains("after reset"));
    mock.assert_complete();
}
