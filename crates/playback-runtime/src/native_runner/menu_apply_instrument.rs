use super::{
    cutoff_display_to_hz, cutoff_hz_to_display, derive_instrument_name, set_json_path_number,
    set_json_path_string, synth_filter_cutoff, synth_filter_resonance, synth_i32_at,
    synth_string_at, value_i32_at, value_string_at, NativeRunner,
};
use platform_core::PAN_POSITION_COUNT;

impl NativeRunner {
    pub(super) fn apply_instrument_menu_state(&mut self) -> bool {
        let mut changed = false;
        for index in 0..self.instruments.len() {
            let Some(instrument) = self.instruments.get_mut(index) else {
                continue;
            };
            let before_name = instrument.name.clone();
            if let Some(kind) = self
                .menu
                .value_for_key(&format!("instruments.{index}.type"))
            {
                if instrument.kind != kind {
                    instrument.kind = kind;
                    if instrument.auto_name {
                        instrument.name = derive_instrument_name(index, &instrument.kind);
                    }
                    changed = true;
                }
            }
            if let Some(note_behavior) = self
                .menu
                .value_for_key(&format!("instruments.{index}.noteBehavior"))
            {
                if instrument.note_behavior != note_behavior {
                    instrument.note_behavior = note_behavior;
                    changed = true;
                }
            }
            if let Some(auto_name) = self
                .menu
                .value_for_key(&format!("instruments.{index}.autoName"))
                .map(|value| value == "true")
            {
                if instrument.auto_name != auto_name {
                    instrument.auto_name = auto_name;
                    if auto_name {
                        instrument.name = derive_instrument_name(index, &instrument.kind);
                    }
                    changed = true;
                }
            }
            if let Some(name) = self
                .menu
                .value_for_key(&format!("instruments.{index}.name"))
            {
                if name != before_name {
                    instrument.name = name;
                    instrument.auto_name = false;
                    changed = true;
                }
            }
            if let Some(volume) = self
                .menu
                .number_for_key(&format!("instruments.{index}.mixer.volume"))
            {
                let volume = volume.clamp(0, 100) as u8;
                if instrument.volume != volume {
                    instrument.volume = volume;
                    changed = true;
                }
            }
            if let Some(pan_pos) = self
                .menu
                .number_for_key(&format!("instruments.{index}.mixer.panPos"))
            {
                let pan_pos = pan_pos.clamp(0, i32::from(PAN_POSITION_COUNT - 1)) as u8;
                if instrument.pan_pos != pan_pos {
                    instrument.pan_pos = pan_pos;
                    changed = true;
                }
            }
            if let Some(route) = self
                .menu
                .value_for_key(&format!("instruments.{index}.mixer.route"))
            {
                if instrument.route != route {
                    instrument.route = route;
                    changed = true;
                }
            }
            if let Some(sample_slot) = self
                .menu
                .value_for_key(&format!("instruments.{index}.sample.selectedSlot"))
                .and_then(|value| value.parse::<usize>().ok())
                .and_then(|value| value.checked_sub(1))
            {
                let sample_slot = sample_slot.min(7);
                if instrument.selected_sample_slot != sample_slot {
                    instrument.selected_sample_slot = sample_slot;
                    changed = true;
                }
            }
            if let Some(gain) = self
                .menu
                .number_for_key(&format!("instruments.{index}.synth.amp.gainPct"))
            {
                let gain = gain.clamp(0, 100) as u8;
                if instrument.synth_gain_pct != gain {
                    instrument.synth_gain_pct = gain;
                    set_json_path_number(
                        &mut instrument.synth_config,
                        &["amp", "gainPct"],
                        f64::from(gain),
                    );
                    changed = true;
                }
            }
            if let Some(waveform) = self
                .menu
                .value_for_key(&format!("instruments.{index}.synth.osc1.waveform"))
            {
                if synth_string_at(instrument, &["osc1", "waveform"], "saw") != waveform {
                    set_json_path_string(
                        &mut instrument.synth_config,
                        &["osc1", "waveform"],
                        &waveform,
                    );
                    changed = true;
                }
            }
            if let Some(waveform) = self
                .menu
                .value_for_key(&format!("instruments.{index}.synth.osc2.waveform"))
            {
                if synth_string_at(instrument, &["osc2", "waveform"], "square") != waveform {
                    set_json_path_string(
                        &mut instrument.synth_config,
                        &["osc2", "waveform"],
                        &waveform,
                    );
                    changed = true;
                }
            }
            if let Some(filter_type) = self
                .menu
                .value_for_key(&format!("instruments.{index}.synth.filter.type"))
            {
                if synth_string_at(instrument, &["filter", "type"], "lowpass") != filter_type {
                    set_json_path_string(
                        &mut instrument.synth_config,
                        &["filter", "type"],
                        &filter_type,
                    );
                    changed = true;
                }
            }
            if let Some(cutoff) = self
                .menu
                .number_for_key(&format!("instruments.{index}.synth.filter.cutoffHz"))
            {
                let cutoff_display = cutoff.clamp(0, 255);
                let cutoff = cutoff_display_to_hz(cutoff_display) as u16;
                if cutoff_hz_to_display(i32::from(synth_filter_cutoff(instrument)))
                    != cutoff_display
                {
                    set_json_path_number(
                        &mut instrument.synth_config,
                        &["filter", "cutoffHz"],
                        f64::from(cutoff),
                    );
                    changed = true;
                }
            }
            if let Some(resonance) = self
                .menu
                .number_for_key(&format!("instruments.{index}.synth.filter.resonance"))
            {
                let resonance = resonance.clamp(0, 255) as u8;
                if synth_filter_resonance(instrument) != resonance {
                    set_json_path_number(
                        &mut instrument.synth_config,
                        &["filter", "resonance"],
                        f64::from(resonance),
                    );
                    changed = true;
                }
            }
            for (suffix, path, min, max) in [
                ("osc1.octave", ["osc1", "octave"], -2, 2),
                ("osc1.levelPct", ["osc1", "levelPct"], 0, 100),
                ("osc1.detuneCents", ["osc1", "detuneCents"], -50, 50),
                ("osc1.pulseWidthPct", ["osc1", "pulseWidthPct"], 5, 95),
                ("osc2.octave", ["osc2", "octave"], -2, 2),
                ("osc2.levelPct", ["osc2", "levelPct"], 0, 100),
                ("osc2.detuneCents", ["osc2", "detuneCents"], -50, 50),
                ("osc2.pulseWidthPct", ["osc2", "pulseWidthPct"], 5, 95),
                ("filter.envAmountPct", ["filter", "envAmountPct"], -100, 100),
                (
                    "filter.keyTrackingPct",
                    ["filter", "keyTrackingPct"],
                    0,
                    100,
                ),
                (
                    "amp.velocitySensitivityPct",
                    ["amp", "velocitySensitivityPct"],
                    0,
                    100,
                ),
                ("ampEnv.attackMs", ["ampEnv", "attackMs"], 0, 5000),
                ("ampEnv.decayMs", ["ampEnv", "decayMs"], 0, 5000),
                ("ampEnv.sustainPct", ["ampEnv", "sustainPct"], 0, 100),
                ("ampEnv.releaseMs", ["ampEnv", "releaseMs"], 0, 10000),
                ("filterEnv.attackMs", ["filterEnv", "attackMs"], 0, 5000),
                ("filterEnv.decayMs", ["filterEnv", "decayMs"], 0, 5000),
                ("filterEnv.sustainPct", ["filterEnv", "sustainPct"], 0, 100),
                ("filterEnv.releaseMs", ["filterEnv", "releaseMs"], 0, 10000),
            ] {
                if let Some(value) = self
                    .menu
                    .number_for_key(&format!("instruments.{index}.synth.{suffix}"))
                {
                    let value = value.clamp(min, max);
                    if synth_i32_at(instrument, &path, i32::MIN) != value {
                        set_json_path_number(&mut instrument.synth_config, &path, f64::from(value));
                        changed = true;
                    }
                }
            }
            if let Some(tune) = self
                .menu
                .number_for_key(&format!("instruments.{index}.sample.tuneSemis"))
            {
                let tune = tune.clamp(-24, 24) as i8;
                if instrument.sample_tune_semis != tune {
                    instrument.sample_tune_semis = tune;
                    changed = true;
                }
            }
            if let Some(gain) = self
                .menu
                .number_for_key(&format!("instruments.{index}.sample.amp.gainPct"))
            {
                let gain = gain.clamp(0, 100) as u8;
                if instrument.sample_gain_pct != gain {
                    instrument.sample_gain_pct = gain;
                    changed = true;
                }
            }
            if let Some(base_velocity) = self
                .menu
                .number_for_key(&format!("instruments.{index}.sample.baseVelocity"))
            {
                let base_velocity = base_velocity.clamp(1, 127) as u8;
                if instrument.sample_base_velocity != base_velocity {
                    instrument.sample_base_velocity = base_velocity;
                    changed = true;
                }
            }
            if let Some(velocity_sens) = self.menu.number_for_key(&format!(
                "instruments.{index}.sample.amp.velocitySensitivityPct"
            )) {
                let velocity_sens = velocity_sens.clamp(0, 100) as u8;
                if instrument.sample_amp_velocity_sensitivity_pct != velocity_sens {
                    instrument.sample_amp_velocity_sensitivity_pct = velocity_sens;
                    changed = true;
                }
            }
            if let Some(enabled) = self
                .menu
                .value_for_key(&format!("instruments.{index}.sample.velocityLevelsEnabled"))
                .map(|value| value == "true")
            {
                if instrument.sample_velocity_levels_enabled != enabled {
                    instrument.sample_velocity_levels_enabled = enabled;
                    changed = true;
                }
            }
            for (suffix, target, min, max) in [
                ("velocityLevels.high", "high", 1, 127),
                ("velocityLevels.medium", "medium", 1, 127),
                ("velocityLevels.low", "low", 1, 127),
            ] {
                if let Some(value) = self
                    .menu
                    .number_for_key(&format!("instruments.{index}.sample.{suffix}"))
                {
                    let value = value.clamp(min, max) as u8;
                    let current = match target {
                        "high" => &mut instrument.sample_velocity_high,
                        "medium" => &mut instrument.sample_velocity_medium,
                        _ => &mut instrument.sample_velocity_low,
                    };
                    if *current != value {
                        *current = value;
                        changed = true;
                    }
                }
            }
            if let Some(filter_type) = self
                .menu
                .value_for_key(&format!("instruments.{index}.sample.filter.type"))
            {
                if value_string_at(&instrument.sample_filter, &["type"], "lowpass") != filter_type {
                    set_json_path_string(&mut instrument.sample_filter, &["type"], &filter_type);
                    changed = true;
                }
            }
            for (suffix, path, min, max) in [
                ("filter.cutoffHz", ["cutoffHz"], 0, 255),
                ("filter.resonance", ["resonance"], 0, 255),
                ("filter.envAmountPct", ["envAmountPct"], -100, 100),
                ("filter.keyTrackingPct", ["keyTrackingPct"], 0, 100),
            ] {
                if let Some(value) = self
                    .menu
                    .number_for_key(&format!("instruments.{index}.sample.{suffix}"))
                {
                    let value = value.clamp(min, max);
                    let stored_value = if suffix == "filter.cutoffHz" {
                        cutoff_display_to_hz(value)
                    } else {
                        value
                    };
                    let current = value_i32_at(&instrument.sample_filter, &path, i32::MIN);
                    let unchanged = if suffix == "filter.cutoffHz" {
                        cutoff_hz_to_display(current) == value
                    } else {
                        current == stored_value
                    };
                    if !unchanged {
                        set_json_path_number(
                            &mut instrument.sample_filter,
                            &path,
                            f64::from(stored_value),
                        );
                        changed = true;
                    }
                }
            }
            for (prefix_key, target, path, min, max) in [
                ("ampEnv", "amp", ["attackMs"], 0, 5000),
                ("ampEnv", "amp", ["decayMs"], 0, 5000),
                ("ampEnv", "amp", ["sustainPct"], 0, 100),
                ("ampEnv", "amp", ["releaseMs"], 0, 10000),
                ("filterEnv", "filter", ["attackMs"], 0, 5000),
                ("filterEnv", "filter", ["decayMs"], 0, 5000),
                ("filterEnv", "filter", ["sustainPct"], 0, 100),
                ("filterEnv", "filter", ["releaseMs"], 0, 10000),
            ] {
                let field = path[0];
                if let Some(value) = self
                    .menu
                    .number_for_key(&format!("instruments.{index}.sample.{prefix_key}.{field}"))
                {
                    let value = value.clamp(min, max);
                    let config = if target == "amp" {
                        &mut instrument.sample_amp_env
                    } else {
                        &mut instrument.sample_filter_env
                    };
                    if value_i32_at(config, &path, i32::MIN) != value {
                        set_json_path_number(config, &path, f64::from(value));
                        changed = true;
                    }
                }
            }
            if let Some(enabled) = self
                .menu
                .value_for_key(&format!("instruments.{index}.midi.enabled"))
                .map(|value| value == "true")
            {
                if instrument.midi_enabled != enabled {
                    instrument.midi_enabled = enabled;
                    changed = true;
                }
            }
            if let Some(channel) = self
                .menu
                .number_for_key(&format!("instruments.{index}.midi.channel"))
            {
                let channel = channel.clamp(1, 16) as u8;
                if instrument.midi_channel != channel {
                    instrument.midi_channel = channel;
                    changed = true;
                }
            }
            if let Some(velocity) = self
                .menu
                .number_for_key(&format!("instruments.{index}.midi.velocity"))
            {
                let velocity = velocity.clamp(1, 127) as u8;
                if instrument.midi_velocity != velocity {
                    instrument.midi_velocity = velocity;
                    changed = true;
                }
            }
            if let Some(duration_ms) = self
                .menu
                .number_for_key(&format!("instruments.{index}.midi.durationMs"))
            {
                let duration_ms = duration_ms.clamp(10, 2000) as u16;
                if instrument.midi_duration_ms != duration_ms {
                    instrument.midi_duration_ms = duration_ms;
                    changed = true;
                }
            }
        }
        changed
    }
}
