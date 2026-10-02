use crate::protocol::{HostMessage, RunnerMessage, RuntimePlatformRequest, RuntimeStoreResult};
use std::time::Instant;

use super::{DeviceInput, NativeRunner, RuntimeTransportState};

impl NativeRunner {
    pub(crate) fn send_system_store_result_music_first(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>), String> {
        let was_deferred = self.pending.external_autosave_deferred;
        self.pending.external_autosave_deferred = true;
        let result = <Self as super::CoreRunner>::send_system_store_result(self, message);
        self.pending.external_autosave_deferred = was_deferred;
        result
    }

    pub(crate) fn send_store_result_handoff_music_first(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>, bool), String> {
        let was_deferred = self.pending.external_autosave_deferred;
        self.pending.external_autosave_deferred = true;
        let result = <Self as super::CoreRunner>::send_store_result_handoff(self, message);
        self.pending.external_autosave_deferred = was_deferred;
        result
    }

    pub fn send_music_first(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        self.pending.external_autosave_deferred = true;
        let result = self.send_music_first_scoped(message);
        self.pending.external_autosave_deferred = false;
        result
    }

    fn send_music_first_scoped(
        &mut self,
        message: HostMessage,
    ) -> Result<Vec<RunnerMessage>, String> {
        let deferred_persistence_result = !self.restart_settings.is_saving()
            && matches!(&message, HostMessage::RuntimeResult { result }
                if is_successful_manual_save_result(self, result));
        let supported_music_input = matches!(
            &message,
            HostMessage::TransportPulseStep { .. }
                | HostMessage::MidiRealtimeClock { .. }
                | HostMessage::DeviceInput { .. }
        );
        if self.transport.transport != RuntimeTransportState::Playing
            || self.display.runtime_error_presentation.is_some()
            || !(supported_music_input || deferred_persistence_result)
            || matches!(&message, HostMessage::DeviceInput { input, .. }
                if !matches!(input.get("type").and_then(serde_json::Value::as_str),
                    Some("encoder_turn" | "encoder_press" | "grid_press" | "grid_release"
                        | "button_a" | "button_s" | "button_shift" | "button_fn" | "button_combined_modifier")))
        {
            return <Self as super::CoreRunner>::send(self, message)
                .map(order_terminal_presentation);
        }
        if let HostMessage::DeviceInput {
            input,
            request_snapshot: Some(false),
        } = &message
        {
            if input.get("type").and_then(serde_json::Value::as_str) != Some("encoder_turn")
                || input.get("delta").and_then(serde_json::Value::as_i64) != Some(0)
            {
                self.display.transients.mark_presentation_due();
            }
        }
        self.pending.presentation_deferred = true;
        let result = <Self as super::CoreRunner>::send(self, message);
        self.pending.presentation_deferred = false;
        result.map(order_terminal_presentation)
    }

    pub(super) fn require_synchronous_action_presentation(&mut self) {
        self.pending.presentation_deferred = false;
        self.pending.suppress_snapshot_response = false;
    }

    fn send_device_input(
        &mut self,
        input: serde_json::Value,
        request_snapshot: Option<bool>,
    ) -> Result<Vec<RunnerMessage>, String> {
        let input = serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
        if request_snapshot.unwrap_or(true) {
            return self.handle_device_input(input);
        }
        self.pending.suppress_snapshot_response = true;
        let messages = self.handle_device_input(input);
        self.pending.suppress_snapshot_response = false;
        messages
    }

    fn send_presented_runtime_error_input(
        &mut self,
        input: serde_json::Value,
    ) -> Result<Vec<RunnerMessage>, String> {
        let input = serde_json::from_value::<DeviceInput>(input).unwrap_or(DeviceInput::Other);
        self.handle_presented_runtime_error_input(input)
    }
}

fn is_successful_manual_save_result(runner: &NativeRunner, result: &RuntimeStoreResult) -> bool {
    let RuntimeStoreResult::Identified {
        result,
        request_id,
        revision: Some(revision),
    } = result
    else {
        return false;
    };
    match result.as_ref() {
        RuntimeStoreResult::SaveDefaultResult { ok: true, .. } => runner
            .restart_settings
            .native_write_matches(request_id, *revision),
        RuntimeStoreResult::SavePresetResult { name, .. } => {
            runner.native_preset_write_matches(request_id, *revision, name)
        }
        RuntimeStoreResult::SaveBackupResult { ok: true } => true,
        _ => false,
    }
}

