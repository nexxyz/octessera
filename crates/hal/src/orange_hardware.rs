#[cfg(target_os = "linux")]
use crate::board_profiles::ORANGE_PI_ZERO_2W_DEVICES;
#[cfg(target_os = "linux")]
use crate::board_profiles::{DeviceDescriptor, OrangeGpioDescriptor};

#[cfg(target_os = "linux")]
use gpiocdev::line::Value;
#[cfg(target_os = "linux")]
use gpiocdev::{Chip, Request};
#[cfg(target_os = "linux")]
use std::fs::{self, File, OpenOptions};
#[cfg(target_os = "linux")]
use std::io::Write;
#[cfg(target_os = "linux")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(any(test, target_os = "linux"))]
const ORANGE_OLED_SPI_HZ_LADDER: &[u32] = &[
    1_000_000, 2_000_000, 4_000_000, 8_000_000, 12_000_000, 16_000_000,
];
#[cfg(target_os = "linux")]
const OLED_FRAME_BYTES: usize = timing::OLED_FRAME_BYTES;
#[cfg(any(test, target_os = "linux"))]
const FRAME_CHUNK_BYTES: usize = 1024;

pub use crate::orange_timing as timing;
pub use crate::orange_timing::{
    POST_DISPLAY_ON_MS, PRE_RESET_DELAY_MS, RESET_HIGH_MS, RESET_LOW_MS, RESET_SETTLE_MS,
};

pub const ORANGE_INPUTS_UNSUPPORTED_ERROR: &str =
    "Orange Pi encoder and Seesaw interrupt mappings are unsupported and unqualified; no input GPIO mappings are selected in this backend";
pub const ORANGE_AUDIO_UNAVAILABLE_ERROR: &str =
    "Orange Pi audio/I2S backend is intentionally unavailable in this HAL";

#[cfg(target_os = "linux")]
pub struct OrangeHardware {
    _i2c: File,
    oled: OrangeOledTransport,
}

#[cfg(target_os = "linux")]
impl OrangeHardware {
    pub fn open() -> Result<Self, String> {
        Self::open_until_mode(timing::operation_deadline(), false)
    }

    pub fn open_preserve_existing() -> Result<Self, String> {
        Self::open_until_mode(timing::operation_deadline(), true)
    }

    pub fn open_until(deadline: std::time::Instant) -> Result<Self, String> {
        Self::open_until_mode(deadline, false)
    }

    fn open_until_mode(
        deadline: std::time::Instant,
        preserve_existing: bool,
    ) -> Result<Self, String> {
        let startup_plan = crate::oled_startup_plan::OledStartupPlan::new(preserve_existing);
        let devices = ORANGE_PI_ZERO_2W_DEVICES;
        check_deadline(deadline)?;
        verify_device_identity(devices.i2c, "/sys/class/i2c-dev")?;
        let i2c = OpenOptions::new()
            .read(true)
            .write(true)
            .open(devices.i2c.path)
            .map_err(|error| format!("I2C open failed for {}: {error}", devices.i2c.path))?;
        check_deadline(deadline)?;
        let oled = OrangeOledTransport::open_until(
            devices.spi,
            devices.gpio,
            deadline,
            startup_plan.operations().is_empty(),
        )?;
        Ok(Self { _i2c: i2c, oled })
    }

    pub fn initialize_inputs(&self) -> Result<(), String> {
        Err(ORANGE_INPUTS_UNSUPPORTED_ERROR.into())
    }

    pub fn initialize_audio(&self) -> Result<(), String> {
        Err(ORANGE_AUDIO_UNAVAILABLE_ERROR.into())
    }

    pub fn oled_mut(&mut self) -> &mut OrangeOledTransport {
        &mut self.oled
    }

    pub fn into_oled(self) -> OrangeOledTransport {
        self.oled
    }
}

#[cfg(target_os = "linux")]
pub struct OrangeOledTransport {
    spi: spidev::Spidev,
    gpio: Request,
    gpio_plan: OrangeGpioDescriptor,
    shutdown_complete: bool,
}

#[cfg(any(test, target_os = "linux"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupStep {
    DisplayOff,
    BlackFrame,
}

