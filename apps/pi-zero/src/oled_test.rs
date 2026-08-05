use octessera_hal::OledSsd1351;
use platform_core::palette;
use std::thread;
use std::time::{Duration, Instant};

const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const BYTES_PER_PIXEL: usize = 2;
const FRAME_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;
const BOOT_SPLASH_ATTEMPTS: usize = 12;
const BOOT_SPLASH_RETRY_DELAY: Duration = Duration::from_millis(75);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OledUtility {
    Test,
    AllOn,
    OffOnce,
    BootSplashOnce,
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
        "--boot-splash-once" => Ok(Some(OledUtility::BootSplashOnce)),
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
        OledUtility::BootSplashOnce => return run_boot_splash_once(),
        OledUtility::BootSplashLoop => return run_boot_splash_loop(),
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
    let mut oled = match open_oled_legacy() {
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
    match open_oled_legacy() {
        Ok(mut oled) => oled.display_off().is_ok(),
        Err(error) => {
            eprintln!("FAIL OLED off init failed: {error}");
            false
        }
    }
}

fn run_boot_splash_once() -> bool {
    if std::env::var("OCTESSERA_INITRAMFS_BOOT_SPLASH").as_deref() != Ok("1") {
        eprintln!("FAIL --boot-splash-once requires OCTESSERA_INITRAMFS_BOOT_SPLASH=1");
        return false;
    }
    let mut last_error = String::new();
    for _ in 0..BOOT_SPLASH_ATTEMPTS {
        match OledSsd1351::new() {
            Ok(mut oled) => {
                return crate::render::render_boot_splash(&mut oled).is_ok();
            }
            Err(error) => {
                last_error = error;
                thread::sleep(BOOT_SPLASH_RETRY_DELAY);
            }
        }
    }
    eprintln!("FAIL OLED boot splash init failed: {last_error}");
    false
}

fn open_oled_legacy() -> Result<OledSsd1351, String> {
    if std::env::var("OCTESSERA_EARLY_BOOT_SPLASH").as_deref() == Ok("1") {
        OledSsd1351::adopt_existing()
    } else {
        OledSsd1351::new()
    }
}

fn run_boot_splash_loop() -> bool {
    if crate::boot_oled_handoff::mode_from_env() != Ok(crate::boot_oled_handoff::HandoffMode::V1) {
        eprintln!(
            "FAIL --boot-splash-loop requires {}=v1",
            crate::boot_oled_handoff::HANDOFF_ENV
        );
        return false;
    }
    let mut handoff = match crate::boot_oled_handoff::animator_start() {
        Ok(handoff) => handoff,
        Err(error) => {
            eprintln!("FAIL OLED boot handoff init failed: {error}");
            return false;
        }
    };
    let adopt_existing = match crate::boot_oled_handoff::validate_initramfs_marker_if_present() {
        Ok(adopt_existing) => adopt_existing,
        Err(error) => {
            handoff.mark_failed();
            eprintln!("FAIL initramfs OLED marker validation failed: {error}");
            return false;
        }
    };
    let result = (|| -> Result<(), String> {
        let mut oled = if adopt_existing {
            OledSsd1351::adopt_existing()?
        } else {
            OledSsd1351::new()?
        };
        oled.display_on()?;
        let mut frame_index = 0;
        let mut cycle_start = Instant::now();
        loop {
            sleep_until(crate::render::boot_sweep_deadline(cycle_start, frame_index));
            crate::render::render_boot_splash_frame(&mut oled, frame_index)?;
            let stop_requested = handoff.stop_requested()?;
            if frame_index == 23 {
                if stop_requested {
                    break Ok(());
                }
                let next_cycle_start = cycle_start + Duration::from_secs(1);
                sleep_until(next_cycle_start);
                handoff.publish_cycle()?;
                cycle_start = next_cycle_start;
                frame_index = 0;
            } else if stop_requested {
                break Ok(());
            } else {
                frame_index += 1;
            }
        }
    })();
    if result.is_err() {
        handoff.mark_failed();
    }
    match result.and_then(|()| handoff.release()) {
        Ok(()) => true,
        Err(error) => {
            eprintln!("FAIL OLED loop handoff release failed: {error}");
            false
        }
    }
}

fn sleep_until(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        thread::sleep(remaining);
    }
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
        0 => palette::RED_RGB565,
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
            ("--boot-splash-once", OledUtility::BootSplashOnce),
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
            &["--oled-test", "--other"][..],
            &["--unknown"][..],
        ] {
            assert!(parse_utility_args(args).is_err(), "args={args:?}");
        }
    }
}
