use cellsymphony_hal::OledSsd1351;
use std::thread;
use std::time::Duration;

const WIDTH: usize = 128;
const HEIGHT: usize = 128;
const BYTES_PER_PIXEL: usize = 2;
const FRAME_BYTES: usize = WIDTH * HEIGHT * BYTES_PER_PIXEL;
const BOOT_SPLASH_ATTEMPTS: usize = 12;
const BOOT_SPLASH_RETRY_DELAY: Duration = Duration::from_millis(75);

pub fn requested() -> bool {
    std::env::args().skip(1).any(|arg| {
        arg == "--oled-test"
            || arg == "--oled-all-on"
            || arg == "--boot-splash-once"
            || arg == "--oled-off-once"
    })
}

pub fn run() -> bool {
    if std::env::args()
        .skip(1)
        .any(|arg| arg == "--boot-splash-once")
    {
        return run_boot_splash_once();
    }
    if std::env::args().skip(1).any(|arg| arg == "--oled-off-once") {
        return run_oled_off_once();
    }
    println!("Cell Symphony OLED persistent test pattern");
    let mut oled = match OledSsd1351::new() {
        Ok(oled) => oled,
        Err(error) => {
            eprintln!("FAIL OLED init failed: {error}");
            return false;
        }
    };
    if std::env::args().skip(1).any(|arg| arg == "--oled-all-on") {
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
    match OledSsd1351::new() {
        Ok(mut oled) => oled.display_off().is_ok(),
        Err(error) => {
            eprintln!("FAIL OLED off init failed: {error}");
            false
        }
    }
}

fn run_boot_splash_once() -> bool {
    let mut last_error = String::new();
    for _ in 0..BOOT_SPLASH_ATTEMPTS {
        match OledSsd1351::new() {
            Ok(mut oled) => {
                crate::render::render_boot_splash(&mut oled);
                return true;
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
        return 0xffff;
    }
    if x == y || x == WIDTH - 1 - y {
        return 0xffff;
    }
    match x / 16 {
        0 => 0xf800,
        1 => 0x07e0,
        2 => 0x001f,
        3 => 0xffe0,
        4 => 0xf81f,
        5 => 0x07ff,
        6 => 0xffff,
        _ => 0x39e7,
    }
}
