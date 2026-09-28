use super::dispatch;
use super::timing_menu;
use crate::autoaux_sequence::{AutoAuxAction, AutoAuxSequence};
#[cfg(test)]
use crate::autoaux_sequence::{
    Phase, BASELINE, PLATEAU, RAPID, SAVE_COMPLETION_TIMEOUT, TURN_INTERVAL,
};
use crate::input::{encoder_turn_message, neokey_message};
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{NativeRunner, PlaybackRuntime, RuntimeStoreResult, RuntimeTransportState};
use std::time::{Duration, Instant};

const AUTOAUX_ENV: &str = "OCTESSERA_TIMING_AUTOAUX";
pub(super) struct OrangeTimingInput {
    sequence: AutoAuxSequence,
    starting_cutoff: u16,
    plateau_values: [u16; 2],
}

impl std::ops::Deref for OrangeTimingInput {
    type Target = AutoAuxSequence;

    fn deref(&self) -> &Self::Target {
        &self.sequence
    }
}

impl std::ops::DerefMut for OrangeTimingInput {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sequence
    }
}

impl OrangeTimingInput {
    pub(super) fn validate_opt_in() -> Result<bool, String> {
        let auto_aux = match std::env::var_os(AUTOAUX_ENV) {
            None => return Ok(false),
            Some(value) => value,
        };
        if auto_aux.to_str() != Some("1") {
            return Err(format!("{AUTOAUX_ENV} must be exactly 1 when set"));
        }
        for variable in [
            "OCTESSERA_TIMING_AUTOPLAY",
            "OCTESSERA_PI_TIMING_KEEP_AWAKE",
            "OCTESSERA_PI_UI_PROFILE",
        ] {
            if std::env::var(variable).as_deref() != Ok("1") {
                return Err(format!("Orange Aux timing smoke requires {variable}=1"));
            }
        }
        if crate::board_profile::BOARD_PROFILE_ID != "orange-pi-zero-2w" {
            return Err("Orange Aux timing smoke requires the Orange Pi Zero 2W profile".into());
        }
        let store = std::env::var("OCTESSERA_PI_STORE_DIR")
            .map_err(|_| "Orange Aux timing smoke requires an isolated study store".to_string())?;
        if !validate_study_store(&store) {
            return Err("Orange Aux timing smoke store is not an isolated study clone".into());
        }
        Ok(true)
    }

    pub(super) fn prepare(
        auto_aux: bool,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut OrangeHostAdapter,
    ) -> Result<Option<Self>, String> {
        if !auto_aux {
            if std::env::var("OCTESSERA_TIMING_AUTOPLAY").as_deref() == Ok("1") {
                start_playback(playback, runner, host)?;
            }
            return Ok(None);
        }
        timing_menu::require_stopped_normal_menu(playback)?;
        if !runner.is_canonical_menu_presentation() {
            return Err("Orange Aux timing smoke requires the canonical normal menu".into());
        }

        timing_menu::enable_study_auto_save(playback, runner, host)?;
        timing_menu::navigate_to_cutoff(playback, runner, host)?;
        let original = timing_menu::cutoff_display_value(playback)?;
        dispatch(
            playback,
            runner,
            host,
            encoder_turn_message("encoder_aux_1", 1),
        )?;
        let first = timing_menu::cutoff_display_value(playback)?;
        if first == original || first >= 255 {
            return Err("Orange Aux 1 did not change Cutoff by one display step".into());
        }
        if first != original + 1 {
            return Err("Orange Aux 1 did not change Cutoff by exactly one display step".into());
        }
        dispatch(
            playback,
            runner,
            host,
            encoder_turn_message("encoder_aux_1", -1),
        )?;
        if timing_menu::cutoff_display_value(playback)? != original {
            return Err("Orange Aux 1 did not restore the starting Cutoff value".into());
        }
        let second = first + 1;
        start_playback(playback, runner, host)?;
        host.begin_autoaux_command_evidence();
        let started_at = Instant::now();
        Ok(Some(Self {
            sequence: AutoAuxSequence::new(started_at),
            starting_cutoff: original,
            plateau_values: [first, second],
        }))
    }

    pub(super) fn plateau_values(&self) -> [u16; 2] {
        self.plateau_values
    }

