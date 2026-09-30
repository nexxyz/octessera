use crate::autoaux_menu;
use crate::host_adapter::PiPlaybackHostAdapter;
use crate::raspberry_native_scene::NativeScenePump;
use playback_runtime::{
    HostMessage, NativeRunner, PlaybackRuntime, RuntimeStoreResult, RuntimeTransportState,
};
use std::path::Path;
use std::time::Instant;

const AUTOAUX_ENV: &str = "OCTESSERA_PI_TIMING_AUTOAUX";
const UI_PROFILE_ENV: &str = "OCTESSERA_PI_UI_PROFILE";
const KEEP_AWAKE_ENV: &str = "OCTESSERA_PI_TIMING_KEEP_AWAKE";
const STORE_DIR_ENV: &str = "OCTESSERA_PI_STORE_DIR";

impl PiPlaybackHostAdapter {
    pub(crate) fn begin_autoaux_evidence(&mut self) {
        self.autoaux_audio_evidence = Some(Default::default());
        self.autoaux_store_result = None;
        self.autoaux_expected_revision = None;
    }

    pub(crate) fn autoaux_active(&self) -> bool {
        self.autoaux_audio_evidence.is_some()
    }

    pub(crate) fn take_autoaux_audio_evidence(
        &mut self,
    ) -> Option<crate::host_adapter::RaspberryAutoAuxAudioEvidence> {
        self.autoaux_audio_evidence.take()
    }

    pub(crate) fn finish_autoaux_evidence(&mut self) {
        self.autoaux_store_result = None;
        self.autoaux_expected_revision = None;
        self.autoaux_audio_evidence = None;
    }

    pub(crate) fn observe_autoaux_store_result(
        &mut self,
        result: RuntimeStoreResult,
        accepted_at: Instant,
    ) {
        if !self.autoaux_active() {
            return;
        }
        let Some(revision) = identified_revision(&result) else {
            return;
        };
        if self
            .autoaux_expected_revision
            .is_some_and(|expected| expected != revision)
        {
            return;
        }
        self.autoaux_store_result = Some((result, accepted_at));
    }

    pub(crate) fn set_autoaux_expected_revision(&mut self, revision: Option<u64>) {
        self.autoaux_expected_revision = revision;
        if revision.is_some_and(|expected| {
            self.autoaux_store_result
                .as_ref()
                .is_some_and(|(result, _)| identified_revision(result) != Some(expected))
        }) {
            self.autoaux_store_result = None;
        }
    }

    pub(crate) fn take_autoaux_store_result(&mut self) -> Option<(RuntimeStoreResult, Instant)> {
        self.autoaux_store_result.take()
    }
}

fn identified_revision(result: &RuntimeStoreResult) -> Option<u64> {
    match result {
        RuntimeStoreResult::Identified {
            revision: Some(revision),
            ..
        } => Some(*revision),
        _ => None,
    }
}

pub(crate) struct RaspberryAutoAux {
    sequence: crate::autoaux_sequence::AutoAuxSequence,
    starting_cutoff: u16,
}

