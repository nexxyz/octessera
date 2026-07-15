use crate::delay_timing::{normalized_delay_params, strip_delay_timing_metadata};
use crate::protocol::RuntimeAudioCommand;

use super::NativeRunner;

impl NativeRunner {
    pub(super) fn retime_note_mode_bus_delays(&mut self) -> bool {
        let bpm = self.current_menu_bpm();
        let mut changed = false;
        let mut commands = Vec::new();
        let mut menu_updates = Vec::new();
        for (bus_index, bus) in self.fx_buses.iter_mut().enumerate() {
            for slot_index in 0..3 {
                let (fx_type, params) = match slot_index {
                    0 => (&bus.slot1_type, &mut bus.slot1_params),
                    1 => (&bus.slot2_type, &mut bus.slot2_params),
                    2 => (&bus.slot3_type, &mut bus.slot3_params),
                    _ => unreachable!(),
                };
                if fx_type != "delay" {
                    continue;
                }
                let normalized = normalized_delay_params(params, bpm);
                if normalized
                    .get("timeMode")
                    .and_then(serde_json::Value::as_str)
                    != Some("note")
                {
                    *params = normalized;
                    continue;
                }
                let before = params.get("timeMs").cloned();
                *params = normalized;
                if params.get("timeMs") != before.as_ref() {
                    changed = true;
                    if let Some(time_ms) = params.get("timeMs").and_then(serde_json::Value::as_i64)
                    {
                        menu_updates.push((bus_index, slot_index, time_ms as i32));
                    }
                    commands.push(RuntimeAudioCommand::SetFxBusSlot {
                        bus_index,
                        slot_index,
                        fx_type: fx_type.clone(),
                        params: strip_delay_timing_metadata(params).into_iter().collect(),
                    });
                }
            }
        }
        for command in commands {
            self.queue_audio_command(command);
        }
        for (bus_index, slot_index, time_ms) in menu_updates {
            let key = format!(
                "mixer.buses.{bus_index}.slot{}.params.timeMs",
                slot_index + 1
            );
            self.menu.set_number_value_for_key(&key, time_ms);
        }
        changed
    }
}
