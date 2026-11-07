use crate::protocol::RuntimeAudioCommand;

use super::super::{
    derive_instrument_name, drum_config::drum_default_config, fm_default_config,
    pluck_default_config, synth_default_config, NativeRunner, NativeSampleAvailability, Value,
};
use crate::instrument_presets::instrument_preset_config;

const SDBKIT: [&str; 8] = [
    "samples/Drum/kick/sdbkit-kick.wav",
    "samples/Drum/snare/sdbkit-snare.wav",
    "samples/Drum/hihat closed/sdbkit-hatclsd.wav",
    "samples/Drum/hihat open/sdbkit-hatopen.wav",
    "samples/Drum/toms/sdbkit-lotom.wav",
    "samples/Drum/toms/sdbkit-hitom.wav",
    "samples/Drum/claps/sdbkit-clap.wav",
    "samples/Drum/percussion/sdbkit-fmperc.wav",
];
const SYNTHKIT: [&str; 8] = [
    "samples/Drum/kick/synthkit-kick.wav",
    "samples/Drum/snare/synthkit-snare.wav",
    "samples/Drum/hihat closed/synthkit-hatclsd.wav",
    "samples/Drum/hihat open/synthkit-hatopen.wav",
    "samples/Drum/toms/synthkit-lotom.wav",
    "samples/Drum/toms/synthkit-hitom.wav",
    "samples/Drum/claps/synthkit-clap.wav",
    "samples/Drum/percussion/synthkit-8bit.wav",
];
const DISTKIT: [&str; 8] = [
    "samples/Drum/kick/distkit-kick.wav",
    "samples/Drum/snare/distkit-snare.wav",
    "samples/Drum/hihat closed/distkit-hatclsd.wav",
    "samples/Drum/hihat open/distkit-hatopen.wav",
    "samples/Drum/toms/distkit-lotom.wav",
    "samples/Drum/toms/distkit-hitom.wav",
    "samples/Drum/claps/distkit-clap.wav",
    "samples/Drum/percussion/distkit-cowbell.wav",
];

fn sample_kit(id: &str) -> Result<&'static [&'static str; 8], String> {
    let paths = match id {
        "sdbkit" => &SDBKIT,
        "synthkit" => &SYNTHKIT,
        "distkit" => &DISTKIT,
        _ => return Err(format!("unknown Sampler kit {id}")),
    };
    Ok(paths)
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
        let (type_name, config, paths) = match kind {
            "synth.preset" => ("synth", Some(factory_preset("synth", id)?), None),
            "fm.preset" => ("fm", Some(factory_preset("fm", id)?), None),
            "pluck.preset" => ("pluck", Some(factory_preset("pluck", id)?), None),
            "sample.kit" => ("sampler", None, Some(sample_kit(id)?)),
            "drum.kit" => ("drum", Some(factory_preset("drum", id)?), None),
            _ => return Err(format!("unknown factory load action {kind}")),
        };
        let instrument = &mut self.instruments[slot];
        instrument.kind = type_name.into();
        if instrument.auto_name {
            instrument.name = derive_instrument_name(slot, type_name);
        }
        match kind {
            "synth.preset" => {
                let config = config.expect("Synth preset");
                instrument.synth_gain_pct = config["amp"]["gainPct"].as_u64().unwrap_or(80) as u8;
                instrument.synth_config = config;
            }
            "fm.preset" => instrument.fm_config = config.expect("FM preset"),
            "pluck.preset" => instrument.pluck_config = config.expect("Plucked preset"),
            "drum.kit" => {
                instrument.drum_config["voices"] = config.expect("Drum kit")["voices"].take();
            }
            "sample.kit" => {
                instrument.sample_paths = paths
                    .expect("Sampler kit")
                    .iter()
                    .map(|path| Some((*path).into()))
                    .collect();
                instrument.sample_playback = Default::default();
                self.sample_availability[slot].fill(NativeSampleAvailability::Unknown);
            }
            _ => unreachable!(),
        }
        let config = self
            .instrument_audio_config(slot)
            .expect("validated instrument slot");
        self.queue_audio_command(RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: slot,
            generation: 0,
            config,
        });
        self.mark_fast_autosave_dirty();
        self.show_instrument_identity_rows(slot);
        self.show_toast(format!("Loaded I{} {type_name} {id}", slot + 1));
        Ok(())
    }
}
