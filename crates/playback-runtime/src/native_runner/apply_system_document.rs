use super::{AudioOptimization, AudioOutputSet, NativeRunner, UsbDataRole, Value};

impl NativeRunner {
    pub(super) fn apply_system_document_preserving_patch(
        &mut self,
        system: &Value,
    ) -> Result<(), String> {
        let current = self.config_payload();
        let documents = super::split_local_system_patch_documents(&current)?;
        let full = super::compose_local_system_patch_documents(system, &documents.patch)?;
        let runtime = full
            .get("runtimeConfig")
            .ok_or_else(|| "System document is missing runtimeConfig".to_string())?;

        let optimization = runtime
            .get("sound")
            .and_then(|sound| sound.get("optimizeFor"))
            .and_then(Value::as_str)
            .and_then(AudioOptimization::from_wire_name)
            .ok_or_else(|| "System document has an invalid audio optimization".to_string())?;
        if !optimization.is_supported(self.audio_optimization_capacity_available) {
            return Err("audio optimization capacity is unavailable".into());
        }
        let dsp = realtime_engine::synth::DspRuntimeConfig::from_value(
            runtime
                .get("dsp")
                .ok_or_else(|| "System document is missing DSP settings".to_string())?,
        )?;
        let audio_outputs = AudioOutputSet::decode(
            runtime
                .get("audioOutputs")
                .ok_or_else(|| "System document is missing audio outputs".to_string())?,
        )?;
        if self.jack_audio_required && !audio_outputs.dac() {
            return Err(super::JACK_AUDIO_REQUIRED_MESSAGE.into());
        }
        let usb_data_role = runtime
            .get("usb")
            .and_then(|usb| usb.get("dataRole"))
            .and_then(Value::as_str)
            .and_then(UsbDataRole::from_menu_value)
            .ok_or_else(|| "System document has an invalid USB data role".to_string())?;
        if usb_data_role.is_host()
            && (audio_outputs.usb()
                || runtime
                    .get("usb")
                    .and_then(|usb| usb.get("midiOutEnabled"))
                    .and_then(Value::as_bool)
                    .unwrap_or(false))
        {
            return Err("USB host role conflicts with USB audio or MIDI output".into());
        }

        let before = self.configuration_aggregate();
        self.apply_system_sound_payload(runtime);
        self.dsp_config = dsp;
        self.apply_display_payload(runtime);
        self.apply_runtime_input_payload(runtime);
        self.apply_aux_mapping_payload(runtime);
        self.apply_midi_payload(runtime);
        self.apply_usb_payload(runtime);
        self.audio_outputs = audio_outputs;
        self.apply_recording_payload(runtime);
        self.apply_hdmi_payload(runtime);
        self.apply_bluetooth_payload(runtime);
        self.apply_sample_browser_favourites_payload(runtime);
        let plan = before.resolve_plan(&self.configuration_aggregate(), self.audio_config_revision);
        self.commit_configuration_runtime_plan(&plan);
        self.enqueue_configuration_runtime_plan(plan);
        self.menu.rebuild(self.menu_config());

        Ok(())
    }
}
