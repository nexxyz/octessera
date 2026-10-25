use octessera_hal::OledSsd1351;

mod boot_splash;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OledUtility {
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
        "--oled-off-once" => Ok(Some(OledUtility::OffOnce)),
        "--boot-splash-static" => Ok(Some(OledUtility::BootSplashStatic)),
        "--boot-splash-loop" => Ok(Some(OledUtility::BootSplashLoop)),
        _ => Err(format!("unsupported OLED utility argument {argument:?}")),
    }
}

pub fn requested() -> bool {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    args.iter()
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
        OledUtility::OffOnce => {}
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
    run_oled_off_once()
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

#[cfg(test)]
mod cli_tests {
    use super::{parse_utility_args, OledUtility};

    #[test]
    fn utility_cli_parser_accepts_exactly_one_known_mode() {
        for (argument, utility) in [
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
            &["--oled-off-once", "--oled-off-once"][..],
            &["--oled-off-once", "--boot-splash-loop"][..],
            &["--boot-splash-static", "--boot-splash-loop"][..],
            &["--oled-off-once", "--other"][..],
            &["--oled-test"][..],
            &["--boot-splash-once"][..],
            &["--unknown"][..],
        ] {
            assert!(parse_utility_args(args).is_err(), "args={args:?}");
        }
    }
}
