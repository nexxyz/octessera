use std::env;
use std::process;

#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::time::Instant;

#[cfg(target_os = "linux")]
use octessera_hal::orange_hardware::OrangeHardware;
use octessera_hal::orange_metadata::print_build_metadata;

#[cfg(any(test, target_os = "linux"))]
const DISPLAY_WIDTH: usize = 128;
#[cfg(any(test, target_os = "linux"))]
const DISPLAY_HEIGHT: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    confirm_active_test: bool,
    print_build_metadata: bool,
}

#[cfg(target_os = "linux")]
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

fn main() {
    let options = match parse_args(env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: orange-oled-smoke --confirm-active-test | --print-build-metadata");
            process::exit(2);
        }
    };

    if options.print_build_metadata {
        if let Err(error) = print_build_metadata() {
            eprintln!("Orange OLED build metadata check failed: {error}");
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
        eprintln!("Orange OLED smoke requires a Linux target with the real Orange HAL");
        process::exit(2);
    }

    #[cfg(target_os = "linux")]
    if let Err(error) = run_smoke_test() {
        eprintln!("Orange OLED smoke failed: {error}");
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
        Err("refusing active OLED test without --confirm-active-test")
    }
}

#[cfg(target_os = "linux")]
fn run_smoke_test() -> Result<(), String> {
    install_interrupt_handlers()?;
    let operation_deadline = octessera_hal::orange_timing::operation_deadline();
    let hardware = OrangeHardware::open_until(operation_deadline)?;
    let mut session = DiagnosticSession {
        hardware,
        operation_deadline,
    };
    session.run()
}

#[cfg(target_os = "linux")]
struct DiagnosticSession {
    hardware: OrangeHardware,
    operation_deadline: Instant,
}

#[cfg(target_os = "linux")]
impl DiagnosticSession {
    fn run(&mut self) -> Result<(), String> {
        self.check_safety_bound()?;
        self.hardware
            .oled_mut()
            .write_frame_until(&static_test_pattern(), self.operation_deadline)?;
        self.check_safety_bound()?;
        self.hardware
            .oled_mut()
            .shutdown_until(&black_frame(), self.operation_deadline)?;
        self.check_safety_bound()?;
        println!("Orange OLED smoke test completed");
        Ok(())
    }

    fn check_safety_bound(&self) -> Result<(), String> {
        ensure_not_interrupted(INTERRUPTED.load(Ordering::SeqCst))?;
        octessera_hal::orange_timing::ensure_before_deadline(self.operation_deadline)
    }
}

#[cfg(target_os = "linux")]
fn install_interrupt_handlers() -> Result<(), String> {
    unsafe {
        let handler = interrupt_handler as *const () as libc::sighandler_t;
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            if libc::signal(signal, handler) == libc::SIG_ERR {
                return Err(format!(
                    "could not install OLED cleanup handler for signal {signal}"
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

#[cfg(any(test, target_os = "linux"))]
fn ensure_not_interrupted(interrupted: bool) -> Result<(), &'static str> {
    if interrupted {
        Err("Orange OLED smoke interrupted; cleanup is being attempted")
    } else {
        Ok(())
    }
}

#[cfg(any(test, target_os = "linux"))]
fn static_test_pattern() -> Vec<u8> {
    let mut pattern = Vec::with_capacity(DISPLAY_WIDTH * DISPLAY_HEIGHT * 2);
    for y in 0..DISPLAY_HEIGHT {
        for x in 0..DISPLAY_WIDTH {
            let red = (x * 31 / DISPLAY_WIDTH) as u16;
            let green = (y * 63 / DISPLAY_HEIGHT) as u16;
            let blue = ((x + y) * 31 / (DISPLAY_WIDTH + DISPLAY_HEIGHT)) as u16;
            let pixel = (red << 11) | (green << 5) | blue;
            pattern.extend_from_slice(&pixel.to_be_bytes());
        }
    }
    pattern
}

#[cfg(any(test, target_os = "linux"))]
fn black_frame() -> Vec<u8> {
    vec![0; DISPLAY_WIDTH * DISPLAY_HEIGHT * 2]
}

#[cfg(test)]
#[path = "tests/orange_oled_smoke_tests.rs"]
mod tests;