impl RaspberryAutoAux {
    pub(crate) fn prepare(
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        adapter: &mut PiPlaybackHostAdapter,
        scenes: &mut NativeScenePump,
        profiler_enabled: bool,
    ) -> Result<Option<Self>, String> {
        match std::env::var_os(AUTOAUX_ENV) {
            None => return Ok(None),
            Some(value) if value != "1" => return Err(format!("{AUTOAUX_ENV} must be exactly 1")),
            Some(_) => {}
        }
        validate_gates(playback, adapter, profiler_enabled)?;
        if !runner.is_canonical_menu_presentation() {
            return Err("Raspberry AutoAux requires the canonical normal menu".into());
        }
        autoaux_menu::require_stopped_normal_menu(playback, "Raspberry")?;
        let (starting_cutoff, targets) = {
            let mut dispatch = |playback: &mut PlaybackRuntime,
                                runner: &mut NativeRunner,
                                message: HostMessage| {
                crate::runtime_loop::dispatch_runtime_message(playback, runner, adapter, message)
            };
            autoaux_menu::enable_study_auto_save(playback, runner, &mut dispatch, "Raspberry")?;
            autoaux_menu::navigate_to_cutoff(playback, runner, &mut dispatch, "Raspberry")?;
            autoaux_menu::preflight_aux_cutoff(playback, runner, &mut dispatch, "Raspberry")?
        };
        for pressed in [true, false] {
            let message = crate::input::neokey_message(1, pressed)
                .ok_or("Raspberry AutoAux Play key unavailable")?;
            crate::runtime_loop::dispatch_runtime_message(playback, runner, adapter, message)?;
        }
        if !playback
            .last_status()
            .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
        {
            return Err("Raspberry AutoAux did not enter Playing".into());
        }
        adapter.begin_autoaux_evidence();
        scenes.begin_autoaux_cutoff_evidence(targets);
        Ok(Some(Self {
            sequence: crate::autoaux_sequence::AutoAuxSequence::new(Instant::now()),
            starting_cutoff,
        }))
    }

    pub(crate) fn tick(
        &mut self,
        now: Instant,
        playback: &mut PlaybackRuntime,
        runner: &mut NativeRunner,
        adapter: &mut PiPlaybackHostAdapter,
        scenes: &NativeScenePump,
    ) -> Result<bool, String> {
        if !playback
            .last_status()
            .is_some_and(|status| status.transport == RuntimeTransportState::Playing)
        {
            return Err("Raspberry AutoAux left Playing unexpectedly".into());
        }
        if matches!(
            self.sequence.phase,
            crate::autoaux_sequence::Phase::AwaitSave { .. }
        ) && self.sequence.final_revision.is_none()
        {
            self.sequence.final_revision = runner
                .persistence_intent_at(now)
                .filter(|intent| intent.default_eligible())
                .map(|intent| intent.revision());
        }
        adapter.set_autoaux_expected_revision(self.sequence.final_revision);
        self.accept_buffered_store_result(adapter)?;
        let action = self
            .sequence
            .next_action(now, runner)
            .map_err(raspberry_sequence_error)?;
        adapter.set_autoaux_expected_revision(self.sequence.final_revision);
        let completed = match action {
            crate::autoaux_sequence::AutoAuxAction::None => false,
            crate::autoaux_sequence::AutoAuxAction::Turn(delta) => {
                crate::runtime_loop::dispatch_runtime_message(
                    playback,
                    runner,
                    adapter,
                    crate::input::encoder_turn_message("encoder_aux_1", delta),
                )?;
                self.sequence.turn_issued(now);
                false
            }
            crate::autoaux_sequence::AutoAuxAction::Completed => {
                let rows = scenes
                    .autoaux_cutoff_acceptances()
                    .ok_or("Raspberry AutoAux did not receive two physical Cutoff frames")?;
                if rows[0].0 == rows[1].0 || rows[0].1 == rows[1].1 {
                    return Err("Raspberry AutoAux physical Cutoff evidence is not distinct".into());
                }
                let audio = adapter
                    .take_autoaux_audio_evidence()
                    .ok_or("Raspberry AutoAux has no successful synth command evidence")?;
                if !audio.has_two_values() {
                    return Err(
                        "Raspberry AutoAux did not host-handle two distinct synth Cutoff values"
                            .into(),
                    );
                }
                let (request, elapsed) = self
                    .sequence
                    .save_completion
                    .as_ref()
                    .ok_or("Raspberry AutoAux completed without automatic save evidence")?;
                let values = audio.distinct_values();
                eprintln!(
                    "raspberry-autoaux cutoff_start={} oled_cutoff_a={} oled_frame_a={} oled_cutoff_b={} oled_frame_b={} synth_cutoff_commands={} synth_cutoff_a={} synth_cutoff_b={} save_revision={} save_request={} save_elapsed_ms={} aux_turns={} rapid_turns={} missed_turns={}",
                    self.starting_cutoff,
                    rows[0].1,
                    rows[0].0,
                    rows[1].1,
                    rows[1].0,
                    audio.successful_count,
                    values[0].unwrap_or_default(),
                    values[1].unwrap_or_default(),
                    self.sequence.final_revision.unwrap_or_default(),
                    request,
                    elapsed.as_millis(),
                    self.sequence.aux_turns,
                    self.sequence.rapid_turns,
                    self.sequence.missed_turns,
                );
                adapter.finish_autoaux_evidence();
                true
            }
        };
        Ok(completed)
    }

