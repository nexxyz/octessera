use crate::autoaux_menu;
use crate::autoaux_sequence::{AutoAuxAction, AutoAuxSequence};
use crate::input::{encoder_turn_message, neokey_message};
use crate::normal_menu::is_normal_menu_snapshot;
use crate::runtime_output::PiRuntimeHost;
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimeAudioCommand, RuntimeStoreResult,
    RuntimeTransportState,
};
use std::time::{Duration, Instant};

const AUTOAUX_ENV: &str = "OCTESSERA_TIMING_AUTOAUX";
const AUTOPLAY_ENV: &str = "OCTESSERA_TIMING_AUTOPLAY";
const KEEP_AWAKE_ENV: &str = "OCTESSERA_TIMING_KEEP_AWAKE";
const UI_PROFILE_ENV: &str = "OCTESSERA_PI_UI_PROFILE";
const STUDY_STORE_PREFIX: &str = "/var/lib/octessera/study-stores/";

pub(crate) trait TimingHost: PiRuntimeHost {
    const STUDY_BOARD: &'static str;
    const REPORT_PREFIX: &'static str;

    fn timing_evidence(&mut self) -> &mut Option<TimingStudyEvidence>;
}

#[derive(Default)]
pub(crate) struct TimingStudyEvidence {
    cutoff_commands: u64,
    cutoff_value_bits: [Option<u32>; 2],
    store_results: Vec<(RuntimeStoreResult, Instant)>,
}

impl TimingStudyEvidence {
    pub(crate) fn record_command(&mut self, command: &RuntimeAudioCommand) {
        let RuntimeAudioCommand::SetSynthParam {
            instrument_slot: 0,
            path,
            value,
            ..
        } = command
        else {
            return;
        };
        if path != "synth.filter.cutoffHz" {
            return;
        }
        self.cutoff_commands = self.cutoff_commands.saturating_add(1);
        let bits = value.to_bits();
        if self
            .cutoff_value_bits
            .iter()
            .flatten()
            .any(|seen| *seen == bits)
        {
            return;
        }
        if let Some(empty) = self
            .cutoff_value_bits
            .iter_mut()
            .find(|value| value.is_none())
        {
            *empty = Some(bits);
        }
    }

    pub(crate) fn record_host_message(&mut self, message: &HostMessage) {
        if let HostMessage::RuntimeResult { result } = message {
            self.store_results.push((result.clone(), Instant::now()));
        }
    }

    pub(crate) fn cutoff_command_count(&self) -> u64 {
        self.cutoff_commands
    }

    pub(crate) fn cutoff_values(&self) -> [Option<f32>; 2] {
        self.cutoff_value_bits
            .map(|value| value.map(f32::from_bits))
    }

    fn has_two_cutoff_values(&self) -> bool {
        self.cutoff_commands > 0
            && self.cutoff_value_bits[0].is_some()
            && self.cutoff_value_bits[1].is_some()
            && self.cutoff_value_bits[0] != self.cutoff_value_bits[1]
    }
}

pub(crate) struct TimingInput {
    pub(crate) sequence: AutoAuxSequence,
    pub(crate) starting_cutoff: u16,
    pub(crate) plateau_values: [u16; 2],
}

impl TimingInput {
    pub(crate) fn validate_opt_in() -> Result<bool, String> {
        let Some(auto_aux) = std::env::var_os(AUTOAUX_ENV) else {
            return Ok(false);
        };
        if auto_aux.to_str() != Some("1") {
            return Err(format!("{AUTOAUX_ENV} must be exactly 1 when set"));
        }
        for variable in [AUTOPLAY_ENV, KEEP_AWAKE_ENV, UI_PROFILE_ENV] {
            if std::env::var(variable).as_deref() != Ok("1") {
                return Err(format!("Aux timing smoke requires {variable}=1"));
            }
        }
        if !matches!(
            crate::board_profile::BOARD_PROFILE_ID,
            "orange-pi-zero-2w" | "raspberry-pi-zero-2w"
        ) {
            return Err("Aux timing smoke requires a fixed Octessera board profile".into());
        }
        let store = std::env::var("OCTESSERA_PI_STORE_DIR")
            .map_err(|_| "Aux timing smoke requires an isolated study store".to_string())?;
        if !is_study_store(&store) {
            return Err("Aux timing smoke store is not an isolated study clone".into());
        }
        Ok(true)
    }