#[cfg(any(test, target_os = "linux"))]
fn fallback_cleanup_steps() -> [CleanupStep; 2] {
    [CleanupStep::DisplayOff, CleanupStep::BlackFrame]
}

#[cfg(target_os = "linux")]
impl OrangeOledTransport {
    fn open_until(
        spi_device: DeviceDescriptor,
        gpio_plan: OrangeGpioDescriptor,
        deadline: std::time::Instant,
        preserve_existing: bool,
    ) -> Result<Self, String> {
        check_deadline(deadline)?;
        verify_device_identity(spi_device, "/sys/class/spidev")?;
        let chip_path = find_gpio_chip(gpio_plan)?;
        let gpio = request_gpio(&chip_path, gpio_plan)?;
        let mut oled = Self {
            spi: open_spi(spi_device.path)?,
            gpio,
            gpio_plan,
            shutdown_complete: false,
        };
        if !preserve_existing {
            oled.perform_reset_until(deadline)?;
            oled.initialize_display_until(deadline)?;
        }
        Ok(oled)
    }

    pub fn display_off(&mut self) -> Result<(), String> {
        self.display_off_until(timing::operation_deadline())
    }

    pub fn display_on(&mut self) -> Result<(), String> {
        let deadline = timing::operation_deadline();
        self.write_command_until(0xA6, &[], deadline)?;
        self.write_command_until(0xAF, &[], deadline)
    }

    pub fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        self.write_frame_until(frame, timing::operation_deadline())
    }

    pub fn display_off_until(&mut self, deadline: std::time::Instant) -> Result<(), String> {
        self.write_command_until(0xAE, &[], deadline)
    }

    pub fn write_frame_until(
        &mut self,
        frame: &[u8],
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        self.write_command_until(0x15, &[0x00, 0x7F], deadline)?;
        self.write_command_until(0x75, &[0x00, 0x7F], deadline)?;
        check_deadline(deadline)?;
        self.set_dc(false)?;
        self.spi
            .write_all(&[0x5C])
            .map_err(|error| format!("RAM write command failed: {error}"))?;
        check_deadline(deadline)?;
        self.set_dc(true)?;
        for range in frame_chunk_ranges(frame.len()) {
            check_deadline(deadline)?;
            self.spi
                .write_all(&frame[range])
                .map_err(|error| format!("OLED frame chunk write failed: {error}"))?;
        }
        check_deadline(deadline)?;
        Ok(())
    }

    pub fn shutdown_until(
        &mut self,
        black_frame: &[u8],
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        self.write_frame_until(black_frame, deadline)?;
        self.display_off_until(deadline)?;
        self.shutdown_complete = true;
        Ok(())
    }

    fn set_dc(&mut self, active: bool) -> Result<(), String> {
        self.gpio
            .set_value(self.gpio_plan.dc_offset, gpio_value(active))
            .map_err(|error| format!("D/C GPIO update failed: {error}"))
    }

    fn set_dc_until(&mut self, active: bool, deadline: std::time::Instant) -> Result<(), String> {
        check_deadline(deadline)?;
        self.set_dc(active)?;
        check_deadline(deadline)
    }

    fn set_reset_until(
        &mut self,
        active: bool,
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        check_deadline(deadline)?;
        self.gpio
            .set_value(
                self.gpio_plan.reset_offset,
                reset_gpio_value(active, self.gpio_plan),
            )
            .map_err(|error| format!("reset cleanup update failed: {error}"))?;
        check_deadline(deadline)
    }

    fn write_command_until(
        &mut self,
        command: u8,
        data: &[u8],
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        check_deadline(deadline)?;
        self.set_dc(false)?;
        self.spi
            .write_all(&[command])
            .map_err(|error| format!("OLED command 0x{command:02X} failed: {error}"))?;
        if !data.is_empty() {
            self.set_dc(true)?;
            self.spi
                .write_all(data)
                .map_err(|error| format!("OLED command 0x{command:02X} data failed: {error}"))?;
        }
        check_deadline(deadline)
    }

    fn perform_reset_until(&mut self, deadline: std::time::Instant) -> Result<(), String> {
        check_deadline(deadline)?;
        timing::sleep_within_budget(deadline, Duration::from_millis(PRE_RESET_DELAY_MS))?;
        check_deadline(deadline)?;
        self.gpio
            .set_value(
                self.gpio_plan.reset_offset,
                reset_gpio_value(false, self.gpio_plan),
            )
            .map_err(|error| format!("reset high failed: {error}"))?;
        timing::sleep_within_budget(deadline, Duration::from_millis(RESET_HIGH_MS))?;
        check_deadline(deadline)?;
        self.gpio
            .set_value(
                self.gpio_plan.reset_offset,
                reset_gpio_value(true, self.gpio_plan),
            )
            .map_err(|error| format!("reset low failed: {error}"))?;
        timing::sleep_within_budget(deadline, Duration::from_millis(RESET_LOW_MS))?;
        check_deadline(deadline)?;
        self.gpio
            .set_value(
                self.gpio_plan.reset_offset,
                reset_gpio_value(false, self.gpio_plan),
            )
            .map_err(|error| format!("reset release failed: {error}"))?;
        timing::sleep_within_budget(deadline, Duration::from_millis(RESET_SETTLE_MS))
    }

    fn initialize_display_until(&mut self, deadline: std::time::Instant) -> Result<(), String> {
        for (command, data) in [
            (0xFD, &[0x12][..]),
            (0xFD, &[0xB1][..]),
            (0xAE, &[][..]),
            (0xB3, &[0xF1][..]),
            (0xCA, &[0x7F][..]),
            (0xA0, &[0x74][..]),
            (0x15, &[0x00, 0x7F][..]),
            (0x75, &[0x00, 0x7F][..]),
            (0xA1, &[0x00][..]),
            (0xA2, &[0x00][..]),
            (0xB5, &[0x00][..]),
            (0xAB, &[0x01][..]),
            (0xB1, &[0x32][..]),
            (0xBB, &[0x17][..]),
            (0xBE, &[0x05][..]),
            (0xA6, &[][..]),
            (0xC1, &[0xC8, 0x80, 0xC8][..]),
            (0xC7, &[0x0F][..]),
            (0xB4, &[0xA0, 0xB5, 0x55][..]),
            (0xB6, &[0x01][..]),
            (0xAF, &[][..]),
        ] {
            self.write_command_until(command, data, deadline)?;
        }
        timing::sleep_within_budget(deadline, Duration::from_millis(POST_DISPLAY_ON_MS))
    }
}

