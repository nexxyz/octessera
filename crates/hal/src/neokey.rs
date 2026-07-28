#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use crate::board_profiles::SeesawInputMode;
#[cfg(feature = "rpi-zero-2w")]
use crate::pinmap::NEOKEY_ADDR;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::fs::{File, OpenOptions};
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::io::{Read, Write};
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
use std::thread;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
use std::time::{Duration, Instant};

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
use std::fmt;

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
pub struct NeoKey {
    i2c_path: String,
    addr: u16,
    debouncer: NeoKeyDebouncer,
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_STATUS_BASE: u8 = 0x00;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_HW_ID: u8 = 0x01;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_SW_RESET: u8 = 0x7F;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_BASE: u8 = 0x01;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_DIRCLR_BULK: u8 = 0x03;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_BULK: u8 = 0x04;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_BULK_SET: u8 = 0x05;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_INTENSET: u8 = 0x08;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_INTFLAG: u8 = 0x0A;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_GPIO_PULLENSET: u8 = 0x0B;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_BASE: u8 = 0x0E;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_PIN: u8 = 0x01;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_BUF_LENGTH: u8 = 0x03;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_BUF: u8 = 0x04;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const SEESAW_NEOPIXEL_SHOW: u8 = 0x05;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const NEOKEY_BUTTON_MASK: u32 = 0xF0;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const NEOKEY_NEOPIXEL_PIN: u8 = 3;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const NEOKEY_LED_BYTES: u16 = 12;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum NeoKeyInitCommand {
    Write {
        base: u8,
        function: u8,
    },
    Read {
        base: u8,
        function: u8,
        bytes: usize,
    },
    DelayMillis(u64),
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn neokey_init_plan(mode: SeesawInputMode) -> Vec<NeoKeyInitCommand> {
    let mut plan = vec![
        NeoKeyInitCommand::Write {
            base: SEESAW_STATUS_BASE,
            function: SEESAW_SW_RESET,
        },
        NeoKeyInitCommand::DelayMillis(500),
        NeoKeyInitCommand::Read {
            base: SEESAW_STATUS_BASE,
            function: SEESAW_HW_ID,
            bytes: 1,
        },
        NeoKeyInitCommand::Write {
            base: SEESAW_GPIO_BASE,
            function: SEESAW_GPIO_DIRCLR_BULK,
        },
        NeoKeyInitCommand::Write {
            base: SEESAW_GPIO_BASE,
            function: SEESAW_GPIO_PULLENSET,
        },
        NeoKeyInitCommand::Write {
            base: SEESAW_GPIO_BASE,
            function: SEESAW_GPIO_BULK_SET,
        },
    ];
    if mode == SeesawInputMode::Interrupt {
        plan.extend([
            NeoKeyInitCommand::Write {
                base: SEESAW_GPIO_BASE,
                function: SEESAW_GPIO_INTENSET,
            },
            NeoKeyInitCommand::Read {
                base: SEESAW_GPIO_BASE,
                function: SEESAW_GPIO_INTFLAG,
                bytes: 4,
            },
        ]);
    }
    plan.extend([
        NeoKeyInitCommand::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_PIN,
        },
        NeoKeyInitCommand::Write {
            base: SEESAW_NEOPIXEL_BASE,
            function: SEESAW_NEOPIXEL_BUF_LENGTH,
        },
    ]);
    plan
}
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
#[path = "neokey_debounce.rs"]
mod debounce;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w", test))]
use debounce::NeoKeyDebouncer;
#[cfg(test)]
use debounce::NEOKEY_DEBOUNCE;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const NEOKEY_INIT_ATTEMPTS: usize = 3;
#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
const NEOKEY_INIT_RETRY_DELAY: Duration = Duration::from_millis(250);

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
impl NeoKey {
    #[cfg(feature = "rpi-zero-2w")]
    pub fn new(i2c_path: &str) -> Result<Self, String> {
        Self::new_with_mode(i2c_path, NEOKEY_ADDR, SeesawInputMode::Interrupt)
    }

