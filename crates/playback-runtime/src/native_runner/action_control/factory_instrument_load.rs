use crate::protocol::RuntimeAudioCommand;

use super::super::{
    derive_instrument_name, drum_config::drum_default_config, fm_default_config,
    pluck_default_config, portable_patch_validation::validate_default_sample_id, NativeRunner,
    NativeSampleAvailability, Value,
};

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

fn fm_preset(id: &str) -> Result<Value, String> {
    let mut config = fm_default_config();
    match id {
        "init" => {}
        "soft_keys" => {
            config["ratio"] = "1".into();
            config["index"] = 24.into();
            config["indexEnv"]["attackMs"] = 12.into();
            config["indexEnv"]["decayMs"] = 420.into();
            config["indexEnv"]["sustainPct"] = 12.into();
            config["ampEnv"]["releaseMs"] = 650.into();
        }
        "bell" => {
            config["ratio"] = "3".into();
            config["index"] = 78.into();
            config["indexEnv"]["decayMs"] = 850.into();
            config["indexEnv"]["sustainPct"] = 0.into();
            config["ampEnv"]["decayMs"] = 900.into();
            config["ampEnv"]["sustainPct"] = 0.into();
            config["ampEnv"]["releaseMs"] = 700.into();
        }
        _ => return Err(format!("unknown FM preset {id}")),
    }
    Ok(config)
}

fn pluck_preset(id: &str) -> Result<Value, String> {
    let mut config = pluck_default_config();
    match id {
        "init" => {}
        "nylon" => {
            config["decayMs"] = 2400.into();
            config["brightnessPct"] = 35.into();
            config["pickPositionPct"] = 32.into();
        }
        "steel" => {
            config["decayMs"] = 3200.into();
            config["brightnessPct"] = 88.into();
            config["pickPositionPct"] = 17.into();
        }
        "muted" => {
            config["decayMs"] = 350.into();
            config["brightnessPct"] = 22.into();
            config["pickPositionPct"] = 38.into();
        }
        _ => return Err(format!("unknown Plucked preset {id}")),
    }
    Ok(config)
}

fn sample_kit(id: &str) -> Result<&'static [&'static str; 8], String> {
    let paths = match id {
        "sdbkit" => &SDBKIT,
        "synthkit" => &SYNTHKIT,
        "distkit" => &DISTKIT,
        _ => return Err(format!("unknown Sampler kit {id}")),
    };
    for path in paths {
        validate_default_sample_id(path, path)?;
    }
    Ok(paths)
}

fn drum_kit(id: &str) -> Result<Value, String> {
    let mut voices = drum_default_config()["voices"].clone();
    let (decay, tone, tune) = match id {
        "default" => return Ok(voices),
        "tight" => (
            [260, 140, 50, 260, 300, 190, 135, 65],
            [50, 82, 95, 90, 65, 75, 90, 90],
            [0; 8],
        ),
        "heavy" => (
            [750, 360, 110, 850, 880, 590, 320, 150],
            [25, 55, 75, 72, 40, 48, 65, 60],
            [-4, 0, 0, 0, -3, -2, 0, 0],
        ),
        _ => return Err(format!("unknown Drum kit {id}")),
    };
    for (index, voice) in voices
        .as_array_mut()
        .expect("default Drum voices")
        .iter_mut()
        .enumerate()
    {
        voice["decayMs"] = decay[index].into();
        voice["tonePct"] = tone[index].into();
        voice["tuneSemis"] = tune[index].into();
    }
    Ok(voices)
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
            "fm.preset" => ("fm", Some(fm_preset(id)?), None),
            "pluck.preset" => ("pluck", Some(pluck_preset(id)?), None),
            "sample.kit" => ("sampler", None, Some(sample_kit(id)?)),
            "drum.kit" => ("drum", Some(drum_kit(id)?), None),
            _ => return Err(format!("unknown factory load action {kind}")),
        };
        let instrument = &mut self.instruments[slot];
        instrument.kind = type_name.into();
        if instrument.auto_name {
            instrument.name = derive_instrument_name(slot, type_name);
        }
        match kind {
            "fm.preset" => instrument.fm_config = config.expect("FM preset"),
            "pluck.preset" => instrument.pluck_config = config.expect("Plucked preset"),
            "drum.kit" => instrument.drum_config["voices"] = config.expect("Drum voices"),
            "sample.kit" => {
                instrument.sample_paths = paths
                    .expect("Sampler kit")
                    .iter()
                    .map(|path| Some((*path).into()))
                    .collect();
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
        self.menu.rebuild(self.menu_config());
        self.show_toast(format!("Loaded I{} {type_name} {id}", slot + 1));
        Ok(())
    }
}
