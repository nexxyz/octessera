use std::env;
use std::process;
#[cfg(any(test, target_os = "linux"))]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(any(test, target_os = "linux"))]
use octessera_hal::board_profiles::ORANGE_PI_ZERO_2W_DEVICES;
use octessera_hal::orange_metadata::{print_build_metadata_for, SEESAW_BINARY_NAME};

#[cfg(any(test, target_os = "linux"))]
const I2C_PATH: &str = ORANGE_PI_ZERO_2W_DEVICES.i2c.path;
#[cfg(any(test, target_os = "linux"))]
const STATUS_BASE: u8 = 0x00;
#[cfg(any(test, target_os = "linux"))]
const HW_ID: u8 = 0x01;
#[cfg(any(test, target_os = "linux"))]
const SW_RESET: u8 = 0x7F;
#[cfg(any(test, target_os = "linux"))]
const VALID_HW_IDS: [u8; 7] = [0x55, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89];
#[cfg(target_os = "linux")]
const RESET_DELAY: Duration = Duration::from_millis(500);
#[cfg(target_os = "linux")]
const OPERATION_BUDGET: Duration = Duration::from_secs(3);

#[cfg(target_os = "linux")]
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    confirm_active_test: bool,
    print_build_metadata: bool,
}

#[cfg(any(test, target_os = "linux"))]
trait I2cOperations {
    fn write(&mut self, address: u16, data: &[u8]) -> Result<(), String>;

    fn write_read(
        &mut self,
        address: u16,
        write_data: &[u8],
        read_data: &mut [u8],
    ) -> Result<(), String>;
}

fn main() {
    let options = match parse_args(env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: orange-seesaw-smoke --confirm-active-test | --print-build-metadata");
            process::exit(2);
        }
    };

    if options.print_build_metadata {
        if let Err(error) = print_build_metadata_for(SEESAW_BINARY_NAME) {
            eprintln!("Orange Seesaw build metadata check failed: {error}");
            process::exit(1);
        }
        return;
    }

    if let Err(error) = require_active_test_confirmation(options) {
        eprintln!("{error}");
        process::exit(2);
    }

    #[cfg(not(target_os = "linux"))]
    {
        eprintln!("Orange Seesaw smoke requires a Linux target with the real Orange HAL");
        process::exit(2);
    }

    #[cfg(target_os = "linux")]
    if let Err(error) = run_smoke_test() {
        eprintln!("Orange Seesaw smoke failed: {error}");
        process::exit(1);
    }
}

fn parse_args<I, S>(args: I) -> Result<Options, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut options = Options {
        confirm_active_test: false,
        print_build_metadata: false,
    };
    for arg in args {
        match arg.into().as_str() {
            "--confirm-active-test" if !options.confirm_active_test => {
                options.confirm_active_test = true;
            }
            "--confirm-active-test" => return Err("duplicate --confirm-active-test".into()),
            "--print-build-metadata" if !options.print_build_metadata => {
                options.print_build_metadata = true;
            }
            "--print-build-metadata" => return Err("duplicate --print-build-metadata".into()),
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }
    if options.confirm_active_test && options.print_build_metadata {
        return Err("--print-build-metadata is exclusive with --confirm-active-test".into());
    }
    Ok(options)
}

fn require_active_test_confirmation(options: Options) -> Result<(), &'static str> {
    if options.confirm_active_test {
        Ok(())
    } else {
        Err("refusing active Orange Seesaw test without --confirm-active-test")
    }
}

#[cfg(any(test, target_os = "linux"))]
fn ensure_safe<F>(deadline: Instant, interrupted: &mut F, phase: &str) -> Result<(), String>
where
    F: FnMut() -> bool,
{
    if interrupted() {
        return Err(format!(
            "Orange Seesaw smoke interrupted; synchronous I2C operations are observed only between calls ({phase})"
        ));
    }
    if Instant::now() >= deadline {
        return Err(format!(
            "Orange Seesaw smoke cooperative budget expired; {phase}"
        ));
    }
    Ok(())
}

#[cfg(any(test, target_os = "linux"))]
fn sleep_reset_delay_until<F>(
    deadline: Instant,
    reset_delay: Duration,
    interrupted: &mut F,
) -> Result<(), String>
where
    F: FnMut() -> bool,
{
    ensure_safe(deadline, interrupted, "before reset delay")?;
    let wake_time = Instant::now()
        .checked_add(reset_delay)
        .ok_or_else(|| "Orange Seesaw smoke reset delay overflowed its deadline".to_string())?;
    if wake_time >= deadline {
        return Err("Orange Seesaw smoke reset delay would exceed its cooperative budget".into());
    }
    std::thread::sleep(reset_delay);
    ensure_safe(deadline, interrupted, "after reset delay")
}

#[cfg(any(test, target_os = "linux"))]
fn device_addresses() -> [u16; 5] {
    let devices = ORANGE_PI_ZERO_2W_DEVICES;
    [
        devices.trellis_addrs[0],
        devices.trellis_addrs[1],
        devices.trellis_addrs[2],
        devices.trellis_addrs[3],
        devices.neokey_addr,
    ]
}

