use std::env;
use std::process;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

#[cfg(target_os = "linux")]
use octessera_hal::orange_hardware::OrangeHardware;

#[cfg(any(test, target_os = "linux"))]
const DISPLAY_WIDTH: usize = 128;
#[cfg(any(test, target_os = "linux"))]
const DISPLAY_HEIGHT: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Options {
    confirm_active_test: bool,
}

#[cfg(target_os = "linux")]
const OPERATION_TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(target_os = "linux")]
const CLEANUP_TIMEOUT: Duration = Duration::from_millis(500);
#[cfg(target_os = "linux")]
static INTERRUPTED: AtomicBool = AtomicBool::new(false);

fn main() {
    let options = match parse_args(env::args().skip(1)) {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: orange-oled-smoke --confirm-active-test");
            process::exit(2);
        }
    };

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
    let mut confirm_active_test = false;
    for arg in args {
        match arg.into().as_str() {
            "--confirm-active-test" if !confirm_active_test => confirm_active_test = true,
            "--confirm-active-test" => return Err("duplicate --confirm-active-test".into()),
            unknown => return Err(format!("unknown argument: {unknown}")),
        }
    }
    Ok(Options {
        confirm_active_test,
    })
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
    let deadline = Instant::now() + OPERATION_TIMEOUT;
    let hardware = OrangeHardware::open_until(deadline)?;
    let mut session = DiagnosticSession {
        hardware,
        deadline,
        finished: false,
    };
    session.run()
}

#[cfg(target_os = "linux")]
struct DiagnosticSession {
    hardware: OrangeHardware,
    deadline: Instant,
    finished: bool,
}

#[cfg(target_os = "linux")]
impl DiagnosticSession {
    fn run(&mut self) -> Result<(), String> {
        self.check_safety_bound()?;
        self.hardware
            .oled_mut()
            .write_frame_until(&static_test_pattern(), self.deadline)?;
        self.check_safety_bound()?;
        self.hardware
            .oled_mut()
            .write_frame_until(&black_frame(), self.deadline)?;
        self.check_safety_bound()?;
        self.hardware.oled_mut().display_off_until(self.deadline)?;
        self.finished = true;
        println!("Orange OLED smoke test completed");
        Ok(())
    }

    fn check_safety_bound(&self) -> Result<(), String> {
        if INTERRUPTED.load(Ordering::SeqCst) {
            Err("Orange OLED smoke interrupted; cleanup is being attempted".into())
        } else if Instant::now() >= self.deadline {
            Err("Orange OLED smoke exceeded its 2-second safety bound".into())
        } else {
            Ok(())
        }
    }
}

#[cfg(target_os = "linux")]
impl Drop for DiagnosticSession {
    fn drop(&mut self) {
        if self.finished {
            return;
        }
        let cleanup_deadline = Instant::now() + CLEANUP_TIMEOUT;
        let black = black_frame();
        let _ = self
            .hardware
            .oled_mut()
            .write_frame_until(&black, cleanup_deadline);
        let _ = self.hardware.oled_mut().display_off();
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
