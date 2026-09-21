use super::{NativeRunner, NativeXyGlide, DEFAULT_XY_SMOOTHING_MS};
use std::time::{Duration, Instant};

pub(super) fn normalize_xy_smoothing_ms(value: u64) -> u16 {
    if value == 0 || (10..=500).contains(&value) && value.is_multiple_of(10) {
        value as u16
    } else {
        DEFAULT_XY_SMOOTHING_MS
    }
}

impl NativeRunner {
    pub fn next_xy_glide_deadline(&self) -> Option<Instant> {
        [
            self.xy_x_glide
                .as_ref()
                .and_then(|glide| glide.started_at.checked_add(glide.duration)),
            self.xy_y_glide
                .as_ref()
                .and_then(|glide| glide.started_at.checked_add(glide.duration)),
        ]
        .into_iter()
        .flatten()
        .min()
    }

    pub(super) fn retarget_xy_runtime_sources_at(&mut self, now: Instant, force: bool) -> bool {
        let targets = [
            if self.xy_invert_x {
                1.0 - self.xy_touch.display_x
            } else {
                self.xy_touch.display_x
            },
            if self.xy_invert_y {
                1.0 - self.xy_touch.display_y
            } else {
                self.xy_touch.display_y
            },
        ];
        self.retarget_xy_axis(true, targets[0], now, force);
        self.retarget_xy_axis(false, targets[1], now, force);
        if force {
            self.set_xy_runtime_sources_forced([self.xy_touch.x, self.xy_touch.y])
        } else {
            self.set_xy_runtime_sources([self.xy_touch.x, self.xy_touch.y])
        }
    }

    pub(super) fn set_xy_smoothing_ms_at(&mut self, value: u64, now: Instant) -> bool {
        self.xy_smoothing_ms = normalize_xy_smoothing_ms(value);
        self.retarget_xy_runtime_sources_at(now, true)
    }

    pub(super) fn advance_xy_smoothing_at(&mut self, now: Instant) -> Result<(), String> {
        let x_changed = advance_xy_axis(&mut self.xy_touch.x, &mut self.xy_x_glide, now);
        let y_changed = advance_xy_axis(&mut self.xy_touch.y, &mut self.xy_y_glide, now);
        if !(x_changed || y_changed) {
            return Ok(());
        }
        if self.set_xy_runtime_sources([self.xy_touch.x, self.xy_touch.y]) {
            self.process_dirty_modulation_step(true)?;
        }
        Ok(())
    }

    fn retarget_xy_axis(&mut self, x_axis: bool, target: f32, now: Instant, force: bool) {
        let smoothing_ms = self.xy_smoothing_ms;
        let (value, glide) = if x_axis {
            (&mut self.xy_touch.x, &mut self.xy_x_glide)
        } else {
            (&mut self.xy_touch.y, &mut self.xy_y_glide)
        };
        let target_unchanged = glide
            .as_ref()
            .map(|current| (current.target - target).abs() <= f32::EPSILON)
            .unwrap_or_else(|| (*value - target).abs() <= f32::EPSILON);
        let duration_changed = glide.as_ref().is_some_and(|current| {
            current.duration != Duration::from_millis(u64::from(smoothing_ms))
        });
        if !force && target_unchanged && !duration_changed {
            return;
        }
        advance_xy_axis(value, glide, now);
        if smoothing_ms == 0 || (*value - target).abs() <= f32::EPSILON {
            *value = target;
            *glide = None;
        } else {
            *glide = Some(NativeXyGlide {
                from: *value,
                target,
                started_at: now,
                duration: Duration::from_millis(u64::from(smoothing_ms)),
            });
        }
    }
}

fn advance_xy_axis(value: &mut f32, glide: &mut Option<NativeXyGlide>, now: Instant) -> bool {
    let Some(current) = glide.as_ref() else {
        return false;
    };
    let current = current.clone();
    if current.duration.is_zero() {
        let changed = *value != current.target;
        *value = current.target;
        *glide = None;
        return changed;
    }
    let elapsed = now
        .checked_duration_since(current.started_at)
        .unwrap_or_default();
    let progress = (elapsed.as_secs_f64() / current.duration.as_secs_f64()).clamp(0.0, 1.0);
    let next = current.from + (current.target - current.from) * progress as f32;
    let changed = *value != next;
    *value = next;
    if progress >= 1.0 {
        *value = current.target;
        *glide = None;
    }
    changed
}