    pub(crate) fn prepare<H: TimingHost>(
        auto_aux: bool,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut H,
    ) -> Result<Option<Self>, String> {
        if !auto_aux {
            if std::env::var(AUTOPLAY_ENV).as_deref() == Ok("1") {
                start_playback(playback, runner, host)?;
            }
            return Ok(None);
        }
        autoaux_menu::require_stopped_normal_menu(playback, H::STUDY_BOARD)?;
        if !runner.is_canonical_menu_presentation() {
            return Err("Aux timing smoke requires the canonical normal menu".into());
        }
        let (starting_cutoff, plateau_values) = {
            let mut send = |playback: &mut PlaybackRuntime,
                            runner: &mut NativeRunner,
                            message: HostMessage| {
                H::dispatch(playback, runner, host, message)
            };
            autoaux_menu::enable_study_auto_save(playback, runner, &mut send, H::STUDY_BOARD)?;
            autoaux_menu::navigate_to_cutoff(playback, runner, &mut send, H::STUDY_BOARD)?;
            autoaux_menu::preflight_aux_cutoff(playback, runner, &mut send, H::STUDY_BOARD)?
        };
        start_playback(playback, runner, host)?;
        *host.timing_evidence() = Some(TimingStudyEvidence::default());
        Ok(Some(Self {
            sequence: AutoAuxSequence::new(Instant::now()),
            starting_cutoff,
            plateau_values,
        }))
    }

    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    pub(crate) fn for_test(sequence: AutoAuxSequence, starting_cutoff: u16) -> Self {
        Self {
            sequence,
            starting_cutoff,
            plateau_values: [0, 0],
        }
    }

    pub(crate) fn plateau_values(&self) -> [u16; 2] {
        self.plateau_values
    }

    pub(crate) fn tick<H: TimingHost>(
        &mut self,
        now: Instant,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut H,
    ) -> Result<bool, String> {
        if !is_playing(playback) {
            return Err("Aux timing smoke left Playing unexpectedly".into());
        }
        let store_results = host
            .timing_evidence()
            .as_mut()
            .map(|evidence| std::mem::take(&mut evidence.store_results))
            .unwrap_or_default();
        for (result, accepted_at) in store_results {
            self.sequence.refresh_final_revision(now, runner);
            self.sequence.accept_store_result(&result, accepted_at)?;
        }
        match self.sequence.next_action(now, runner)? {
            AutoAuxAction::Turn(delta) => {
                H::dispatch(
                    playback,
                    runner,
                    host,
                    encoder_turn_message("encoder_aux_1", delta),
                )?;
                self.sequence.turn_issued(now);
                Ok(false)
            }
            AutoAuxAction::Completed => Ok(true),
            AutoAuxAction::None => Ok(false),
        }
    }

    fn report(
        &self,
        prefix: &str,
        rows: [(u64, u16); 2],
        evidence: &TimingStudyEvidence,
        request_id: &str,
        save_elapsed: Duration,
    ) {
        let values = evidence.cutoff_values();
        eprintln!(
            "{prefix} cutoff_start={} oled_cutoff_a={} rev_a={} oled_cutoff_b={} rev_b={} synth_cutoff_successes={} synth_cutoff_a={} synth_cutoff_b={} save_revision={} save_request={} save_elapsed_ms={} aux_turns={} rapid_turns={} missed_turns={}",
            self.starting_cutoff,
            rows[0].1,
            rows[0].0,
            rows[1].1,
            rows[1].0,
            evidence.cutoff_command_count(),
            values[0].unwrap_or_default(),
            values[1].unwrap_or_default(),
            self.sequence.final_revision.unwrap_or_default(),
            request_id,
            save_elapsed.as_millis(),
            self.sequence.aux_turns,
            self.sequence.rapid_turns,
            self.sequence.missed_turns,
        );
    }
}