#[cfg(any(test, target_os = "linux"))]
fn device_name(address: u16) -> &'static str {
    if ORANGE_PI_ZERO_2W_DEVICES.neokey_addr == address {
        "NeoKey"
    } else {
        "NeoTrellis"
    }
}

#[cfg(any(test, target_os = "linux"))]
fn run_diagnostic<T: I2cOperations>(
    bus: &mut T,
    reset_delay: Duration,
    deadline: Instant,
    mut interrupted: impl FnMut() -> bool,
) -> Result<Vec<(u16, u8)>, String> {
    for address in device_addresses() {
        ensure_safe(deadline, &mut interrupted, "before reset")?;
        bus.write(address, &[STATUS_BASE, SW_RESET, 0xFF])
            .map_err(|error| {
                format!(
                    "{} reset failed at {address:#04x}: {error}",
                    device_name(address)
                )
            })?;
        ensure_safe(deadline, &mut interrupted, "after reset")?;
    }
    sleep_reset_delay_until(deadline, reset_delay, &mut interrupted)?;

    let mut ids = Vec::with_capacity(device_addresses().len());
    for address in device_addresses() {
        let mut id = [0_u8; 1];
        ensure_safe(deadline, &mut interrupted, "before HW ID read")?;
        bus.write_read(address, &[STATUS_BASE, HW_ID], &mut id)
            .map_err(|error| {
                format!(
                    "{} HW ID read failed at {address:#04x}: {error}",
                    device_name(address)
                )
            })?;
        ensure_safe(deadline, &mut interrupted, "after HW ID read")?;
        if !is_valid_hw_id(id[0]) {
            return Err(format!(
                "{} HW ID invalid at {address:#04x}: {:#04x}",
                device_name(address),
                id[0]
            ));
        }
        ids.push((address, id[0]));
    }
    Ok(ids)
}

#[cfg(any(test, target_os = "linux"))]
fn is_valid_hw_id(id: u8) -> bool {
    VALID_HW_IDS.contains(&id)
}

#[cfg(target_os = "linux")]
struct LinuxI2cBus {
    file: std::fs::File,
}

#[cfg(target_os = "linux")]
impl LinuxI2cBus {
    fn open() -> Result<Self, String> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(I2C_PATH)
            .map_err(|error| format!("I2C open failed for {I2C_PATH}: {error}"))?;
        Ok(Self { file })
    }

    fn select_address(&self, address: u16) -> Result<(), String> {
        let result = unsafe {
            libc::ioctl(
                std::os::fd::AsRawFd::as_raw_fd(&self.file),
                0x0703,
                address as u64,
            )
        };
        if result < 0 {
            return Err(format!(
                "I2C slave select failed at {address:#04x}: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl I2cOperations for LinuxI2cBus {
    fn write(&mut self, address: u16, data: &[u8]) -> Result<(), String> {
        use std::io::Write;

        self.select_address(address)?;
        self.file
            .write_all(data)
            .map_err(|error| format!("I2C write failed at {address:#04x}: {error}"))
    }

    fn write_read(
        &mut self,
        address: u16,
        write_data: &[u8],
        read_data: &mut [u8],
    ) -> Result<(), String> {
        use std::io::{Read, Write};

        self.select_address(address)?;
        self.file
            .write_all(write_data)
            .map_err(|error| format!("I2C command write failed at {address:#04x}: {error}"))?;
        std::thread::sleep(Duration::from_millis(1));
        self.file
            .read_exact(read_data)
            .map_err(|error| format!("I2C read failed at {address:#04x}: {error}"))
    }
}

#[cfg(target_os = "linux")]
fn run_smoke_test() -> Result<(), String> {
    install_interrupt_handlers()?;
    let deadline = Instant::now() + OPERATION_BUDGET;
    let mut interrupted = || INTERRUPTED.load(Ordering::SeqCst);
    ensure_safe(deadline, &mut interrupted, "before I2C bus open")?;
    let mut bus = LinuxI2cBus::open()?;
    ensure_safe(deadline, &mut interrupted, "after I2C bus open")?;
    let ids = run_diagnostic(&mut bus, RESET_DELAY, deadline, &mut interrupted)?;
    for (address, id) in ids {
        println!(
            "{} at {address:#04x}: HW ID {id:#04x}",
            device_name(address)
        );
    }
    println!("Orange Seesaw smoke test completed");
    Ok(())
}

#[cfg(target_os = "linux")]
fn install_interrupt_handlers() -> Result<(), String> {
    unsafe {
        let handler = interrupt_handler as *const () as libc::sighandler_t;
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if libc::signal(signal, handler) == libc::SIG_ERR {
                return Err(format!(
                    "could not install Seesaw diagnostic handler for signal {signal}"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
extern "C" fn interrupt_handler(_: libc::c_int) {
    INTERRUPTED.store(true, Ordering::SeqCst);
}

#[cfg(test)]
#[path = "tests/orange_seesaw_smoke_tests.rs"]
mod tests;
