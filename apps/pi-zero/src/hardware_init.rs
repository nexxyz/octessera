use cellsymphony_hal::{
    encoder_gpio::HardwareEvent, EncoderGpio, I2CBus, I2sDac, NeoKey, NeoTrellis, OledSsd1351,
    SeesawInterrupt,
};
use std::sync::mpsc;

use crate::hardware_fault::HardwareFault;

pub(crate) struct HardwareDevices {
    pub(crate) _i2c_bus: I2CBus,
    pub(crate) oled: OledSsd1351,
    pub(crate) trellis: NeoTrellis,
    pub(crate) neokey: NeoKey,
    pub(crate) input_interrupt: SeesawInterrupt,
    pub(crate) _dac: I2sDac,
}

pub(crate) fn init_hardware() -> Result<HardwareDevices, HardwareFault> {
    let mut fault = HardwareFault::new();
    let i2c_bus = init_device("I2C", I2CBus::new(1), &mut fault);
    let oled = init_device("OLED", OledSsd1351::new(), &mut fault);
    let trellis = init_device("TRELLIS", NeoTrellis::new("/dev/i2c-1"), &mut fault);
    let neokey = init_device("NEOKEY", NeoKey::new("/dev/i2c-1"), &mut fault);
    let input_interrupt = init_device("SEESAW_INT", SeesawInterrupt::new(), &mut fault);
    let dac = init_device("DAC", I2sDac::new(), &mut fault);

    match (i2c_bus, oled, trellis, neokey, input_interrupt, dac) {
        (
            Some(_i2c_bus),
            Some(oled),
            Some(trellis),
            Some(neokey),
            Some(input_interrupt),
            Some(_dac),
        ) if fault.is_empty() => Ok(HardwareDevices {
            _i2c_bus,
            oled,
            trellis,
            neokey,
            input_interrupt,
            _dac,
        }),
        (_, oled, trellis, neokey, _, _) => {
            fault.attach_outputs(oled, trellis, neokey);
            Err(fault)
        }
    }
}

pub(crate) fn init_encoders(
) -> Result<(mpsc::Receiver<HardwareEvent>, Vec<EncoderGpio>), HardwareFault> {
    let (event_tx, event_rx) = mpsc::channel::<HardwareEvent>();
    let mut encoders = Vec::new();
    let mut fault = HardwareFault::new();
    for (index, pins) in cellsymphony_hal::pinmap::ENCODERS.iter().enumerate() {
        let id = match index {
            0 => "encoder_main",
            1 => "encoder_aux_1",
            2 => "encoder_aux_2",
            3 => "encoder_aux_3",
            _ => unreachable!("encoder pin count follows platform capabilities"),
        };
        match EncoderGpio::new(id, pins, event_tx.clone()) {
            Ok(encoder) => encoders.push(encoder),
            Err(error) => fault.push(id, error),
        }
    }
    if fault.is_empty() {
        Ok((event_rx, encoders))
    } else {
        Err(fault)
    }
}

fn init_device<T>(
    name: &'static str,
    result: Result<T, String>,
    fault: &mut HardwareFault,
) -> Option<T> {
    match result {
        Ok(device) => Some(device),
        Err(error) => {
            fault.push(name, error);
            None
        }
    }
}
