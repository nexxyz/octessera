use crate::delay_timing::{normalized_delay_params, strip_delay_timing_metadata};
use crate::native_menu::{delay_time_field, delay_time_row_item, NativeMenuItem};
use crate::protocol::RuntimeAudioCommand;

use super::NativeRunner;

impl NativeRunner {
    /// The delay time row its Time Mode hides (Time ms in note mode, Time Note in ms mode),
    /// built from the stored params so a binding to it still reads the stored value.
    pub(super) fn hidden_delay_time_item(&self, key: &str) -> Option<NativeMenuItem> {
        let rest = key.strip_prefix("mixer.buses.")?;
        let (bus_index, rest) = rest.split_once(".slot")?;
        let bus = self.fx_buses.get(bus_index.parse::<usize>().ok()?)?;
        let (slot, field) = rest.split_once(".params.")?;
        let (slot_type, params) = match slot {
            "1" => (&bus.slot1_type, &bus.slot1_params),
            "2" => (&bus.slot2_type, &bus.slot2_params),
            "3" => (&bus.slot3_type, &bus.slot3_params),
            _ => return None,
        };
        let bpm = self.current_menu_bpm();
        let hidden = slot_type == "delay"
            && matches!(field, "timeMs" | "timeNote")
            && delay_time_field(params, bpm) != field;
        hidden.then(|| {
            delay_time_row_item(
                &format!("mixer.buses.{bus_index}.slot{slot}.params"),
                params,
                bpm,
                field,
            )
        })
    }

    pub(super) fn retime_note_mode_bus_delays(&mut self) -> bool {
        let bpm = self.current_menu_bpm();
        let mut changed = false;
        let mut commands = Vec::new();
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
                    commands.push(RuntimeAudioCommand::SetFxBusSlot {
                        bus_index,
                        slot_index,
                        generation: 0,
                        fx_type: fx_type.clone(),
                        params: strip_delay_timing_metadata(params).into_iter().collect(),
                    });
                }
            }
        }
        for command in commands {
            self.queue_audio_command(command);
        }
        changed
    }
}
