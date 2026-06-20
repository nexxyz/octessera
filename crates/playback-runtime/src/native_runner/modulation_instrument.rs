use super::{
    cutoff_display_to_hz, set_json_path_number, set_json_path_string, synth_string_at,
    value_string_at, Value, PAN_POSITION_COUNT,
};

pub(super) fn apply_instrument_binding_value(
    instrument: &mut super::NativeInstrumentSlot,
    field: &str,
    value: Value,
    config_dirty: &mut bool,
) {
    match field {
        "type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if instrument.kind != value {
                instrument.kind = value.into();
            } else {
                return;
            }
        }
        "noteBehavior" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if instrument.note_behavior != value {
                instrument.note_behavior = value.into();
            } else {
                return;
            }
        }
        "mixer.route" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if instrument.route != value {
                instrument.route = value.into();
            } else {
                return;
            }
        }
        "synth.osc1.waveform" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if synth_string_at(instrument, &["osc1", "waveform"], "saw") != value {
                set_json_path_string(&mut instrument.synth_config, &["osc1", "waveform"], value);
            } else {
                return;
            }
        }
        "synth.osc2.waveform" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if synth_string_at(instrument, &["osc2", "waveform"], "square") != value {
                set_json_path_string(&mut instrument.synth_config, &["osc2", "waveform"], value);
            } else {
                return;
            }
        }
        "synth.filter.type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if synth_string_at(instrument, &["filter", "type"], "lowpass") != value {
                set_json_path_string(&mut instrument.synth_config, &["filter", "type"], value);
            } else {
                return;
            }
        }
        "sample.filter.type" => {
            let Some(value) = value.as_str() else {
                return;
            };
            if value_string_at(&instrument.sample_filter, &["type"], "lowpass") != value {
                set_json_path_string(&mut instrument.sample_filter, &["type"], value);
            } else {
                return;
            }
        }
        "midi.enabled" => {
            let Some(value) = value.as_bool() else {
                return;
            };
            instrument.midi_enabled = value;
        }
        "sample.velocityLevelsEnabled" => {
            let Some(value) = value.as_bool() else {
                return;
            };
            instrument.sample_velocity_levels_enabled = value;
        }
        _ => {
            let Some(value) = value.as_f64() else {
                return;
            };
            match field {
                "mixer.volume" => instrument.volume = value.round().clamp(0.0, 127.0) as u8,
                "mixer.panPos" => {
                    instrument.pan_pos =
                        value.round().clamp(0.0, f64::from(PAN_POSITION_COUNT - 1)) as u8
                }
                "synth.amp.gainPct" => {
                    instrument.synth_gain_pct = value.round().clamp(0.0, 100.0) as u8
                }
                "synth.filter.cutoffHz" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "cutoffHz"],
                    f64::from(cutoff_display_to_hz(value.round() as i32)),
                ),
                "synth.filter.resonance" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "resonance"],
                    value.round().clamp(0.0, 255.0),
                ),
                "synth.osc1.octave" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "octave"],
                    value.round().clamp(-2.0, 2.0),
                ),
                "synth.osc1.levelPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "levelPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.osc1.detuneCents" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "detuneCents"],
                    value.round().clamp(-50.0, 50.0),
                ),
                "synth.osc1.pulseWidthPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc1", "pulseWidthPct"],
                    value.round().clamp(5.0, 95.0),
                ),
                "synth.osc2.octave" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "octave"],
                    value.round().clamp(-2.0, 2.0),
                ),
                "synth.osc2.levelPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "levelPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.osc2.detuneCents" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "detuneCents"],
                    value.round().clamp(-50.0, 50.0),
                ),
                "synth.osc2.pulseWidthPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["osc2", "pulseWidthPct"],
                    value.round().clamp(5.0, 95.0),
                ),
                "synth.filter.envAmountPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "envAmountPct"],
                    value.round().clamp(-100.0, 100.0),
                ),
                "synth.filter.keyTrackingPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filter", "keyTrackingPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.amp.velocitySensitivityPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["amp", "velocitySensitivityPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.ampEnv.attackMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.ampEnv.decayMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.ampEnv.sustainPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.ampEnv.releaseMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["ampEnv", "releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "synth.filterEnv.attackMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.filterEnv.decayMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "synth.filterEnv.sustainPct" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "synth.filterEnv.releaseMs" => set_json_path_number(
                    &mut instrument.synth_config,
                    &["filterEnv", "releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "sample.tuneSemis" => {
                    instrument.sample_tune_semis = value.round().clamp(-24.0, 24.0) as i8
                }
                "sample.amp.gainPct" => {
                    instrument.sample_gain_pct = value.round().clamp(0.0, 100.0) as u8
                }
                "sample.amp.velocitySensitivityPct" => {
                    instrument.sample_amp_velocity_sensitivity_pct =
                        value.round().clamp(0.0, 100.0) as u8
                }
                "sample.baseVelocity" => {
                    instrument.sample_base_velocity = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.selectedSlot" => {
                    instrument.selected_sample_slot = value.round().clamp(1.0, 8.0) as usize - 1
                }
                "sample.velocityLevels.high" => {
                    instrument.sample_velocity_high = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.velocityLevels.medium" => {
                    instrument.sample_velocity_medium = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.velocityLevels.low" => {
                    instrument.sample_velocity_low = value.round().clamp(1.0, 127.0) as u8
                }
                "sample.filter.cutoffHz" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["cutoffHz"],
                    f64::from(cutoff_display_to_hz(value.round() as i32)),
                ),
                "sample.filter.resonance" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["resonance"],
                    value.round().clamp(0.0, 255.0),
                ),
                "sample.filter.envAmountPct" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["envAmountPct"],
                    value.round().clamp(-100.0, 100.0),
                ),
                "sample.filter.keyTrackingPct" => set_json_path_number(
                    &mut instrument.sample_filter,
                    &["keyTrackingPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "sample.ampEnv.attackMs" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.ampEnv.decayMs" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.ampEnv.sustainPct" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "sample.ampEnv.releaseMs" => set_json_path_number(
                    &mut instrument.sample_amp_env,
                    &["releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "sample.filterEnv.attackMs" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["attackMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.filterEnv.decayMs" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["decayMs"],
                    value.round().clamp(0.0, 5000.0),
                ),
                "sample.filterEnv.sustainPct" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["sustainPct"],
                    value.round().clamp(0.0, 100.0),
                ),
                "sample.filterEnv.releaseMs" => set_json_path_number(
                    &mut instrument.sample_filter_env,
                    &["releaseMs"],
                    value.round().clamp(0.0, 10000.0),
                ),
                "midi.channel" => instrument.midi_channel = value.round().clamp(1.0, 16.0) as u8,
                "midi.velocity" => instrument.midi_velocity = value.round().clamp(1.0, 127.0) as u8,
                "midi.durationMs" => {
                    instrument.midi_duration_ms = value.round().clamp(10.0, 5000.0) as u16
                }
                _ => return,
            }
        }
    }
    *config_dirty = true;
}