#[cfg(any(test, target_os = "linux"))]
fn frame_chunk_ranges(total_bytes: usize) -> Vec<std::ops::Range<usize>> {
    (0..total_bytes)
        .step_by(FRAME_CHUNK_BYTES)
        .map(|start| start..(start + FRAME_CHUNK_BYTES).min(total_bytes))
        .collect()
}

#[cfg(target_os = "linux")]
impl Drop for OrangeOledTransport {
    fn drop(&mut self) {
        if self.shutdown_complete {
            return;
        }
        let cleanup_deadline = timing::cleanup_deadline();
        let black = vec![0; OLED_FRAME_BYTES];
        for step in fallback_cleanup_steps() {
            match step {
                CleanupStep::DisplayOff => {
                    let _ = self.display_off_until(cleanup_deadline);
                }
                CleanupStep::BlackFrame => {
                    let _ = self.write_frame_until(&black, cleanup_deadline);
                }
            }
        }
        let _ = self.set_dc_until(false, cleanup_deadline);
        let _ = self.set_reset_until(false, cleanup_deadline);
    }
}

#[cfg(target_os = "linux")]
fn check_deadline(deadline: std::time::Instant) -> Result<(), String> {
    timing::ensure_before_deadline(deadline)
}

#[cfg(target_os = "linux")]
fn verify_device_identity(device: DeviceDescriptor, sysfs_root: &str) -> Result<(), String> {
    if !Path::new(device.path).exists() {
        return Err(format!("device {} does not exist", device.path));
    }
    let name = Path::new(device.path)
        .file_name()
        .ok_or_else(|| format!("device path {} has no basename", device.path))?;
    let sysfs_device = Path::new(sysfs_root).join(name).join("device");
    let resolved = fs::canonicalize(&sysfs_device).map_err(|error| {
        format!(
            "cannot resolve {} for expected {}: {error}",
            sysfs_device.display(),
            device.controller
        )
    })?;
    if !resolved.to_string_lossy().contains(device.controller) {
        return Err(format!(
            "{} resolves to {}, expected {}",
            device.path,
            resolved.display(),
            device.controller
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn find_gpio_chip(plan: OrangeGpioDescriptor) -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir("/dev").map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("gpiochip") {
            continue;
        }
        let path = entry.path();
        let chip = Chip::from_path(&path)
            .map_err(|error| format!("cannot open GPIO chip {}: {error}", path.display()))?;
        let info = chip
            .info()
            .map_err(|error| format!("cannot read GPIO chip {}: {error}", path.display()))?;
        if info.label == plan.chip_label {
            if plan.dc_offset >= info.num_lines || plan.reset_offset >= info.num_lines {
                return Err(format!(
                    "GPIO chip {} has only {} lines for offsets {} and {}",
                    plan.chip_label, info.num_lines, plan.dc_offset, plan.reset_offset
                ));
            }
            return Ok(path);
        }
        candidates.push(format!("{}={}", path.display(), info.label));
    }
    Err(format!(
        "no GPIO chip label {}; candidates: {}",
        plan.chip_label,
        candidates.join(", ")
    ))
}

#[cfg(target_os = "linux")]
fn request_gpio(chip_path: &Path, plan: OrangeGpioDescriptor) -> Result<Request, String> {
    Request::builder()
        .on_chip(chip_path)
        .with_consumer("octessera-orange-hardware")
        .with_lines(&[plan.dc_offset, plan.reset_offset])
        .as_output(gpio_value(false))
        .with_line(plan.reset_offset)
        .with_value(reset_gpio_value(false, plan))
        .request()
        .map_err(|error| format!("Orange GPIO request failed: {error}"))
}

#[cfg(target_os = "linux")]
fn open_spi(path: &str) -> Result<spidev::Spidev, String> {
    use spidev::{SpiModeFlags, Spidev, SpidevOptions};
    let speed_hz = orange_oled_spi_hz_from_env(
        std::env::var("OCTESSERA_ORANGE_OLED_SPI_HZ")
            .ok()
            .as_deref(),
    )?;
    let mut spi = Spidev::open(path).map_err(|error| format!("SPI open failed: {error}"))?;
    let options = SpidevOptions::new()
        .bits_per_word(8)
        .max_speed_hz(speed_hz)
        .mode(SpiModeFlags::SPI_MODE_0)
        .build();
    spi.configure(&options)
        .map_err(|error| format!("SPI configure failed: {error}"))?;
    Ok(spi)
}

#[cfg(any(test, target_os = "linux"))]
fn orange_oled_spi_hz_from_env(value: Option<&str>) -> Result<u32, String> {
    let Some(value) = value else {
        return Ok(crate::orange_timing::SPI_SPEED_HZ as u32);
    };
    let speed_hz = value.parse::<u32>().map_err(|_| {
        format!(
            "OCTESSERA_ORANGE_OLED_SPI_HZ must be one of {:?} Hz; got {value:?}",
            ORANGE_OLED_SPI_HZ_LADDER
        )
    })?;
    if ORANGE_OLED_SPI_HZ_LADDER.contains(&speed_hz) {
        Ok(speed_hz)
    } else {
        Err(format!(
            "OCTESSERA_ORANGE_OLED_SPI_HZ must be one of {:?} Hz; got {value:?}",
            ORANGE_OLED_SPI_HZ_LADDER
        ))
    }
}

#[cfg(target_os = "linux")]
fn gpio_value(active: bool) -> Value {
    if active {
        Value::Active
    } else {
        Value::Inactive
    }
}

#[cfg(target_os = "linux")]
fn reset_gpio_value(active: bool, plan: OrangeGpioDescriptor) -> Value {
    gpio_value(if plan.reset_active_low {
        !active
    } else {
        active
    })
}

#[cfg(test)]
#[path = "orange_hardware_tests.rs"]
mod tests;
