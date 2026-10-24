use octessera_hal::OledSsd1351;
use std::thread;
use std::time::{Duration, Instant};

const NATIVE_HANDOFF_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) fn run_boot_splash_static() -> bool {
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

pub(super) fn run_boot_splash_loop() -> bool {
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
    let handoff_deadline = Instant::now() + NATIVE_HANDOFF_TIMEOUT;
    let result = (|| -> Result<(), String> {
        let mut oled = OledSsd1351::new()?;
        oled.display_on()?;
        let frames = crate::render::boot_sweep_frames();
        let clean_frame = crate::render::boot_sweep_base_frame();
        let startup_delayed_frame = crate::boot_startup_delayed::render_startup_delayed_frame();
        let mut frame_index = 0;
        let mut cycle_start = Instant::now();
        loop {
            sleep_until(
                crate::render::boot_sweep_deadline(cycle_start, frame_index).min(handoff_deadline),
            );
            if Instant::now() >= handoff_deadline {
                render_startup_delayed_and_wait(&mut oled, &mut handoff, &startup_delayed_frame)?;
                break Ok(());
            }
            oled.write_frame(&frames[frame_index])?;
            let stop_requested = handoff.stop_requested()?;
            if frame_index == crate::render::BOOT_SWEEP_FRAMES - 1 {
                if stop_requested {
                    break Ok(());
                }
                let cycle_end =
                    cycle_start + Duration::from_nanos(crate::render::BOOT_SWEEP_CYCLE_NS);
                sleep_until(cycle_end.min(handoff_deadline));
                if handoff.stop_requested()? {
                    break Ok(());
                }
                if Instant::now() >= handoff_deadline {
                    render_startup_delayed_and_wait(
                        &mut oled,
                        &mut handoff,
                        &startup_delayed_frame,
                    )?;
                    break Ok(());
                }
                oled.write_frame(&clean_frame)?;
                if handoff.stop_requested()? {
                    break Ok(());
                }
                let (next_cycle_start, next_frame_index) =
                    advance_boot_sweep(cycle_start, frame_index);
                if sleep_boot_sweep_rest(&mut handoff, cycle_end, handoff_deadline)? {
                    break Ok(());
                }
                if Instant::now() >= handoff_deadline {
                    render_startup_delayed_and_wait(
                        &mut oled,
                        &mut handoff,
                        &startup_delayed_frame,
                    )?;
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
    handoff_deadline: Instant,
) -> Result<bool, String> {
    let rest_deadline = rest_start + Duration::from_nanos(crate::render::BOOT_SWEEP_REST_NS);
    loop {
        if handoff.stop_requested()? {
            return Ok(true);
        }
        let now = Instant::now();
        if now >= handoff_deadline {
            return Ok(false);
        }
        if now >= rest_deadline {
            return Ok(false);
        }
        let check_deadline = now + Duration::from_nanos(crate::render::BOOT_SWEEP_REST_CHECK_NS);
        sleep_until(check_deadline.min(rest_deadline).min(handoff_deadline));
    }
}

fn render_startup_delayed_and_wait(
    oled: &mut OledSsd1351,
    handoff: &mut crate::boot_oled_handoff::AnimatorHandoff,
    frame: &[u8],
) -> Result<(), String> {
    if handoff.stop_requested()? {
        return Ok(());
    }
    oled.write_frame(frame)?;
    loop {
        if handoff.stop_requested()? {
            return Ok(());
        }
        thread::sleep(Duration::from_nanos(
            crate::render::BOOT_SWEEP_REST_CHECK_NS,
        ));
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

#[cfg(test)]
mod tests {
    use super::super::FRAME_BYTES;
    use super::advance_boot_sweep;
    use std::time::{Duration, Instant};

    #[test]
    fn static_boot_splash_writes_one_clean_frame_without_runtime_waits() {
        let source = include_str!("boot_splash.rs");
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
    fn boot_loop_uses_one_startup_delayed_frame_and_keeps_polling_handoff() {
        let source = include_str!("boot_splash.rs");
        let loop_body = source
            .split_once("fn run_boot_splash_loop()")
            .and_then(|(_, body)| body.split_once("fn sleep_boot_sweep_rest("))
            .map(|(body, _)| body)
            .expect("boot splash loop body");
        assert!(loop_body.contains("NATIVE_HANDOFF_TIMEOUT"));
        assert!(loop_body.contains("boot_startup_delayed::render_startup_delayed_frame"));
        assert_eq!(
            loop_body.matches("render_startup_delayed_and_wait").count(),
            3
        );
        let startup_delayed_body = source
            .split_once("fn render_startup_delayed_and_wait(")
            .and_then(|(_, body)| body.split_once("fn sleep_until("))
            .map(|(body, _)| body)
            .expect("startup delayed wait body");
        assert!(startup_delayed_body.contains("thread::sleep(Duration::from_nanos("));
        assert!(!loop_body.contains("display_off"));
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
