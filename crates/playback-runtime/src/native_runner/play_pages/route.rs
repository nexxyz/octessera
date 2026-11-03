use serde_json::Value;

use super::super::led_color::LedColor;
use super::super::menu_apply_fast_fx_bus::audio_params_for_fx;
use super::super::{
    NativeFxBus, NativeRunner, BUS_COUNT, GLOBAL_FX_SLOT_COUNT, GRID_WIDTH, INSTRUMENT_COUNT,
};
use crate::protocol::RuntimeAudioCommand;

const ROUTES: [&str; 5] = ["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"];
const BYPASS_ROW: usize = 5;
const DUCK_KEY_ROW: usize = 7;
const BUS_SLOT_COUNT: usize = 3;

/// Live-only FX overrides from the Route page; the saved config never changes.
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::native_runner) struct LiveFxOverrides {
    bus_bypassed: [bool; BUS_COUNT],
    global_bypassed: [bool; GLOBAL_FX_SLOT_COUNT],
    duck_key: Option<usize>,
}

impl LiveFxOverrides {
    fn is_empty(&self) -> bool {
        self.duck_key.is_none()
            && !self.bus_bypassed.contains(&true)
            && !self.global_bypassed.contains(&true)
    }
}

impl NativeRunner {
    pub(in crate::native_runner) fn handle_play_route_press(&mut self, x: usize, y: usize) {
        let x = x.min(GRID_WIDTH - 1);
        match y {
            0..=4 => self.set_route_from_grid(x, ROUTES[y]),
            BYPASS_ROW if x < BUS_COUNT => self.toggle_bus_fx_bypass(x),
            BYPASS_ROW if x < BUS_COUNT + GLOBAL_FX_SLOT_COUNT => {
                self.toggle_global_fx_bypass(x - BUS_COUNT);
            }
            DUCK_KEY_ROW if self.instrument_has_audio(x) => self.toggle_duck_key(x),
            _ => {}
        }
    }

    pub(in crate::native_runner) fn apply_play_route_overlay(&self, leds: &mut [LedColor]) {
        self.dim_leds(leds, 4);
        for x in 0..INSTRUMENT_COUNT.min(GRID_WIDTH) {
            let audio = self.instrument_has_audio(x);
            let route = self.instruments.get(x).map(|slot| slot.route.as_str());
            for (y, candidate) in ROUTES.iter().enumerate() {
                let color = match (audio, route == Some(*candidate)) {
                    (false, _) => LedColor::BLACK,
                    (true, true) => route_color(y),
                    (true, false) => route_color(y).dim(5),
                };
                self.set_display_led(leds, x, y, color);
            }
            let duck = match (audio, self.play_live.fx_overrides.duck_key == Some(x)) {
                (false, _) => LedColor::BLACK,
                (true, true) => LedColor::RED,
                (true, false) => LedColor::RED.dim(6),
            };
            self.set_display_led(leds, x, DUCK_KEY_ROW, duck);
            self.set_display_led(leds, x, DUCK_KEY_ROW - 1, LedColor::BLACK);
        }
        let overrides = &self.play_live.fx_overrides;
        for bus in 0..BUS_COUNT {
            let color = route_color(bus + 1);
            let lit = !overrides.bus_bypassed[bus];
            self.set_display_led(
                leds,
                bus,
                BYPASS_ROW,
                if lit { color } else { color.dim(6) },
            );
        }
        for slot in 0..GLOBAL_FX_SLOT_COUNT {
            let lit = !overrides.global_bypassed[slot];
            let color = if lit {
                LedColor::WHITE
            } else {
                LedColor::WHITE.dim(6)
            };
            self.set_display_led(leds, BUS_COUNT + slot, BYPASS_ROW, color);
        }
        for x in BUS_COUNT + GLOBAL_FX_SLOT_COUNT..GRID_WIDTH {
            self.set_display_led(leds, x, BYPASS_ROW, LedColor::BLACK);
        }
    }