fn order_terminal_presentation(mut messages: Vec<RunnerMessage>) -> Vec<RunnerMessage> {
    let Some(terminal) = messages.iter().position(|message| matches!(message,
        RunnerMessage::PlatformEffects { effects } if effects.iter().any(|effect| matches!(effect,
            crate::RuntimePlatformEffect::Shutdown | crate::RuntimePlatformEffect::Reboot))
    )) else { return messages; };
    let Some(snapshot) = messages.iter().position(|message| {
        matches!(message,
            RunnerMessage::Snapshot { snapshot } if snapshot["display"]["splash"] == "shutdown"
        )
    }) else {
        return messages;
    };
    if terminal < snapshot {
        let effect = messages.remove(terminal);
        messages.insert(snapshot, effect);
    }
    messages
}

impl super::CoreRunner for NativeRunner {
    fn register_platform_request(&mut self, request: &RuntimePlatformRequest) {
        if request.operation() == crate::RuntimeOperation::StoreSaveDefault {
            self.register_default_write_request(&request.request_id, request.revision);
        }
        let apply_scope = if request.operation() == crate::RuntimeOperation::StoreSaveSystem {
            self.restart_settings
                .pending_write_scope()
                .filter(|scope| scope.is_restart())
        } else {
            None
        };
        if request.operation() == crate::RuntimeOperation::StoreLoadDefault {
            if self.restore_rehydration_pending() {
                self.register_restore_patch_request(request);
            } else {
                self.pending.pending_default_load_request =
                    Some((request.request_id.clone(), request.revision));
            }
        }
        if apply_scope.is_some() {
            let expected = self
                .restart_settings
                .pending_write_payload()
                .expect("System Apply has a captured payload");
            let crate::RuntimePlatformEffect::StoreSaveSystem { payload } = &request.effect else {
                return;
            };
            if payload != expected.as_ref() {
                return;
            }
            self.register_default_write_request(&request.request_id, request.revision);
        }
        self.pending
            .system_persistence
            .register_request(request, apply_scope);
    }

    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        if matches!(&message, HostMessage::RuntimeResult { result }
            if super::store::is_system_store_operation(&result.operation()))
        {
            return <Self as super::CoreRunner>::send_system_store_result(self, message)
                .map(|(messages, _)| messages);
        }
        if matches!(&message, HostMessage::RuntimeResult { result }
            if result.operation() == crate::RuntimeOperation::StoreLoadDefault)
        {
            return <Self as super::CoreRunner>::send_store_result_handoff(self, message)
                .map(|(messages, _, _)| messages);
        }
        let flush_time = Instant::now();
        self.advance_xy_smoothing_at(flush_time)?;
        let presented_error_input =
            matches!(&message, HostMessage::PresentedRuntimeErrorInput { .. });
        let mut messages = match message {
            HostMessage::TransportPulseStep {
                pulses,
                request_snapshot,
                ..
            } => self.send_transport_pulse_step(pulses, request_snapshot),
            HostMessage::DeviceInput {
                input,
                request_snapshot,
            } => self.send_device_input(input, request_snapshot),
            HostMessage::PresentedRuntimeErrorInput { input } => {
                self.send_presented_runtime_error_input(input)
            }
            HostMessage::MidiRealtimeStart => self.send_midi_realtime_start(),
            HostMessage::MidiRealtimeContinue => self.send_midi_realtime_continue(),
            HostMessage::MidiRealtimeStop => self.send_midi_realtime_stop(),
            HostMessage::TransportStop => self.send_transport_stop(),
            HostMessage::MidiRealtimeClock { pulses } => self.send_midi_realtime_clock(pulses),
            HostMessage::RuntimeResult { result } => {
                let is_default_load =
                    result.operation() == crate::RuntimeOperation::StoreLoadDefault;
                let request_id = restore_result_request_id(&result).map(str::to_owned);
                let revision = restore_result_revision(&result);
                let messages = self.send_runtime_result(result)?;
                if is_default_load
                    && self
                        .pending
                        .pending_default_load_request
                        .as_ref()
                        .is_some_and(|(expected_id, expected_revision)| {
                            request_id.as_deref() == Some(expected_id.as_str())
                                && revision == *expected_revision
                        })
                {
                    self.pending.pending_default_load_request = None;
                }
                Ok(messages)
            }
        }?;
        if presented_error_input {
            return Ok(messages);
        }
        if self.pending.presentation_deferred {
            messages.extend(self.flush_deferred_menu_apply_music_first(flush_time)?);
        } else {
            messages.extend(self.flush_deferred_menu_apply_at(flush_time)?);
        }
        self.append_runtime_config_if_changed(&mut messages);
        Ok(messages)
    }

    fn send_system_store_result(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>), String> {
        let HostMessage::RuntimeResult { result } = message else {
            return Err("System store handoff requires a runtime result".into());
        };
        let had_runtime_error = self.display.runtime_error_presentation.is_some();
        let save_flash_serial = self.display.auto_save_flash_serial;
        let accepted = self.apply_system_store_result(result)?;
        let Some(accepted) = accepted else {
            return Ok((Vec::new(), None));
        };
        let messages = self.store_result_handoff_messages(had_runtime_error, save_flash_serial)?;
        Ok((messages, Some(accepted)))
    }

    fn send_store_result_handoff(
        &mut self,
        message: HostMessage,
    ) -> Result<(Vec<RunnerMessage>, Option<RuntimeStoreResult>, bool), String> {
        let result = match message {
            HostMessage::RuntimeResult { result } => result,
            message => return self.send(message).map(|messages| (messages, None, false)),
        };
        let operation = result.operation();
        if super::store::is_system_store_operation(&operation) {
            let (messages, accepted) = <Self as super::CoreRunner>::send_system_store_result(
                self,
                HostMessage::RuntimeResult { result },
            )?;
            return Ok((messages, accepted, false));
        }
        if operation != crate::RuntimeOperation::StoreLoadDefault {
            return self
                .send(HostMessage::RuntimeResult { result })
                .map(|messages| (messages, None, false));
        }

        let had_runtime_error = self.display.runtime_error_presentation.is_some();
        let save_flash_serial = self.display.auto_save_flash_serial;
        let restore_patch_result = self.routes_restore_patch_result(
            restore_result_request_id(&result),
            restore_result_revision(&result),
        );
        if restore_patch_result {
            let Some((accepted, rehydration_accepted)) =
                self.apply_restored_patch_result(result)?
            else {
                return Ok((Vec::new(), None, false));
            };
            let messages =
                self.store_result_handoff_messages(had_runtime_error, save_flash_serial)?;
            return Ok((messages, Some(accepted), rehydration_accepted));
        };
        let request_id = restore_result_request_id(&result).map(str::to_owned);
        let revision = restore_result_revision(&result);
        let accepted = self.apply_default_patch_result_for_runtime(result)?;
        if self
            .pending
            .pending_default_load_request
            .as_ref()
            .is_some_and(|(expected_id, expected_revision)| {
                request_id.as_deref() == Some(expected_id.as_str())
                    && revision == *expected_revision
            })
        {
            self.pending.pending_default_load_request = None;
        }
        let messages = self.store_result_handoff_messages(had_runtime_error, save_flash_serial)?;
        Ok((messages, Some(accepted), false))
    }
}

