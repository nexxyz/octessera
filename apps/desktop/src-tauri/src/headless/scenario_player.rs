use super::HeadlessSession;
use playback_runtime::scenario::{button, Cue, Scenario, SetupStep};

/// One 24-PPQN pulse at 120 BPM; pulse cues left after the transport stops keep this tempo.
const PULSE_SECONDS: f64 = 0.5 / 24.0;

/// Where a played scenario sits in the rendered audio.
#[derive(Clone, Debug)]
pub struct PlayedScenario {
    /// Frame index of B1, the start of the timed section.
    pub start_frame: usize,
    /// Transport pulse at B1; pulse cues and event pulses count from it.
    pub start_pulse: u64,
    /// The snapshot right after each timed input, in cue order: (seconds after B1, snapshot).
    pub observations: Vec<(f64, serde_json::Value)>,
}

impl HeadlessSession {
    /// Plays `scenario`: setup (checking each checkpoint title), `preroll_seconds` of silence,
    /// then the timed section from B1 until `end_seconds` after B1 and every cue has run.
    ///
    /// The engine applies queued configuration while it renders, as a running device does, so a
    /// scenario whose setup changes instruments needs some pre-roll (0.5 s is plenty) before
    /// its first note.
    pub fn play_scenario(
        &mut self,
        scenario: &Scenario,
        preroll_seconds: f64,
        end_seconds: f64,
    ) -> Result<PlayedScenario, String> {
        self.run_setup(&scenario.setup)?;
        self.render(self.seconds_to_frames(preroll_seconds))?;
        let mut applied = vec![false; scenario.perf.len()];
        for (index, timed) in scenario.perf.iter().enumerate() {
            if timed.cue
                == (Cue::Pulse {
                    pulse: 0,
                    before: true,
                })
            {
                applied[index] = true;
                self.input(timed.input.clone())?;
            }
        }
        if scenario.play_at_b1 {
            self.input(button("button_s", true))?;
            self.input(button("button_s", false))?;
        }
        let mut played = PlayedScenario {
            start_frame: self.frames(),
            start_pulse: self.transport_pulse(),
            observations: Vec::new(),
        };
        let b1_seconds = self.virtual_seconds();
        let mut stopped_at: Option<(f64, u64)> = None;
        let mut max_pulse = 0;
        loop {
            let since_b1 = self.virtual_seconds() - b1_seconds;
            let n = self.transport_pulse().saturating_sub(played.start_pulse);
            max_pulse = max_pulse.max(n);
            if scenario.play_at_b1 && stopped_at.is_none() && max_pulse > 0 && !self.is_playing() {
                stopped_at = Some((since_b1, max_pulse));
            }
            for (index, timed) in scenario.perf.iter().enumerate() {
                let due = match (timed.cue, stopped_at) {
                    (Cue::Seconds(seconds), _) => since_b1 >= seconds,
                    (Cue::Pulse { pulse, .. }, Some((at, stop_pulse))) => {
                        since_b1 - at >= pulse.saturating_sub(stop_pulse + 1) as f64 * PULSE_SECONDS
                    }
                    (
                        Cue::Pulse {
                            pulse,
                            before: true,
                        },
                        None,
                    ) => pulse <= n + 1,
                    (
                        Cue::Pulse {
                            pulse,
                            before: false,
                        },
                        None,
                    ) => pulse <= n,
                };
                if !applied[index] && due {
                    applied[index] = true;
                    self.input(timed.input.clone())?;
                    let snapshot = self.snapshot().cloned().unwrap_or_default();
                    played.observations.push((since_b1, snapshot));
                }
            }
            if since_b1 >= end_seconds && applied.iter().all(|done| *done) {
                break;
            }
            if since_b1 > end_seconds + 30.0 {
                return Err(format!("cues still pending {since_b1:.1} s after B1"));
            }
            self.render_block(super::BLOCK_FRAMES)?;
        }
        Ok(played)
    }

    fn run_setup(&mut self, setup: &[SetupStep]) -> Result<(), String> {
        for step in setup {
            match step {
                SetupStep::Input(input) => {
                    self.input(input.clone())?;
                    // A running device's engine drains its event queue every few milliseconds.
                    self.render_block(super::BLOCK_FRAMES)?;
                }
                SetupStep::Checkpoint { name, title } => {
                    self.settle()?;
                    let shown = self.title();
                    if &shown != title {
                        return Err(format!(
                            "checkpoint {name}: OLED shows {shown:?}, expected {title:?}"
                        ));
                    }
                }
            }
        }
        self.settle()
    }

    fn seconds_to_frames(&self, seconds: f64) -> usize {
        (seconds * f64::from(self.sample_rate())) as usize
    }
}
