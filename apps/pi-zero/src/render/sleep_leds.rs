use super::{scale, sleep_dim_brightness};
use platform_core::palette;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const GRID_LED_COUNT: usize = 64;
const KEY_LED_COUNT: usize = 4;
const MAX_GRID_PULSES: usize = 4;
const MAX_KEY_PULSES: usize = 1;
const ANIMATION_TICK: Duration = Duration::from_millis(100);
const PULSE_MIN: Duration = Duration::from_millis(2_400);
const PULSE_MAX: Duration = Duration::from_millis(4_800);
const GRID_SPAWN_MIN: Duration = Duration::from_millis(350);
const GRID_SPAWN_MAX: Duration = Duration::from_millis(1_400);
const KEY_SPAWN_MIN: Duration = Duration::from_millis(700);
const KEY_SPAWN_MAX: Duration = Duration::from_millis(2_200);

const SLEEP_COLORS: [[u8; 3]; 6] = [
    palette::BLUE,
    palette::GREEN,
    palette::RED,
    palette::YELLOW,
    palette::WHITE,
    palette::GRAY,
];

#[derive(Clone, Copy)]
struct Pulse {
    started_at: Instant,
    duration: Duration,
    color: [u8; 3],
}

#[derive(Clone, Copy)]
pub(crate) struct SleepLedFrames {
    pub(crate) grid: [[u8; 3]; GRID_LED_COUNT],
    pub(crate) keys: [[u8; 3]; KEY_LED_COUNT],
}

pub(crate) struct SleepLedAnimation {
    rng: SmallPrng,
    grid: [Option<Pulse>; GRID_LED_COUNT],
    keys: [Option<Pulse>; KEY_LED_COUNT],
    last_grid_location: Option<usize>,
    last_key_location: Option<usize>,
    next_grid_spawn_at: Option<Instant>,
    next_key_spawn_at: Option<Instant>,
    next_tick_at: Option<Instant>,
    grid_brightness: f32,
    key_brightness: f32,
    active: bool,
}

impl SleepLedAnimation {
    pub(crate) fn new() -> Self {
        Self::with_seed(clock_seed())
    }

    pub(crate) fn with_seed(seed: u64) -> Self {
        Self {
            rng: SmallPrng::new(seed),
            grid: [None; GRID_LED_COUNT],
            keys: [None; KEY_LED_COUNT],
            last_grid_location: None,
            last_key_location: None,
            next_grid_spawn_at: None,
            next_key_spawn_at: None,
            next_tick_at: None,
            grid_brightness: 1.0,
            key_brightness: 1.0,
            active: false,
        }
    }

    pub(crate) fn enter(
        &mut self,
        now: Instant,
        grid_brightness: f32,
        key_brightness: f32,
    ) -> bool {
        self.grid_brightness = grid_brightness;
        self.key_brightness = key_brightness;
        if self.active {
            return false;
        }
        self.active = true;
        self.next_grid_spawn_at = Some(now);
        self.next_key_spawn_at = Some(now);
        self.next_tick_at = None;
        true
    }

    pub(crate) fn stop(&mut self) {
        self.grid.fill(None);
        self.keys.fill(None);
        self.last_grid_location = None;
        self.last_key_location = None;
        self.next_grid_spawn_at = None;
        self.next_key_spawn_at = None;
        self.next_tick_at = None;
        self.active = false;
    }

    pub(crate) fn active(&self) -> bool {
        self.active
    }

    pub(crate) fn frames_at(&mut self, now: Instant) -> SleepLedFrames {
        self.advance(now);
        SleepLedFrames {
            grid: self.render_grid(now),
            keys: self.render_keys(now),
        }
    }

    pub(crate) fn frames_if_due(&mut self, now: Instant) -> Option<SleepLedFrames> {
        if self.next_deadline().is_some_and(|deadline| now >= deadline) {
            Some(self.frames_at(now))
        } else {
            None
        }
    }

    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        if !self.active {
            return None;
        }