impl NativeRunner {
    fn store_result_handoff_messages(
        &mut self,
        had_runtime_error: bool,
        save_flash_serial: u64,
    ) -> Result<Vec<RunnerMessage>, String> {
        let mut messages = if self.pending.presentation_deferred {
            if !had_runtime_error && self.display.runtime_error_presentation.is_some() {
                self.require_synchronous_action_presentation();
                self.messages_with_snapshot()?
            } else {
                if self.display.auto_save_flash_serial != save_flash_serial {
                    self.display.transients.mark_presentation_due();
                }
                self.messages_without_presentation()?
            }
        } else {
            self.messages_with_snapshot()?
        };
        if self.pending.presentation_deferred {
            messages.extend(self.flush_deferred_menu_apply_music_first(Instant::now())?);
        } else {
            messages.extend(self.flush_deferred_menu_apply_at(Instant::now())?);
        }
        self.append_runtime_config_if_changed(&mut messages);
        Ok(messages)
    }
}

fn restore_result_request_id(result: &RuntimeStoreResult) -> Option<&str> {
    match result {
        RuntimeStoreResult::Identified { request_id, .. } => Some(request_id),
        RuntimeStoreResult::RuntimeFailure { error } => error.request_id.as_deref(),
        _ => None,
    }
}

fn restore_result_revision(result: &RuntimeStoreResult) -> Option<u64> {
    match result {
        RuntimeStoreResult::Identified { revision, .. } => *revision,
        RuntimeStoreResult::RuntimeFailure { error } => error.revision,
        _ => None,
    }
}
