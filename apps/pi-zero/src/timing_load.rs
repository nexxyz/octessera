//! Opt-in board capacity study (`OCTESSERA_TIMING_LOAD=start,step,max,seconds`).
//! Ramps held notes through the host musical-event path and times probe Keys
//! presses from arrival to audio hand-off. Inert unless the variable is set.

use crate::host_adapter::PiHostAdapter;
use crate::input::grid_message;
use playback_runtime::{HostAdapter, MusicalEvent, NativeRunner, PlaybackRuntime};
use std::time::{Duration, Instant};

const LOAD_ENV: &str = "OCTESSERA_TIMING_LOAD";
const AUTOPLAY_ENV: &str = "OCTESSERA_TIMING_AUTOPLAY";
const NOTE_GATE: Duration = Duration::from_millis(2_000);
const LOAD_SLOTS: [u8; 3] = [0, 2, 3];
const NOTES_PER_TICK: usize = 8;
const PROBE_INTERVAL: Duration = Duration::from_millis(700);
const PROBE_HOLD: Duration = Duration::from_millis(150);

#[derive(Default)]
pub(crate) struct NoteSentProbe {
    first_note_on_at: Option<Instant>,
}

impl NoteSentProbe {
    pub(crate) fn observe(&mut self, event: &MusicalEvent) {
        if self.first_note_on_at.is_none() && matches!(event, MusicalEvent::NoteOn { .. }) {
            self.first_note_on_at = Some(Instant::now());
        }
    }
}

#[derive(Default)]
struct DurationStats {
    count: u32,
    total_us: u64,
    max_us: u64,
}

impl DurationStats {
    fn record(&mut self, value: Duration) {
        let us = value.as_micros() as u64;
        self.count += 1;
        self.total_us += us;
        self.max_us = self.max_us.max(us);
    }

    fn summary(&self) -> String {
        let avg = self
            .total_us
            .checked_div(u64::from(self.count))
            .unwrap_or(0);
        format!("n={} avg={avg}us max={}us", self.count, self.max_us)
    }
}

#[derive(Default)]
struct StepStats {
    notes: u32,
    probe_wait: DurationStats,
    probe_to_audio: DurationStats,
    probe_silent: u32,
}

pub(crate) struct TimingLoad {
    start: u32,
    step: u32,
    max: u32,
    step_duration: Duration,
    started_at: Instant,
    step_index: Option<u32>,
    next_note_at: Instant,
    note_counter: u64,
    next_probe_at: Instant,
    probe_counter: u32,
    release_at: Option<Instant>,
    stats: StepStats,
}

impl TimingLoad {
    pub(crate) fn from_env(now: Instant) -> Result<Option<Self>, String> {
        let Ok(spec) = std::env::var(LOAD_ENV) else {
            return Ok(None);
        };
        if std::env::var(AUTOPLAY_ENV).as_deref() != Ok("1") {
            return Err(format!("{LOAD_ENV} requires {AUTOPLAY_ENV}=1"));
        }
        let store = std::env::var("OCTESSERA_PI_STORE_DIR").unwrap_or_default();
        if !crate::timing_input::is_study_store(&store) {
            return Err(format!("{LOAD_ENV} requires an isolated study store"));
        }
        let [start, step, max, seconds] = parse_spec(&spec)?;
        Ok(Some(Self {
            start,
            step,
            max,
            step_duration: Duration::from_secs(u64::from(seconds)),
            started_at: now,
            step_index: None,
            next_note_at: now,
            note_counter: 0,
            next_probe_at: now + PROBE_INTERVAL,
            probe_counter: 0,
            release_at: None,
            stats: StepStats::default(),
        }))
    }

    /// Returns `true` once the last step has been reported.
    pub(crate) fn tick(
        &mut self,
        now: Instant,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut PiHostAdapter,
    ) -> Result<bool, String> {
        let elapsed_steps =
            (now.duration_since(self.started_at).as_secs() / self.step_duration.as_secs()) as u32;
        if self.step_index != Some(elapsed_steps) {
            if let Some(previous) = self.step_index {
                self.report(previous);
            }
            if self.target(elapsed_steps) > self.max {
                eprintln!("timing-load complete");
                return Ok(true);
            }
            self.step_index = Some(elapsed_steps);
            eprintln!(
                "timing-load step-start step={elapsed_steps} held_notes={}",
                self.target(elapsed_steps)
            );
        }
        self.play_load_notes(now, self.target(elapsed_steps), host)?;
        self.probe(now, playback, runner, host)?;
        Ok(false)
    }

