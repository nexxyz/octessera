use octessera_hal::OledSsd1351;
use platform_core::palette;
use std::thread;
use std::time::{Duration, Instant};

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
        OledUtility::BootSplashStatic => return run_boot_splash_static(),
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

fn open_oled_legacy() -> Result<OledSsd1351, String> {
    OledSsd1351::new()
}

fn run_boot_splash_static() -> bool {
    let result = (|| -> Result<(), String> {
        let mut oled = OledSsd1351::new()?;
        oled.display_on()?;
        oled.write_frame(&crate::render::boot_sweep_base_frame())?;
        Ok(())
    })();
    match result {
        Ok(()) => true,
        Err(error) => {
            eprintln!("FAIL OLED static boot splash failed: {error}");
            false
        }
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
    let result = (|| -> Result<(), String> {
        let mut oled = OledSsd1351::new()?;
        oled.display_on()?;
        let frames = crate::render::boot_sweep_frames();
        let clean_frame = crate::render::boot_sweep_base_frame();
        let mut frame_index = 0;
        let mut cycle_start = Instant::now();
        loop {
            sleep_until(crate::render::boot_sweep_deadline(cycle_start, frame_index));
            oled.write_frame(&frames[frame_index])?;
            let stop_requested = handoff.stop_requested()?;
            if frame_index == crate::render::BOOT_SWEEP_FRAMES - 1 {
                if stop_requested {
                    break Ok(());
                }
                let cycle_end =
                    cycle_start + Duration::from_nanos(crate::render::BOOT_SWEEP_CYCLE_NS);
                sleep_until(cycle_end);
                oled.write_frame(&clean_frame)?;
                if handoff.stop_requested()? {
                    break Ok(());
                }
                let (next_cycle_start, next_frame_index) =
                    advance_boot_sweep(cycle_start, frame_index);
                if sleep_boot_sweep_rest(&mut handoff, cycle_end)? {
                    break Ok(());
                }
                handoff.publish_cycle()?;
                cycle_start = next_cycle_start;
                frame_index = next_frame_index;
            } else if stop_requested {
                break Ok(());
            } else {
                let (_, next_frame_index) = advance_boot_sweep(cycle_start, frame_index);
                frame_index = next_frame_index;
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

fn sleep_boot_sweep_rest(
    handoff: &mut crate::boot_oled_handoff::AnimatorHandoff,
    rest_start: Instant,
) -> Result<bool, String> {
    let rest_deadline = rest_start + Duration::from_nanos(crate::render::BOOT_SWEEP_REST_NS);
    loop {
        if handoff.stop_requested()? {
            return Ok(true);
        }
        let now = Instant::now();
        if now >= rest_deadline {
            return Ok(false);
        }
        let check_deadline = now + Duration::from_nanos(crate::render::BOOT_SWEEP_REST_CHECK_NS);
        sleep_until(check_deadline.min(rest_deadline));
    }
}

fn sleep_until(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if !remaining.is_zero() {
        thread::sleep(remaining);
    }
}

fn advance_boot_sweep(cycle_start: Instant, frame_index: usize) -> (Instant, usize) {
    if frame_index == crate::render::BOOT_SWEEP_FRAMES - 1 {
        (
            cycle_start
                + Duration::from_nanos(
                    crate::render::BOOT_SWEEP_CYCLE_NS + crate::render::BOOT_SWEEP_REST_NS,
                ),
            0,
        )
    } else {
        (cycle_start, frame_index + 1)
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
    use super::{advance_boot_sweep, parse_utility_args, OledUtility, FRAME_BYTES};
    use std::time::{Duration, Instant};

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

    #[test]
    fn static_boot_splash_writes_one_clean_frame_without_runtime_waits() {
        let source = include_str!("oled_test.rs");
        let static_body = source
            .split_once("fn run_boot_splash_static()")
            .and_then(|(_, body)| body.split_once("fn run_boot_splash_loop()"))
            .map(|(body, _)| body)
            .expect("static boot splash body");
        assert_eq!(crate::render::boot_sweep_base_frame().len(), FRAME_BYTES);
        assert_eq!(static_body.matches("write_frame").count(), 1);
        assert!(!static_body.contains("thread::sleep"));
        assert!(!static_body.contains("animator_start"));
        assert!(!static_body.contains("marker"));
    }

    #[test]
    fn boot_sweep_wrap_advances_frame_zero_after_the_exact_sweep_and_rest() {
        let cycle_start = Instant::now();
        let (next_cycle_start, next_frame) =
            advance_boot_sweep(cycle_start, crate::render::BOOT_SWEEP_FRAMES - 1);
        assert_eq!(next_frame, 0);
        assert_eq!(
            next_cycle_start.duration_since(cycle_start),
            Duration::from_nanos(3_200_000_000)
        );
        assert!(next_cycle_start.duration_since(cycle_start) > Duration::from_secs(3));
        assert_eq!(
            crate::render::boot_sweep_deadline(next_cycle_start, next_frame),
            next_cycle_start
        );
    }
}