    fn accept_buffered_store_result(
        &mut self,
        adapter: &mut PiPlaybackHostAdapter,
    ) -> Result<(), String> {
        if self.sequence.final_revision.is_none() {
            return Ok(());
        }
        let Some((result, accepted_at)) = adapter.take_autoaux_store_result() else {
            return Ok(());
        };
        if let (
            crate::autoaux_sequence::Phase::AwaitSave { started },
            RuntimeStoreResult::Identified { result, .. },
        ) = (self.sequence.phase, &result)
        {
            if matches!(
                result.as_ref(),
                RuntimeStoreResult::SaveDefaultResult {
                    ok: true,
                    is_auto: Some(true),
                }
            ) && accepted_at.saturating_duration_since(started)
                >= crate::autoaux_sequence::SAVE_COMPLETION_TIMEOUT
            {
                return Err(raspberry_sequence_error(
                    "Orange Aux timing smoke timed out waiting for the final automatic default save completion".into(),
                ));
            }
        }
        self.sequence
            .accept_store_result(&result, accepted_at)
            .map_err(raspberry_sequence_error)
    }
}

pub(crate) fn fail_autoaux(message: impl std::fmt::Display) -> ! {
    eprintln!("raspberry-autoaux-failed: {message}");
    std::process::exit(2)
}

fn raspberry_sequence_error(error: String) -> String {
    error.replace("Orange Aux timing smoke", "Raspberry AutoAux")
}

fn validate_gates(
    playback: &PlaybackRuntime,
    adapter: &PiPlaybackHostAdapter,
    profiler_enabled: bool,
) -> Result<(), String> {
    if crate::board_profile::BOARD_PROFILE_ID != "raspberry-pi-zero-2w" {
        return Err("Raspberry AutoAux requires the Raspberry Pi Zero 2W profile".into());
    }
    if !profiler_enabled || std::env::var(UI_PROFILE_ENV).as_deref() != Ok("1") {
        return Err(format!("Raspberry AutoAux requires {UI_PROFILE_ENV}=1"));
    }
    if std::env::var(KEEP_AWAKE_ENV).as_deref() != Ok("1") {
        return Err(format!("Raspberry AutoAux requires {KEEP_AWAKE_ENV}=1"));
    }
    let store = std::env::var(STORE_DIR_ENV)
        .map_err(|_| "Raspberry AutoAux requires an isolated study store".to_string())?;
    if !validate_study_store(Path::new(&store)) {
        return Err("Raspberry AutoAux store is not an isolated study clone".into());
    }
    if adapter.audio_service().is_none() {
        return Err("Raspberry AutoAux requires the internal audio host".into());
    }
    let snapshot = playback
        .last_snapshot()
        .ok_or("Raspberry AutoAux has no startup snapshot")?;
    if !crate::normal_menu::is_normal_menu_snapshot(snapshot)
        || snapshot["display"]["off"] != false
        || snapshot["settings"]["dimTimerSeconds"] != 0
        || snapshot["settings"]["screenSleepSeconds"] != 0
        || snapshot["settings"]["ledsDimmed"] != false
    {
        return Err("Raspberry AutoAux requires the awake normal menu".into());
    }
    Ok(())
}

fn validate_study_store(path: &Path) -> bool {
    if path == Path::new("/home/pi/presets") {
        return false;
    }
    let Some(unit) = path
        .to_str()
        .and_then(|path| path.strip_prefix("/var/lib/octessera/study-stores/"))
    else {
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
#[path = "raspberry_autoaux_tests.rs"]
mod tests;