pub(crate) fn validate_startup(playback: &PlaybackRuntime) -> Result<bool, String> {
    ensure_timing_keep_awake(playback)?;
    TimingInput::validate_opt_in()
}

pub(crate) fn tick_if_active<H: TimingHost>(
    timing: &mut Option<TimingInput>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut H,
    cutoff_acceptances: Option<[(u64, u16); 2]>,
) -> Result<(), String> {
    let Some(input) = timing.as_mut() else {
        return Ok(());
    };
    if input.tick(Instant::now(), playback, runner, host)? {
        complete_study(input, host, cutoff_acceptances)?;
        *timing = None;
    }
    Ok(())
}

pub(crate) fn complete_study<H: TimingHost>(
    input: &TimingInput,
    host: &mut H,
    cutoff_acceptances: Option<[(u64, u16); 2]>,
) -> Result<(), String> {
    let (request_id, elapsed) = input
        .sequence
        .save_completion
        .as_ref()
        .ok_or("Aux timing smoke completed without a native save proof")?;
    let rows = cutoff_acceptances
        .ok_or("Aux timing smoke did not physically publish both Cutoff plateaus")?;
    if rows[0].0 == rows[1].0 || rows[0].1 == rows[1].1 {
        return Err(
            "Aux timing smoke Cutoff evidence did not contain two distinct physical frames".into(),
        );
    }
    let evidence = host
        .timing_evidence()
        .take()
        .ok_or("Aux timing smoke has no host command evidence")?;
    if !evidence.has_two_cutoff_values() {
        return Err("Aux timing smoke did not host-handle two distinct synth Cutoff values".into());
    }
    input.report(H::REPORT_PREFIX, rows, &evidence, request_id, *elapsed);
    Ok(())
}

pub(crate) fn fail_study<H: TimingHost>(message: impl std::fmt::Display) -> ! {
    eprintln!("{}-failed: {message}", H::REPORT_PREFIX);
    std::process::exit(2)
}

pub(crate) fn ensure_timing_keep_awake(playback: &PlaybackRuntime) -> Result<(), String> {
    if std::env::var(KEEP_AWAKE_ENV).as_deref() != Ok("1") {
        return Ok(());
    }
    let snapshot = playback
        .last_snapshot()
        .ok_or("AWAKE timing candidate has no native snapshot")?;
    if !is_awake_menu_snapshot(snapshot) {
        return Err("AWAKE timing candidate did not load awake settings".into());
    }
    Ok(())
}

pub(crate) fn is_awake_menu_snapshot(snapshot: &serde_json::Value) -> bool {
    is_normal_menu_snapshot(snapshot)
        && snapshot["settings"]["dimTimerSeconds"] == 0
        && snapshot["settings"]["screenSleepSeconds"] == 0
        && snapshot["settings"]["ledsDimmed"] == false
}

fn is_playing(playback: &PlaybackRuntime) -> bool {
    playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
}

fn start_playback<H: TimingHost>(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut H,
) -> Result<(), String> {
    for pressed in [true, false] {
        let message =
            neokey_message(1, pressed).ok_or("timing autoplay NeoKey index 1 unavailable")?;
        H::dispatch(playback, runner, host, message)?;
    }
    if !is_playing(playback) {
        return Err("timing autoplay did not enter Playing".into());
    }
    Ok(())
}

fn is_study_store(path: &str) -> bool {
    let Some(unit) = path.strip_prefix(STUDY_STORE_PREFIX) else {
        return false;
    };
    let Some(id) = unit
        .strip_prefix("octessera-study-")
        .and_then(|unit| unit.strip_suffix(".service"))
    else {
        return false;
    };
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
}