    pub fn new_with_mode(
        i2c_path: &str,
        address: u16,
        mode: SeesawInputMode,
    ) -> Result<Self, String> {
        let mut last_error = None;
        for attempt in 1..=NEOKEY_INIT_ATTEMPTS {
            match Self::try_new(i2c_path, address, mode) {
                Ok(neokey) => return Ok(neokey),
                Err(error) => {
                    last_error = Some(error);
                    if attempt < NEOKEY_INIT_ATTEMPTS {
                        thread::sleep(NEOKEY_INIT_RETRY_DELAY);
                    }
                }
            }
        }
        Err(last_error.unwrap_or_else(|| "NeoKey init failed".to_string()))
    }

    fn try_new(i2c_path: &str, address: u16, mode: SeesawInputMode) -> Result<Self, String> {
        let mut file = open_device(i2c_path, address)?;
        let mask = NEOKEY_BUTTON_MASK.to_be_bytes();
        let led_bytes = NEOKEY_LED_BYTES.to_be_bytes();
        for command in neokey_init_plan(mode) {
            match command {
                NeoKeyInitCommand::DelayMillis(millis) => {
                    thread::sleep(Duration::from_millis(millis));
                }
                NeoKeyInitCommand::Read {
                    base,
                    function,
                    bytes,
                } => {
                    let mut buffer = [0_u8; 4];
                    let context = if function == SEESAW_HW_ID {
                        "NeoKey HW ID read failed"
                    } else {
                        "NeoKey GPIO interrupt clear failed"
                    };
                    read_register(&mut file, base, function, &mut buffer[..bytes], context)?;
                    if function == SEESAW_HW_ID && !matches!(buffer[0], 0x55 | 0x84..=0x89) {
                        return Err(format!("NeoKey HW ID invalid: {:#04x}", buffer[0]));
                    }
                }
                NeoKeyInitCommand::Write { base, function } => {
                    let (data, context): (&[u8], &str) = match (base, function) {
                        (SEESAW_STATUS_BASE, SEESAW_SW_RESET) => (&[0xFF], "NeoKey reset failed"),
                        (SEESAW_GPIO_BASE, SEESAW_GPIO_DIRCLR_BULK) => {
                            (&mask, "NeoKey GPIO direction init failed")
                        }
                        (SEESAW_GPIO_BASE, SEESAW_GPIO_PULLENSET) => {
                            (&mask, "NeoKey GPIO pullup init failed")
                        }
                        (SEESAW_GPIO_BASE, SEESAW_GPIO_BULK_SET) => {
                            (&mask, "NeoKey GPIO pullup set failed")
                        }
                        (SEESAW_GPIO_BASE, SEESAW_GPIO_INTENSET) => {
                            (&mask, "NeoKey GPIO interrupt init failed")
                        }
                        (SEESAW_NEOPIXEL_BASE, SEESAW_NEOPIXEL_PIN) => {
                            (&[NEOKEY_NEOPIXEL_PIN], "NeoKey LED pin init failed")
                        }
                        (SEESAW_NEOPIXEL_BASE, SEESAW_NEOPIXEL_BUF_LENGTH) => {
                            (&led_bytes, "NeoKey LED length init failed")
                        }
                        _ => {
                            return Err(format!(
                                "unknown NeoKey init command: {base:#04x}/{function:#04x}"
                            ))
                        }
                    };
                    write_register(&mut file, base, function, data, context)?;
                }
            }
        }

        Ok(Self {
            i2c_path: i2c_path.to_string(),
            addr: address,
            debouncer: NeoKeyDebouncer::default(),
        })
    }

    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        let sampled = neokey_buttons_from_raw(self.raw_button_state()?);
        let stable_buttons = self.debouncer.update(sampled, Instant::now());

        let mut result = Vec::new();
        for i in 0..4 {
            result.push((i, stable_buttons[usize::from(i)]));
        }

        Ok(result)
    }