    /// The one place the Route page's live overrides rewrite what the engine receives.
    pub(in crate::native_runner) fn live_fx_audio_command(
        &self,
        mut command: RuntimeAudioCommand,
    ) -> RuntimeAudioCommand {
        let overrides = &self.play_live.fx_overrides;
        if overrides.is_empty() {
            return command;
        }
        match &mut command {
            RuntimeAudioCommand::SetFxBusSlot {
                bus_index,
                fx_type,
                params,
                ..
            } => {
                if overrides.bus_bypassed.get(*bus_index) == Some(&true) {
                    *fx_type = "none".into();
                    params.clear();
                } else if let (Some(key), "duck") = (overrides.duck_key, fx_type.as_str()) {
                    params.insert("source".into(), Value::from(duck_source(key)));
                }
            }
            RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                fx_type,
                params,
                ..
            } if overrides.global_bypassed.get(*slot_index) == Some(&true) => {
                *fx_type = "none".into();
                params.clear();
            }
            RuntimeAudioCommand::SetAudioConfig { config, .. } => {
                self.override_mixer_payload(config);
            }
            _ => {}
        }
        command
    }

    pub(in crate::native_runner) fn take_live_fx_overrides(&mut self) -> bool {
        let had = !self.play_live.fx_overrides.is_empty();
        self.play_live.fx_overrides = LiveFxOverrides::default();
        had
    }

    /// Sends every bus and global FX slot again, through the override gate.
    pub(in crate::native_runner) fn resend_fx_slots(&mut self) {
        let mut commands = Vec::new();
        for (bus_index, bus) in self.fx_buses.iter().enumerate() {
            for slot_index in 0..BUS_SLOT_COUNT {
                let (fx_type, params) = bus_slot(bus, slot_index);
                commands.push(RuntimeAudioCommand::SetFxBusSlot {
                    bus_index,
                    slot_index,
                    generation: 0,
                    fx_type: fx_type.into(),
                    params: audio_params_for_fx(fx_type, params),
                });
            }
        }
        for (slot_index, fx_type) in self.global_fx_slots.iter().enumerate() {
            let params = self
                .global_fx_params
                .get(slot_index)
                .unwrap_or(&Value::Null);
            commands.push(RuntimeAudioCommand::SetGlobalFxSlot {
                slot_index,
                generation: 0,
                fx_type: fx_type.clone(),
                params: audio_params_for_fx(fx_type, params),
            });
        }
        for command in commands {
            self.queue_audio_command(command);
        }
        self.invalidate_lfo_audio_cache();
        if let Err(error) = self.process_modulation_step(true) {
            self.show_toast(format!("LFO composition unavailable: {error}"));
        }
    }

    fn set_route_from_grid(&mut self, index: usize, route: &str) {
        if !self.instrument_has_audio(index) {
            return;
        }
        if self.set_instrument_route(index, route) {
            self.menu
                .set_enum_value_for_key(&format!("instruments.{index}.mixer.route"), route);
            self.show_instruments_rows(&[index]);
        }
        let target = route
            .strip_prefix("fx_bus_")
            .map_or_else(|| route.to_string(), |bus| format!("B{bus}"));
        self.show_toast(format!("I{} {target}", index + 1));
    }

    fn toggle_bus_fx_bypass(&mut self, bus: usize) {
        let bypassed = &mut self.play_live.fx_overrides.bus_bypassed[bus];
        *bypassed = !*bypassed;
        let state = if *bypassed { "bypassed" } else { "on" };
        self.resend_fx_slots();
        self.show_toast(format!("B{} FX {state}", bus + 1));
    }

    fn toggle_global_fx_bypass(&mut self, slot: usize) {
        let bypassed = &mut self.play_live.fx_overrides.global_bypassed[slot];
        *bypassed = !*bypassed;
        let state = if *bypassed { "bypassed" } else { "on" };
        self.resend_fx_slots();
        self.show_toast(format!("Global FX {} {state}", slot + 1));
    }

    fn toggle_duck_key(&mut self, instrument: usize) {
        let duck_key = &mut self.play_live.fx_overrides.duck_key;
        *duck_key = (*duck_key != Some(instrument)).then_some(instrument);
        let toast = match *duck_key {
            Some(key) => format!("Duck key: {}", duck_source(key)),
            None => "Duck key off".into(),
        };
        self.resend_fx_slots();
        self.show_toast(toast);
    }

    fn instrument_has_audio(&self, index: usize) -> bool {
        self.instruments
            .get(index)
            .is_some_and(|slot| !matches!(slot.kind.as_str(), "none" | "midi"))
    }

    fn override_mixer_payload(&self, config: &mut Value) {
        let overrides = &self.play_live.fx_overrides;
        let Some(mixer) = config.get_mut("mixer") else {
            return;
        };
        if let Some(buses) = mixer.get_mut("buses").and_then(Value::as_array_mut) {
            for (index, bus) in buses.iter_mut().enumerate() {
                for slot_index in 1..=BUS_SLOT_COUNT {
                    let Some(slot) = bus.get_mut(format!("slot{slot_index}")) else {
                        continue;
                    };
                    if overrides.bus_bypassed.get(index) == Some(&true) {
                        *slot = serde_json::json!({ "type": "none", "params": {} });
                    } else if let Some(key) = overrides.duck_key {
                        if slot.get("type").and_then(Value::as_str) == Some("duck") {
                            if let Some(params) =
                                slot.get_mut("params").and_then(Value::as_object_mut)
                            {
                                params.insert("source".into(), Value::from(duck_source(key)));
                            }
                        }
                    }
                }
            }
        }
        if let Some(slots) = mixer
            .get_mut("master")
            .and_then(|master| master.get_mut("slots"))
            .and_then(Value::as_array_mut)
        {
            for (index, slot) in slots.iter_mut().enumerate() {
                if overrides.global_bypassed.get(index) == Some(&true) {
                    *slot = serde_json::json!({ "type": "none", "params": {} });
                }
            }
        }
    }
}

fn route_color(row: usize) -> LedColor {
    match row {
        1 => LedColor::MAGENTA,
        2 => LedColor::BLUE,
        3 => LedColor::GREEN,
        4 => LedColor::YELLOW,
        _ => LedColor::WHITE,
    }
}

fn duck_source(instrument: usize) -> String {
    format!("I{}", instrument + 1)
}

fn bus_slot(bus: &NativeFxBus, slot: usize) -> (&str, &Value) {
    match slot {
        0 => (&bus.slot1_type, &bus.slot1_params),
        1 => (&bus.slot2_type, &bus.slot2_params),
        _ => (&bus.slot3_type, &bus.slot3_params),
    }
}
