use std::thread::sleep;
use std::time::{Duration, Instant};

pub const PRE_RESET_DELAY_MS: u64 = 250;
pub const RESET_HIGH_MS: u64 = 100;
pub const RESET_LOW_MS: u64 = 100;
pub const RESET_SETTLE_MS: u64 = 250;
pub const POST_DISPLAY_ON_MS: u64 = 100;
pub const SPI_SPEED_HZ: u64 = 16_000_000;
pub const OLED_FRAME_BYTES: usize = 128 * 128 * 2;
pub const DISPLAY_OFF_SPI_BYTES: usize = 1;
pub const FRAME_WRITE_SPI_OVERHEAD_BYTES: usize = 7;
pub const DISPLAY_INITIALIZATION_SPI_BYTES: usize = 45;
pub const OPERATION_BUDGET: Duration = Duration::from_secs(3);
pub const CLEANUP_BUDGET: Duration = Duration::from_secs(1);

pub fn operation_deadline() -> Instant {
    Instant::now() + OPERATION_BUDGET
}

pub fn cleanup_deadline() -> Instant {
    Instant::now() + CLEANUP_BUDGET
}

pub fn deadline_expired(deadline: Instant) -> bool {
    Instant::now() >= deadline
}

pub fn ensure_before_deadline(deadline: Instant) -> Result<(), String> {
    if deadline_expired(deadline) {
        Err("Orange OLED diagnostic operation exceeded its cooperative budget".into())
    } else {
        Ok(())
    }
}

pub fn spi_transfer_duration(bytes: usize) -> Duration {
    let bits = bytes as u128 * 8;
    let nanos = (bits * 1_000_000_000).div_ceil(SPI_SPEED_HZ as u128);
    Duration::from_nanos(nanos as u64)
}

pub fn reset_delay_duration() -> Duration {
    Duration::from_millis(
        PRE_RESET_DELAY_MS + RESET_HIGH_MS + RESET_LOW_MS + RESET_SETTLE_MS + POST_DISPLAY_ON_MS,
    )
}

pub fn normal_operation_spi_bytes() -> usize {
    let frame_bytes = OLED_FRAME_BYTES + FRAME_WRITE_SPI_OVERHEAD_BYTES;
    DISPLAY_INITIALIZATION_SPI_BYTES + (2 * frame_bytes) + DISPLAY_OFF_SPI_BYTES
}

pub fn fallback_cleanup_spi_bytes() -> usize {
    DISPLAY_OFF_SPI_BYTES + OLED_FRAME_BYTES + FRAME_WRITE_SPI_OVERHEAD_BYTES
}

pub fn normal_operation_minimum() -> Duration {
    reset_delay_duration() + spi_transfer_duration(normal_operation_spi_bytes())
}

pub fn fallback_cleanup_minimum() -> Duration {
    spi_transfer_duration(fallback_cleanup_spi_bytes())
}

pub fn can_admit_sleep(now: Instant, deadline: Instant, duration: Duration) -> bool {
    now.checked_add(duration)
        .is_some_and(|wake_time| wake_time < deadline)
}

pub fn sleep_within_budget(deadline: Instant, duration: Duration) -> Result<(), String> {
    let now = Instant::now();
    if !can_admit_sleep(now, deadline, duration) {
        return Err("Orange OLED sleep would exceed its cooperative budget".into());
    }
    sleep(duration);
    ensure_before_deadline(deadline)
}

#[cfg(test)]
mod tests {
    use super::{
        can_admit_sleep, fallback_cleanup_minimum, fallback_cleanup_spi_bytes,
        normal_operation_minimum, normal_operation_spi_bytes, CLEANUP_BUDGET,
        DISPLAY_OFF_SPI_BYTES, FRAME_WRITE_SPI_OVERHEAD_BYTES, OLED_FRAME_BYTES, OPERATION_BUDGET,
        POST_DISPLAY_ON_MS, PRE_RESET_DELAY_MS, RESET_HIGH_MS, RESET_LOW_MS, RESET_SETTLE_MS,
    };
    use std::time::{Duration, Instant};

    #[test]
    fn budgets_cover_reset_two_frames_and_fallback_off_frame_without_sleeping() {
        assert_eq!(OLED_FRAME_BYTES, 128 * 128 * 2);
        assert_eq!(DISPLAY_OFF_SPI_BYTES, 1);
        assert_eq!(FRAME_WRITE_SPI_OVERHEAD_BYTES, 7);
        assert_eq!(
            normal_operation_spi_bytes(),
            45 + (2 * (OLED_FRAME_BYTES + 7)) + DISPLAY_OFF_SPI_BYTES
        );
        assert_eq!(
            fallback_cleanup_spi_bytes(),
            DISPLAY_OFF_SPI_BYTES + OLED_FRAME_BYTES + FRAME_WRITE_SPI_OVERHEAD_BYTES
        );
        assert_eq!(
            PRE_RESET_DELAY_MS
                + RESET_HIGH_MS
                + RESET_LOW_MS
                + RESET_SETTLE_MS
                + POST_DISPLAY_ON_MS,
            800
        );
        assert!(OPERATION_BUDGET >= normal_operation_minimum());
        assert!(CLEANUP_BUDGET >= fallback_cleanup_minimum());
        assert!(CLEANUP_BUDGET > Duration::from_millis(0));
        assert!(OPERATION_BUDGET > CLEANUP_BUDGET);
    }

    #[test]
    fn sleep_admission_rejects_deadline_overrun_without_sleeping() {
        let now = Instant::now();
        assert!(!can_admit_sleep(
            now,
            now + Duration::from_millis(10),
            Duration::from_millis(10)
        ));
        assert!(!can_admit_sleep(now, now, Duration::ZERO));
        assert!(can_admit_sleep(
            now,
            now + Duration::from_millis(10),
            Duration::from_millis(9)
        ));
    }
}