        let grid_active = pulse_count(&self.grid);
        let keys_active = pulse_count(&self.keys);
        if grid_active != 0 || keys_active != 0 {
            return self.next_tick_at;
        }
        earlier(self.next_grid_spawn_at, self.next_key_spawn_at)
    }

    #[cfg(test)]
    pub(crate) fn key_pulse_windows(&self) -> Vec<(usize, Instant, Instant)> {
        pulse_windows(&self.keys)
    }

    fn advance(&mut self, now: Instant) {
        if !self.active {
            return;
        }
        expire_pulses(&mut self.grid, now);
        expire_pulses(&mut self.keys, now);
        self.spawn_grid_if_due(now);
        self.spawn_key_if_due(now);
        if pulse_count(&self.grid) != 0 || pulse_count(&self.keys) != 0 {
            if self.next_tick_at.is_none_or(|deadline| now >= deadline) {
                self.next_tick_at = Some(now + ANIMATION_TICK);
            }
        } else {
            self.next_tick_at = None;
        }
    }

    fn spawn_grid_if_due(&mut self, now: Instant) {
        if self
            .next_grid_spawn_at
            .is_none_or(|deadline| now < deadline)
        {
            return;
        }
        if pulse_count(&self.grid) < MAX_GRID_PULSES {
            if let Some(location) =
                next_location(&mut self.rng, &self.grid, self.last_grid_location)
            {
                self.grid[location] = Some(self.new_pulse(now));
                self.last_grid_location = Some(location);
            }
        }
        self.next_grid_spawn_at = Some(now + self.spawn_delay(GRID_SPAWN_MIN, GRID_SPAWN_MAX));
    }

    fn spawn_key_if_due(&mut self, now: Instant) {
        if self.next_key_spawn_at.is_none_or(|deadline| now < deadline) {
            return;
        }
        if pulse_count(&self.keys) < MAX_KEY_PULSES {
            if let Some(location) = next_location(&mut self.rng, &self.keys, self.last_key_location)
            {
                self.keys[location] = Some(self.new_pulse(now));
                self.last_key_location = Some(location);
            }
        }
        self.next_key_spawn_at = Some(now + self.spawn_delay(KEY_SPAWN_MIN, KEY_SPAWN_MAX));
    }

    fn new_pulse(&mut self, now: Instant) -> Pulse {
        Pulse {
            started_at: now,
            duration: self.duration(PULSE_MIN, PULSE_MAX),
            color: SLEEP_COLORS[self.rng.index(SLEEP_COLORS.len())],
        }
    }

    fn duration(&mut self, minimum: Duration, maximum: Duration) -> Duration {
        Duration::from_millis(
            self.rng
                .range(minimum.as_millis() as u64, maximum.as_millis() as u64 + 1),
        )
    }

    fn spawn_delay(&mut self, minimum: Duration, maximum: Duration) -> Duration {
        self.duration(minimum, maximum)
    }

    fn render_grid(&self, now: Instant) -> [[u8; 3]; GRID_LED_COUNT] {
        render_pulses(&self.grid, now, self.grid_brightness)
    }

    fn render_keys(&self, now: Instant) -> [[u8; 3]; KEY_LED_COUNT] {
        render_pulses(&self.keys, now, self.key_brightness)
    }
}

impl Default for SleepLedAnimation {
    fn default() -> Self {
        Self::new()
    }
}

fn render_pulses<const COUNT: usize>(
    pulses: &[Option<Pulse>; COUNT],
    now: Instant,
    brightness: f32,
) -> [[u8; 3]; COUNT] {
    let brightness = sleep_dim_brightness(brightness);
    let mut frame = [[0_u8; 3]; COUNT];
    for (index, pulse) in pulses.iter().enumerate() {
        let Some(pulse) = pulse else {
            continue;
        };
        let phase = now
            .saturating_duration_since(pulse.started_at)
            .as_secs_f32()
            / pulse.duration.as_secs_f32();
        let envelope = (std::f32::consts::PI * phase.clamp(0.0, 1.0))
            .sin()
            .max(0.0);
        frame[index] = scale(pulse.color, brightness * envelope);
    }
    frame
}

fn expire_pulses<const COUNT: usize>(pulses: &mut [Option<Pulse>; COUNT], now: Instant) {
    for pulse in pulses {
        if pulse.is_some_and(|pulse| now >= pulse.started_at + pulse.duration) {
            *pulse = None;
        }
    }
}

fn pulse_count<const COUNT: usize>(pulses: &[Option<Pulse>; COUNT]) -> usize {
    pulses.iter().filter(|pulse| pulse.is_some()).count()
}

#[cfg(test)]
fn pulse_windows<const COUNT: usize>(
    pulses: &[Option<Pulse>; COUNT],
) -> Vec<(usize, Instant, Instant)> {
    pulses
        .iter()
        .enumerate()
        .filter_map(|(index, pulse)| {
            pulse.map(|pulse| (index, pulse.started_at, pulse.started_at + pulse.duration))
        })
        .collect()
}

fn next_location<const COUNT: usize>(
    rng: &mut SmallPrng,
    pulses: &[Option<Pulse>; COUNT],
    previous: Option<usize>,
) -> Option<usize> {
    let start = rng.index(COUNT);
    (0..COUNT)
        .map(|offset| (start + offset) % COUNT)
        .find(|index| pulses[*index].is_none() && previous != Some(*index))
}

fn earlier(first: Option<Instant>, second: Option<Instant>) -> Option<Instant> {
    match (first, second) {
        (Some(first), Some(second)) => Some(first.min(second)),
        (Some(first), None) => Some(first),
        (None, Some(second)) => Some(second),
        (None, None) => None,
    }
}

fn clock_seed() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0)
}

struct SmallPrng {
    state: u64,
}

impl SmallPrng {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 {
                0x9e37_79b9_7f4a_7c15
            } else {
                seed
            },
        }
    }

    fn next(&mut self) -> u64 {
        let mut value = self.state;
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        self.state = value;
        value
    }

    fn index(&mut self, length: usize) -> usize {
        (self.next() as usize) % length
    }

    fn range(&mut self, minimum: u64, maximum_exclusive: u64) -> u64 {
        minimum + self.next() % (maximum_exclusive - minimum)
    }
}