    pub(super) fn report(
        &self,
        accepted_rows: [(u64, u16); 2],
        audio: crate::orange_audio::AutoAuxCommandEvidence,
        request_id: &str,
        save_elapsed: Duration,
    ) {
        eprintln!(
            "orange-autoaux cutoff_start={} oled_cutoff_a={} rev_a={} oled_cutoff_b={} rev_b={} synth_cutoff_successes={} synth_cutoff_a={} synth_cutoff_b={} save_revision={} save_request={} save_elapsed_ms={} aux_turns={} rapid_turns={} missed_turns={}",
            self.starting_cutoff,
            accepted_rows[0].1,
            accepted_rows[0].0,
            accepted_rows[1].1,
            accepted_rows[1].0,
            audio.successful_count,
            audio.distinct_values[0].unwrap_or_default(),
            audio.distinct_values[1].unwrap_or_default(),
            self.final_revision.unwrap_or_default(),
            request_id,
            save_elapsed.as_millis(),
            self.aux_turns,
            self.rapid_turns,
            self.missed_turns,
        );
    }

    pub(super) fn tick(
        &mut self,
        now: Instant,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut OrangeHostAdapter,
    ) -> Result<bool, String> {
        if !playback
            .last_status()
            .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
        {
            return Err("Orange Aux timing smoke left Playing unexpectedly".into());
        }
        match self.sequence.next_action(now, runner)? {
            AutoAuxAction::Turn(delta) => {
                self.send_aux(playback, runner, host, now, delta)?;
            }
            AutoAuxAction::Completed => return Ok(true),
            AutoAuxAction::None => {}
        }
        Ok(false)
    }

    pub(super) fn accept_store_result(
        &mut self,
        result: &RuntimeStoreResult,
        accepted_at: Instant,
    ) -> Result<(), String> {
        self.sequence.accept_store_result(result, accepted_at)
    }

    fn send_aux(
        &mut self,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        host: &mut OrangeHostAdapter,
        now: Instant,
        delta: i8,
    ) -> Result<(), String> {
        dispatch(
            playback,
            runner,
            host,
            encoder_turn_message("encoder_aux_1", delta),
        )?;
        self.sequence.turn_issued(now);
        Ok(())
    }
}

pub(super) fn validate_startup(playback: &PlaybackRuntime) -> Result<bool, String> {
    super::super::startup::ensure_timing_keep_awake(playback)?;
    OrangeTimingInput::validate_opt_in()
}

pub(super) fn configure_scene_diagnostics(
    timing: &Option<OrangeTimingInput>,
    scenes: &mut super::super::native_scene::OrangeNativeScenePump,
    profiler: &crate::ui_profile::UiProfiler,
) {
    scenes.set_capture_profile_enabled(profiler.enabled());
    if let Some(timing) = timing {
        scenes.set_timing_cutoff_targets(timing.plateau_values());
    }
}

pub(super) fn tick_if_active(
    timing: &mut Option<OrangeTimingInput>,
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
    scenes: &super::super::native_scene::OrangeNativeScenePump,
) -> Result<(), String> {
    let Some(timing) = timing.as_mut() else {
        return Ok(());
    };
    if timing.tick(Instant::now(), playback, runner, host)? {
        let (request_id, elapsed) = timing
            .save_completion
            .as_ref()
            .ok_or("Orange Aux timing smoke completed without a native save proof")?;
        let rows = scenes
            .timing_cutoff_acceptances()
            .ok_or("Orange Aux timing smoke did not physically publish both Cutoff plateaus")?;
        if rows[0].0 == rows[1].0 || rows[0].1 == rows[1].1 {
            return Err(
                "Orange Aux timing smoke Cutoff evidence did not contain two distinct physical frames"
                    .into(),
            );
        }
        let audio = host
            .take_autoaux_command_evidence()
            .ok_or("Orange Aux timing smoke has no successful host command evidence")?;
        if audio.successful_count == 0
            || audio.distinct_values[0].is_none()
            || audio.distinct_values[1].is_none()
            || audio.distinct_values[0] == audio.distinct_values[1]
        {
            return Err(
                "Orange Aux timing smoke did not host-handle two distinct synth Cutoff values"
                    .into(),
            );
        }
        timing.report(rows, audio, request_id, *elapsed);
    }
    Ok(())
}

fn start_playback(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    for pressed in [true, false] {
        let message = neokey_message(1, pressed)
            .ok_or("Orange timing autoplay NeoKey index 1 unavailable")?;
        dispatch(playback, runner, host, message)?;
    }
    if !playback
        .last_status()
        .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
    {
        return Err("Orange timing autoplay did not enter Playing".into());
    }
    Ok(())
}

fn validate_study_store(path: &str) -> bool {
    let Some(unit) = path.strip_prefix("/var/lib/octessera/study-stores/") else {
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

#[cfg(test)]
#[path = "orange_timing_input_tests.rs"]
mod tests;