    /// Real inputs wake the loop on arrival; the probe models that by capping
    /// the loop's sleep at its own arrival time.
    pub(crate) fn until_next_probe(&self, now: Instant) -> Duration {
        let next = self.release_at.map_or(self.next_probe_at, |release| {
            release.min(self.next_probe_at)
        });
        next.saturating_duration_since(now)
    }

    fn target(&self, step_index: u32) -> u32 {
        self.start
            .saturating_add(self.step.saturating_mul(step_index))
    }

    fn play_load_notes(
        &mut self,
        now: Instant,
        target: u32,
        host: &mut PiHostAdapter,
    ) -> Result<(), String> {
        if target == 0 {
            self.next_note_at = now;
            return Ok(());
        }
        let interval = NOTE_GATE / target;
        if now.duration_since(self.next_note_at) > Duration::from_secs(1) {
            self.next_note_at = now;
        }
        for _ in 0..NOTES_PER_TICK {
            if now < self.next_note_at {
                break;
            }
            let counter = self.note_counter;
            let slot_count = LOAD_SLOTS.len() as u64;
            let event = MusicalEvent::NoteOn {
                channel: LOAD_SLOTS[(counter % slot_count) as usize],
                note: 36 + ((counter / slot_count) % 48) as u8,
                velocity: 100,
                duration_ms: Some(NOTE_GATE.as_millis() as u32),
            };
            host.handle_musical_event(&event)
                .map_err(|error| format!("timing load note failed: {error:?}"))?;
            self.note_counter += 1;
            self.stats.notes += 1;
            self.next_note_at += interval;
        }
        Ok(())
    }

    fn probe(
        &mut self,
        now: Instant,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut PiHostAdapter,
    ) -> Result<(), String> {
        if self.release_at.is_some_and(|release_at| now >= release_at) {
            self.release_at = None;
            crate::runtime_dispatch::dispatch(playback, runner, host, grid_message(0, 0, false))?;
        }
        if now < self.next_probe_at || self.release_at.is_some() {
            return Ok(());
        }
        self.stats
            .probe_wait
            .record(now.duration_since(self.next_probe_at));
        host.note_sent_probe = Some(NoteSentProbe::default());
        let dispatched_at = Instant::now();
        let dispatched =
            crate::runtime_dispatch::dispatch(playback, runner, host, grid_message(0, 0, true));
        let probe = host.note_sent_probe.take();
        dispatched?;
        match probe.and_then(|probe| probe.first_note_on_at) {
            Some(sent_at) => self
                .stats
                .probe_to_audio
                .record(sent_at.duration_since(dispatched_at)),
            None => self.stats.probe_silent += 1,
        }
        self.release_at = Some(now + PROBE_HOLD);
        self.probe_counter = self.probe_counter.wrapping_add(1);
        let jitter_us = u64::from(self.probe_counter.wrapping_mul(2_654_435_761) % 9_000);
        self.next_probe_at = now + PROBE_INTERVAL + Duration::from_micros(jitter_us);
        Ok(())
    }

    fn report(&mut self, step_index: u32) {
        let stats = std::mem::take(&mut self.stats);
        eprintln!(
            "timing-load step={step_index} held_notes={} notes_sent={} probe_wait={} probe_dispatch_to_audio={} probe_silent={}",
            self.target(step_index),
            stats.notes,
            stats.probe_wait.summary(),
            stats.probe_to_audio.summary(),
            stats.probe_silent,
        );
    }
}

fn parse_spec(spec: &str) -> Result<[u32; 4], String> {
    let values = spec
        .split(',')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| format!("{LOAD_ENV} must be start,step,max,seconds"))?;
    let [start, step, max, seconds] = values[..] else {
        return Err(format!("{LOAD_ENV} must be start,step,max,seconds"));
    };
    if step == 0 || max < start || max > 120 || !(5..=600).contains(&seconds) {
        return Err(format!("{LOAD_ENV} values are out of range"));
    }
    Ok([start, step, max, seconds])
}

#[cfg(test)]
mod tests {
    use super::parse_spec;

    #[test]
    fn load_spec_accepts_a_bounded_ramp_and_rejects_the_rest() {
        assert_eq!(parse_spec("0,4,48,30"), Ok([0, 4, 48, 30]));
        for invalid in [
            "",
            "0,4,48",
            "0,0,48,30",
            "8,4,4,30",
            "0,4,200,30",
            "0,4,48,2",
            "a,4,48,30",
        ] {
            assert!(parse_spec(invalid).is_err(), "{invalid}");
        }
    }
}
