use super::{
    OrangeUartConflict, ACTIVE_BOARD_PROFILE, ORANGE_PI_ZERO_2W, ORANGE_PI_ZERO_2W_DEVICES,
    ORANGE_PI_ZERO_2W_ID,
};

#[test]
fn orange_profile_selects_typed_linux_devices_without_raspberry_pins() {
    assert_eq!(ACTIVE_BOARD_PROFILE, ORANGE_PI_ZERO_2W);
    assert_eq!(ORANGE_PI_ZERO_2W.id, ORANGE_PI_ZERO_2W_ID);
    assert_eq!(ORANGE_PI_ZERO_2W_DEVICES, ORANGE_PI_ZERO_2W.devices);
    assert_eq!(ORANGE_PI_ZERO_2W.devices.i2c.path, "/dev/i2c-2");
    assert_eq!(ORANGE_PI_ZERO_2W.devices.i2c.controller, "5002400.i2c");
    assert_eq!(
        ORANGE_PI_ZERO_2W.devices.trellis_addrs,
        [0x2E, 0x2F, 0x30, 0x31]
    );
    assert_eq!(ORANGE_PI_ZERO_2W.devices.neokey_addr, 0x3F);
    assert_eq!(ORANGE_PI_ZERO_2W.devices.spi.path, "/dev/spidev1.0");
    assert_eq!(ORANGE_PI_ZERO_2W.devices.spi.controller, "5011000.spi");
    assert_eq!(ORANGE_PI_ZERO_2W.devices.gpio.chip_label, "300b000.pinctrl");
    assert_eq!(ORANGE_PI_ZERO_2W.devices.gpio.dc_offset, 270);
    assert_eq!(ORANGE_PI_ZERO_2W.devices.gpio.reset_offset, 76);
}

#[test]
fn orange_encoder_header_mapping_matches_h618_offsets() {
    let encoders = ORANGE_PI_ZERO_2W_DEVICES.encoders;
    assert_eq!(
        encoders.map(|pins| (
            (pins.physical_a, pins.physical_b, pins.physical_sw),
            (pins.a, pins.b, pins.sw),
        )),
        [
            ((29, 31, 32), (256, 271, 267)),
            ((33, 22, 11), (268, 262, 226)),
            ((13, 7, 8), (227, 269, 224)),
            ((37, 18, 15), (272, 228, 261)),
        ]
    );
    assert!(encoders.iter().all(|pins| pins.reverse_direction));
}

#[test]
fn orange_uart_switch_is_optional_but_aux2_quadrature_is_always_requested() {
    let encoders = ORANGE_PI_ZERO_2W_DEVICES.encoders;
    assert_eq!(
        encoders[2].uart_conflict,
        Some(OrangeUartConflict {
            signal: "UART0 TX",
            physical_pin: 8,
            offset: 224,
        })
    );
    assert_eq!(
        encoders[2].requested_gpio_offsets(true),
        [Some(227), Some(269), None]
    );
    assert_eq!(
        encoders[2].requested_gpio_offsets(false),
        [Some(227), Some(269), Some(224)]
    );
}

#[test]
fn orange_descriptor_reverses_literal_board_turns_at_event_boundary() {
    let pins = ORANGE_PI_ZERO_2W_DEVICES.encoders;
    assert_eq!(
        pins.map(|value| value.canonical_turn_delta(1)),
        [-1, -1, -1, -1]
    );
    assert_eq!(
        pins.map(|value| value.canonical_turn_delta(-1)),
        [1, 1, 1, 1]
    );
}
