use octessera_hal::OledSsd1351;
use platform_core::palette;
use std::thread;
use std::time::Duration;

mod boot_splash;

const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const BYTES_PER_PIXEL: usize = 2;
const FRAME_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OledUtility {
    Test,
    AllOn,
    OffOnce,
    BootSplashStatic,
    BootSplashLoop,
}

fn parse_utility_args(args: &[&str]) -> Result<Option<OledUtility>, String> {
    let Some(argument) = args.first() else {
        return Ok(None);
    };
    if args.len() != 1 {
        return Err("exactly one OLED utility argument is required".into());
    }
    match *argument {
        "--oled-test" => Ok(Some(OledUtility::Test)),
        "--oled-all-on" => Ok(Some(OledUtility::AllOn)),
        "--oled-off-once" => Ok(Some(OledUtility::OffOnce)),
        "--boot-splash-static" => Ok(Some(OledUtility::BootSplashStatic)),
        "--boot-splash-loop" => Ok(Some(OledUtility::BootSplashLoop)),
        _ => Err(format!("unsupported OLED utility argument {argument:?}")),
    }
}

pub fn requested() -> bool {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    args.iter()
        .any(|arg| arg == "--oled-test" || arg == "--oled-all-on" || arg == "--oled-off-once")
        || args
            .iter()
            .any(|arg| arg.starts_with("--oled-") || arg.starts_with("--boot-splash-"))
}

pub fn run() -> bool {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let utility = match parse_utility_args(&args.iter().map(String::as_str).collect::<Vec<_>>()) {
        Ok(Some(utility)) => utility,
        Ok(None) => return false,
        Err(error) => {
            eprintln!("FAIL OLED utility arguments: {error}");
            return false;
        }
    };
    match utility {
        OledUtility::BootSplashStatic => return boot_splash::run_boot_splash_static(),
        OledUtility::BootSplashLoop => return boot_splash::run_boot_splash_loop(),
        OledUtility::Test | OledUtility::AllOn | OledUtility::OffOnce => {}
    }
    let _utility_lock = if crate::boot_oled_handoff::mode_from_env()
        == Ok(crate::boot_oled_handoff::HandoffMode::V1)
    {
        match crate::boot_oled_handoff::utility_lock() {
            Ok(lock) => Some(lock),
            Err(error) => {
                eprintln!("FAIL OLED utility lock failed: {error}");
                return false;
            }
        }
    } else {
        None
    };
    if utility == OledUtility::OffOnce {
        return run_oled_off_once();
    }
    println!("octessera OLED persistent test pattern");
    let mut oled = match open_oled_for_diagnostic() {
        Ok(oled) => oled,
        Err(error) => {
            eprintln!("FAIL OLED init failed: {error}");
            return false;
        }
    };
    if utility == OledUtility::AllOn {
        return run_all_on(oled);
    }
    let frame = test_frame();
    match oled.write_frame(&frame) {
        Ok(()) => {
            println!("PASS OLED frame written; pattern will remain until process exits or display is overwritten");
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }
        Err(error) => {
            eprintln!("FAIL OLED frame write failed: {error}");
            false
        }
    }
}

fn run_oled_off_once() -> bool {
    match open_oled_for_diagnostic() {
        Ok(mut oled) => oled.display_off().is_ok(),
        Err(error) => {
            eprintln!("FAIL OLED off init failed: {error}");
            false
        }
    }
}

fn open_oled_for_diagnostic() -> Result<OledSsd1351, String> {
    OledSsd1351::new()
}

fn run_all_on(mut oled: OledSsd1351) -> bool {
    match oled.display_all_on() {
        Ok(()) => {
            println!("PASS OLED display-all-on command written; command will remain active until process exits or display is overwritten");
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }
        Err(error) => {
            eprintln!("FAIL OLED display-all-on failed: {error}");
            false
        }
    }
}

fn test_frame() -> Vec<u8> {
    let mut frame = vec![0_u8; FRAME_BYTES];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let color = color_at(x, y);
            let offset = (y * WIDTH + x) * BYTES_PER_PIXEL;
            frame[offset] = (color >> 8) as u8;
            frame[offset + 1] = color as u8;
        }
    }
    frame
}

fn color_at(x: usize, y: usize) -> u16 {
    if x == 0 || y == 0 || x == WIDTH - 1 || y == HEIGHT - 1 {
        return palette::WHITE_RGB565;
    }
    if x == y || x == WIDTH - 1 - y {
        return palette::WHITE_RGB565;
    }
    match x / 16 {
        0 => palette::MAGENTA_RGB565,
        1 => palette::GREEN_RGB565,
        2 => palette::BLUE_RGB565,
        3 => palette::YELLOW_RGB565,
        4 => palette::GRAY_RGB565,
        5 => palette::WHITE_RGB565,
        6 => palette::BLACK_RGB565,
        _ => palette::GRAY_RGB565,
    }
}

#[cfg(test)]
mod cli_tests {
    use super::{parse_utility_args, OledUtility};

    #[test]
    fn utility_cli_parser_accepts_exactly_one_known_mode() {
        for (argument, utility) in [
            ("--oled-test", OledUtility::Test),
            ("--oled-all-on", OledUtility::AllOn),
            ("--oled-off-once", OledUtility::OffOnce),
            ("--boot-splash-static", OledUtility::BootSplashStatic),
            ("--boot-splash-loop", OledUtility::BootSplashLoop),
        ] {
            assert_eq!(parse_utility_args(&[argument]), Ok(Some(utility)));
        }
        assert_eq!(parse_utility_args(&[]), Ok(None));
    }

    #[test]
    fn utility_cli_parser_rejects_duplicates_mixed_modes_and_unrelated_args() {
        for args in [
            &["--oled-test", "--oled-test"][..],
            &["--oled-test", "--boot-splash-loop"][..],
            &["--boot-splash-static", "--boot-splash-loop"][..],
            &["--oled-test", "--other"][..],
            &["--boot-splash-once"][..],
            &["--unknown"][..],
        ] {
            assert!(parse_utility_args(args).is_err(), "args={args:?}");
        }
    }
}
