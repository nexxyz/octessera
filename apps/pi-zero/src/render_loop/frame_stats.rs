use std::time::{Duration, Instant};

const REPORT_INTERVAL: Duration = Duration::from_secs(10);

pub(super) struct OledFrameStats {
    enabled: bool,
    window_start: Instant,
    thread_cpu_start: Option<Duration>,
    frames_start: u64,
    scenes: u64,
    prepare_us: Vec<u32>,
    present_us: Vec<u32>,
    scene_us: Vec<u32>,
    cpus: [u64; 4],
}

impl Default for OledFrameStats {
    fn default() -> Self {
        Self::new(crate::ui_profile::UiProfiler::from_process().enabled(), 0)
    }
}

impl OledFrameStats {
    fn new(enabled: bool, frames_start: u64) -> Self {
        Self {
            enabled,
            window_start: Instant::now(),
            thread_cpu_start: thread_cpu_time(),
            frames_start,
            scenes: 0,
            prepare_us: Vec::new(),
            present_us: Vec::new(),
            scene_us: Vec::new(),
            cpus: [0; 4],
        }
    }

    pub(super) fn record(
        &mut self,
        prepare: Duration,
        written_present: Option<Duration>,
        scene: Duration,
        frames_total: u64,
    ) {
        if !self.enabled {
            return;
        }
        self.scenes += 1;
        self.prepare_us.push(micros(prepare));
        if let Some(present) = written_present {
            self.present_us.push(micros(present));
        }
        self.scene_us.push(micros(scene));
        if let Some(count) = current_cpu().and_then(|cpu| self.cpus.get_mut(cpu)) {
            *count += 1;
        }
        let window = self.window_start.elapsed();
        if window < REPORT_INTERVAL {
            return;
        }
        let cpu_pct = thread_cpu_time()
            .zip(self.thread_cpu_start)
            .map_or(-1.0, |(now, start)| {
                (now - start).as_secs_f64() * 100.0 / window.as_secs_f64()
            });
        eprintln!(
            "pi-oled-frame-stats window_ms={} scenes={} frames={} prepare_us={} present_us={} scene_us={} thread_cpu_pct={cpu_pct:.1} cpu={:?}",
            window.as_millis(),
            self.scenes,
            frames_total - self.frames_start,
            percentiles(&mut self.prepare_us),
            percentiles(&mut self.present_us),
            percentiles(&mut self.scene_us),
            self.cpus,
        );
        *self = Self::new(true, frames_total);
    }
}

fn micros(duration: Duration) -> u32 {
    u32::try_from(duration.as_micros()).unwrap_or(u32::MAX)
}

fn percentiles(values: &mut [u32]) -> String {
    if values.is_empty() {
        return "n=0".into();
    }
    values.sort_unstable();
    let at = |percent: usize| values[(values.len() - 1) * percent / 100];
    format!(
        "n={},p50={},p95={},max={}",
        values.len(),
        at(50),
        at(95),
        values[values.len() - 1]
    )
}

#[cfg(target_os = "linux")]
fn current_cpu() -> Option<usize> {
    usize::try_from(unsafe { libc::sched_getcpu() }).ok()
}

#[cfg(not(target_os = "linux"))]
fn current_cpu() -> Option<usize> {
    None
}

#[cfg(target_os = "linux")]
fn thread_cpu_time() -> Option<Duration> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    (unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) } == 0).then(|| {
        Duration::new(
            u64::try_from(time.tv_sec).unwrap_or(0),
            u32::try_from(time.tv_nsec).unwrap_or(0),
        )
    })
}

#[cfg(not(target_os = "linux"))]
fn thread_cpu_time() -> Option<Duration> {
    None
}