    #[cfg(feature = "rpi-zero-2w")]
    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        self.clear_interrupt_flags()?;
        self.scan()
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        let mut file = open_device(&self.i2c_path, self.addr)?;
        let mut buf = [0_u8; 4];
        read_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_BULK,
            &mut buf,
            "NeoKey raw scan failed",
        )?;
        Ok(u32::from_be_bytes(buf))
    }

    #[cfg(feature = "rpi-zero-2w")]
    fn clear_interrupt_flags(&mut self) -> Result<(), String> {
        let mut file = open_device(&self.i2c_path, self.addr)?;
        let mut buf = [0_u8; 4];
        read_register(
            &mut file,
            SEESAW_GPIO_BASE,
            SEESAW_GPIO_INTFLAG,
            &mut buf,
            "NeoKey GPIO interrupt clear failed",
        )
    }

    pub fn set_led(&mut self, key: u8, r: u8, g: u8, b: u8) -> Result<(), String> {
        if key >= 4 {
            return Err(format!("NeoKey LED index out of range: {key}"));
        }
        let mut file = open_device(&self.i2c_path, self.addr)?;
        let offset = u16::from(key) * 3;
        let mut data = Vec::with_capacity(5);
        data.extend_from_slice(&offset.to_be_bytes());
        data.extend_from_slice(&[g, r, b]);
        write_register(
            &mut file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_BUF,
            &data,
            "NeoKey LED write failed",
        )?;
        write_register(
            &mut file,
            SEESAW_NEOPIXEL_BASE,
            SEESAW_NEOPIXEL_SHOW,
            &[],
            "NeoKey LED show failed",
        )?;
        thread::sleep(Duration::from_micros(300));
        Ok(())
    }
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn neokey_buttons_from_raw(raw: u32) -> [bool; 4] {
    let state = raw & NEOKEY_BUTTON_MASK;
    [
        (state & (1 << 4)) == 0,
        (state & (1 << 5)) == 0,
        (state & (1 << 6)) == 0,
        (state & (1 << 7)) == 0,
    ]
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn open_device(i2c_path: &str, addr: u16) -> Result<File, String> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(i2c_path)
        .map_err(|e| format!("NeoKey I2C open failed at {addr:#04x}: {e}"))?;
    set_slave_addr(&file, addr)?;
    Ok(file)
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn write_register(
    file: &mut File,
    base: u8,
    function: u8,
    data: &[u8],
    context: &str,
) -> Result<(), String> {
    let mut command = Vec::with_capacity(2 + data.len());
    command.push(base);
    command.push(function);
    command.extend_from_slice(data);
    file.write_all(&command)
        .map_err(|e| format!("{context}: {e}"))
}

#[cfg(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w"))]
fn read_register(
    file: &mut File,
    base: u8,
    function: u8,
    buffer: &mut [u8],
    context: &str,
) -> Result<(), String> {
    file.write_all(&[base, function])
        .map_err(|e| format!("{context}: {e}"))?;
    thread::sleep(Duration::from_millis(1));
    file.read_exact(buffer)
        .map_err(|e| format!("{context}: {e}"))
}

#[cfg(all(feature = "orange-pi-zero-2w", not(unix)))]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    let _ = (file, addr);
    Err("Orange Seesaw I2C requires a Unix target".into())
}

#[cfg(any(feature = "rpi-zero-2w", all(feature = "orange-pi-zero-2w", unix)))]
fn set_slave_addr(file: &File, addr: u16) -> Result<(), String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        use std::os::unix::io::AsRawFd;
        let result = unsafe { libc::ioctl(file.as_raw_fd(), 0x0703, addr as u64) };
        if result < 0 {
            return Err(format!(
                "I2C slave select failed for {addr:#04x}: {}",
                std::io::Error::last_os_error()
            ));
        }
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    let _ = (file, addr);
    Ok(())
}

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
pub struct NeoKey {
    _private: (),
}

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
impl NeoKey {
    pub fn new(_i2c_path: &str) -> Result<Self, String> {
        Ok(Self { _private: () })
    }

    pub fn scan(&mut self) -> Result<Vec<(u8, bool)>, String> {
        Ok(Vec::new())
    }

    pub fn scan_interrupts(&mut self) -> Result<Vec<(u8, bool)>, String> {
        self.scan()
    }

    pub fn raw_button_state(&mut self) -> Result<u32, String> {
        Ok(0)
    }

    pub fn set_led(&mut self, _key: u8, _r: u8, _g: u8, _b: u8) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(not(any(feature = "rpi-zero-2w", feature = "orange-pi-zero-2w")))]
impl fmt::Debug for NeoKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NeoKey {{ ... }}")
    }
}

#[cfg(test)]
#[path = "neokey_tests.rs"]
mod tests;
