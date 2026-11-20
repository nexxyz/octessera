//! The keypad and NeoPixel setup each Trellis board gets at startup and again periodically.

use super::{
    read_register, trellis_key_to_seesaw_key, write_register, KEYPAD_EDGE_FALLING,
    KEYPAD_EDGE_RISING, SEESAW_HW_ID, SEESAW_KEYPAD_BASE, SEESAW_KEYPAD_EVENT,
    SEESAW_KEYPAD_INTENSET, SEESAW_NEOPIXEL_BASE, SEESAW_NEOPIXEL_BUF_LENGTH, SEESAW_NEOPIXEL_PIN,
    SEESAW_STATUS_BASE, TRELLIS_NEOPIXEL_PIN, TRELLIS_PIXEL_BYTES_PER_DEVICE,
};
use crate::seesaw_transport::SeesawTransport;

/// The firmware ORs every edge bit of one event write into the key's active edges, so a single
/// write enables both edges exactly like two single-edge writes.
const KEYPAD_EVENT_BOTH_EDGES: u8 =
    0x01 | (1 << (KEYPAD_EDGE_FALLING + 1)) | (1 << (KEYPAD_EDGE_RISING + 1));
pub(super) const TRELLIS_SETUP_WRITES: usize = 19;
/// Two small writes take about 1 ms on the Orange's 100 kHz bus.
pub(super) const SETUP_WRITES_PER_PART: usize = 2;

pub(super) fn configure_device(
    address: u16,
    transport: &mut dyn SeesawTransport,
) -> Result<(), String> {
    let mut hw_id = [0_u8; 1];
    read_register(
        transport,
        SEESAW_STATUS_BASE,
        SEESAW_HW_ID,
        &mut hw_id,
        "Trellis HW ID read failed",
    )?;
    if !matches!(hw_id[0], 0x55 | 0x84..=0x89) {
        return Err(format!(
            "Trellis HW ID invalid at {address:#04x}: {:#04x}",
            hw_id[0]
        ));
    }
    (0..TRELLIS_SETUP_WRITES).try_for_each(|index| write_setup(transport, index))
}

pub(super) fn write_setup(transport: &mut dyn SeesawTransport, index: usize) -> Result<(), String> {
    match index {
        0 => write_register(
            transport,
            SEESAW_KEYPAD_BASE,
            SEESAW_KEYPAD_INTENSET,
            &[0x01],
            "Trellis keypad interrupt init failed",
        ),
        1..=16 => write_register(
            transport,
            SEESAW_KEYPAD_BASE,
            SEESAW_KEYPAD_EVENT,
            &[
                trellis_key_to_seesaw_key(index as u8 - 1),
                KEYPAD_EVENT_BOTH_EDGES,
            ],
            "Trellis keypad event init failed",
        ),
        17 => write_register(
            transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_PIN,
            &[TRELLIS_NEOPIXEL_PIN],
            "Trellis LED pin init failed",
        ),
        _ => write_register(
            transport,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF_LENGTH,
            &(TRELLIS_PIXEL_BYTES_PER_DEVICE as u16).to_be_bytes(),
            "Trellis LED length init failed",
        ),
    }
}
