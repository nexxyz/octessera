use super::super::{
    derive_instrument_name, drum_config::drum_default_config, fm_default_config,
    pluck_default_config, synth_default_config, NativeRunner, NativeSampleAvailability, Value,
};
use crate::instrument_presets::instrument_preset_config;
use crate::native_menu::SAMPLER_KITS;

/// What a factory load puts into a slot: a family's preset config or a Sampler kit's paths.
enum FactoryLoad {
    Preset(&'static str, Value),
    SampleKit(&'static [&'static str; 8]),
}

fn factory_load(kind: &str, id: &str) -> Result<FactoryLoad, String> {
    let family = match kind {
        "synth.preset" => "synth",
        "fm.preset" => "fm",
        "pluck.preset" => "pluck",
        "drum.kit" => "drum",
        "sample.kit" => {
            return SAMPLER_KITS
                .iter()
                .find(|(kit, ..)| *kit == id)
                .map(|(_, _, paths)| FactoryLoad::SampleKit(paths))
                .ok_or_else(|| format!("unknown Sampler kit {id}"));
        }
        _ => return Err(format!("unknown factory load action {kind}")),
    };
    Ok(FactoryLoad::Preset(family, factory_preset(family, id)?))
}

/// A factory preset's config for `family`: its overrides on that family's default.
fn factory_preset(family: &str, id: &str) -> Result<Value, String> {
    let defaults = match family {
        "synth" => synth_default_config(),
        "fm" => fm_default_config(),
        "pluck" => pluck_default_config(),
        _ => drum_default_config(),
    };
    instrument_preset_config(family, id, defaults)
        .ok_or_else(|| format!("unknown {family} preset {id}"))
}

impl NativeRunner {
    pub(super) fn load_factory_instrument_action(&mut self, action: &str) -> Result<(), String> {
        let (kind, rest) = action
            .split_once(':')
            .ok_or("invalid factory load action")?;
        let (slot, id) = rest.split_once(':').ok_or("invalid factory load action")?;
        let slot = slot
            .parse::<usize>()
            .map_err(|_| "invalid factory instrument slot")?;
        if self.instruments.get(slot).is_none() {
            return Err(format!("invalid factory instrument slot {slot}"));
        }
        let load = factory_load(kind, id)?;
        let type_name = match &load {
            FactoryLoad::Preset(family, _) => *family,
            FactoryLoad::SampleKit(_) => "sampler",
        };
        let instrument = &mut self.instruments[slot];
        instrument.kind = type_name.into();
        if instrument.auto_name {
            instrument.name = derive_instrument_name(slot, type_name);
        }
        match load {
            FactoryLoad::Preset("synth", config) => {
                instrument.synth_gain_pct = config["amp"]["gainPct"].as_u64().unwrap_or(80) as u8;
                instrument.synth_config = config;
            }
            FactoryLoad::Preset("fm", config) => instrument.fm_config = config,
            FactoryLoad::Preset("pluck", config) => instrument.pluck_config = config,
            FactoryLoad::Preset(_, mut config) => {
                instrument.drum_config["voices"] = config["voices"].take();
            }
            FactoryLoad::SampleKit(paths) => {
                instrument.sample_paths = paths.iter().map(|path| Some((*path).into())).collect();
                instrument.sample_playback = Default::default();
                self.sample_availability[slot].fill(NativeSampleAvailability::Unknown);
            }
        }
        self.queue_instrument_slot_config(slot);
        self.mark_fast_autosave_dirty();
        self.show_instrument_identity_rows(slot);
        self.show_toast(format!("Loaded I{} {type_name} {id}", slot + 1));
        Ok(())
    }
}
