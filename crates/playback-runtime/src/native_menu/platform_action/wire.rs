use realtime_engine::synth::SignalSource;

use super::{FactoryLoadKind, PlatformAction};

/// The saved form of a platform action: Aux, Shift-Aux and fader-tile bindings store it, and
/// help keys build on it, so these strings must not change.
impl PlatformAction {
    pub(crate) fn as_wire(&self) -> String {
        match self {
            Self::PresetRefresh => "preset.refresh".into(),
            Self::PresetSaveAs => "preset.saveAs".into(),
            Self::PresetSaveCurrent => "preset.saveCurrent".into(),
            Self::PresetRenameApply => "preset.renameApply".into(),
            Self::DefaultSave => "default.save".into(),
            Self::DefaultLoad => "default.load".into(),
            Self::SystemSave => "system.save".into(),
            Self::SystemLoad => "system.load".into(),
            Self::FactoryLoad => "factory.load".into(),
            Self::SystemClearAll => "system.clearAll".into(),
            Self::SystemReboot => "system.reboot".into(),
            Self::SystemShutdown => "system.shutdown".into(),
            Self::SystemControlsHelp => "system.controlsHelp".into(),
            Self::SystemInfo => "system.info".into(),
            Self::SystemHardwareTest => "system.hardwareTest".into(),
            Self::SystemConfigureWifi => "system.configureWifi".into(),
            Self::SystemBackupRestore => "system.backupRestore".into(),
            Self::SystemUpdateCheck => "system.updateCheck".into(),
            Self::SystemUpdateApply => "system.updateApply".into(),
            Self::SystemRollback => "system.rollback".into(),
            Self::MaintenanceHold => "maintenance.hold".into(),
            Self::MaintenanceRelease => "maintenance.release".into(),
            Self::UsbSdTransferStart => "usb.sdTransferStart".into(),
            Self::UsbSdTransferStop => "usb.sdTransferStop".into(),
            Self::RecordingStartAudio => "recording.startAudio".into(),
            Self::RecordingStartAudioOled => "recording.startAudioOled".into(),
            Self::RecordingStop => "recording.stop".into(),
            Self::RestartSaveChoice => "restart.saveChoice".into(),
            Self::RestartSaving => "restart.saving".into(),
            Self::RestartApp => "restart.app".into(),
            Self::MidiPanic => "midi.panic".into(),
            Self::BluetoothScan => "bluetooth.scan".into(),
            Self::PlayFxMap => "play.fx.map".into(),
            Self::FaderTileMap => "play.faders.tile.map".into(),
            Self::PresetLoad(name) => format!("preset.load:{name}"),
            Self::PresetDelete(name) => format!("preset.delete:{name}"),
            Self::PresetRenamePick(name) => format!("preset.renamePick:{name}"),
            Self::MidiOutput(id) => format!("midi.output:{}", id.as_deref().unwrap_or_default()),
            Self::MidiInput(id) => format!("midi.input:{}", id.as_deref().unwrap_or_default()),
            Self::MidiCcLearn(slot) => format!("midiCc.learn:{slot}"),
            Self::BluetoothToggle(address) => format!("bluetooth.toggle:{address}"),
            Self::BluetoothPair(address) => format!("bluetooth.pair:{address}"),
            Self::BluetoothForget(address) => format!("bluetooth.forget:{address}"),
            Self::TriggerProbabilityAssign(layer) => format!("trigger.probability.assign:{layer}"),
            Self::Signal(SignalSource::Master) => "signal.global".into(),
            Self::Signal(SignalSource::Instrument(slot)) => format!("signal.instrument.{slot}"),
            Self::Signal(SignalSource::Bus(bus)) => format!("signal.bus.{bus}"),
            Self::SampleAssign {
                instrument_slot,
                sample_slot,
            } => format!("sample.assign:{instrument_slot}:{sample_slot}"),
            Self::SampleOpen {
                instrument_slot,
                sample_slot,
                dir,
            } => format!("sample.open:{instrument_slot}:{sample_slot}:{dir}"),
            Self::SampleEnter {
                instrument_slot,
                sample_slot,
                dir,
            } => format!("sample.enter:{instrument_slot}:{sample_slot}:{dir}"),
            Self::SampleUp {
                instrument_slot,
                sample_slot,
            } => format!("sample.up:{instrument_slot}:{sample_slot}"),
            Self::SamplePick {
                instrument_slot,
                sample_slot,
                path,
            } => format!("sample.pick:{instrument_slot}:{sample_slot}:{path}"),
            Self::SampleFavoriteSet {
                instrument_slot,
                sample_slot,
            } => format!("sample.favorite.set:{instrument_slot}:{sample_slot}"),
            Self::SampleFavoriteRemove {
                instrument_slot,
                sample_slot,
            } => format!("sample.favorite.remove:{instrument_slot}:{sample_slot}"),
            Self::DrumAssign { slot, voice } => format!("drum.assign:{slot}:{voice}"),
            Self::DrumPreview { slot, voice } => format!("drum.preview:{slot}:{voice}"),
            Self::MidiDrumAssign(slot) => format!("midiDrum.assign:{slot}"),
            Self::FactoryInstrumentLoad { kind, slot, id } => {
                format!("{}:{slot}:{id}", kind.wire_name())
            }
        }
    }

    pub(crate) fn parse(wire: &str) -> Option<Self> {
        if let Some(action) = Self::parse_unit(wire) {
            return Some(action);
        }
        if let Some(source) = wire.strip_prefix("signal.") {
            return parse_signal_source(source).map(Self::Signal);
        }
        let (family, rest) = wire.split_once(':')?;
        let port = || (!rest.is_empty()).then(|| rest.to_string());
        Some(match family {
            "preset.load" => Self::PresetLoad(rest.into()),
            "preset.delete" => Self::PresetDelete(rest.into()),
            "preset.renamePick" => Self::PresetRenamePick(rest.into()),
            "midi.output" => Self::MidiOutput(port()),
            "midi.input" => Self::MidiInput(port()),
            "midiCc.learn" => Self::MidiCcLearn(rest.parse().ok()?),
            "bluetooth.toggle" => Self::BluetoothToggle(rest.into()),
            "bluetooth.pair" => Self::BluetoothPair(rest.into()),
            "bluetooth.forget" => Self::BluetoothForget(rest.into()),
            "trigger.probability.assign" => Self::TriggerProbabilityAssign(rest.parse().ok()?),
            "midiDrum.assign" => Self::MidiDrumAssign(rest.parse().ok()?),
            "drum.assign" | "drum.preview" => {
                let (slot, voice) = index_pair(rest)?;
                if family == "drum.assign" {
                    Self::DrumAssign { slot, voice }
                } else {
                    Self::DrumPreview { slot, voice }
                }
            }
            family => match FactoryLoadKind::from_wire_name(family) {
                Some(kind) => {
                    let (slot, id) = rest.split_once(':')?;
                    Self::FactoryInstrumentLoad {
                        kind,
                        slot: slot.parse().ok()?,
                        id: id.into(),
                    }
                }
                None => parse_sample_action(family, rest)?,
            },
        })
    }

    fn parse_unit(wire: &str) -> Option<Self> {
        Some(match wire {
            "preset.refresh" => Self::PresetRefresh,
            "preset.saveAs" => Self::PresetSaveAs,
            "preset.saveCurrent" => Self::PresetSaveCurrent,
            "preset.renameApply" => Self::PresetRenameApply,
            "default.save" => Self::DefaultSave,
            "default.load" => Self::DefaultLoad,
            "system.save" => Self::SystemSave,
            "system.load" => Self::SystemLoad,
            "factory.load" => Self::FactoryLoad,
            "system.clearAll" => Self::SystemClearAll,
            "system.reboot" => Self::SystemReboot,
            "system.shutdown" => Self::SystemShutdown,
            "system.controlsHelp" => Self::SystemControlsHelp,
            "system.info" => Self::SystemInfo,
            "system.hardwareTest" => Self::SystemHardwareTest,
            "system.configureWifi" => Self::SystemConfigureWifi,
            "system.backupRestore" => Self::SystemBackupRestore,
            "system.updateCheck" => Self::SystemUpdateCheck,
            "system.updateApply" => Self::SystemUpdateApply,
            "system.rollback" => Self::SystemRollback,
            "maintenance.hold" => Self::MaintenanceHold,
            "maintenance.release" => Self::MaintenanceRelease,
            "usb.sdTransferStart" => Self::UsbSdTransferStart,
            "usb.sdTransferStop" => Self::UsbSdTransferStop,
            "recording.startAudio" => Self::RecordingStartAudio,
            "recording.startAudioOled" => Self::RecordingStartAudioOled,
            "recording.stop" => Self::RecordingStop,
            "restart.saveChoice" => Self::RestartSaveChoice,
            "restart.saving" => Self::RestartSaving,
            "restart.app" => Self::RestartApp,
            "midi.panic" => Self::MidiPanic,
            "bluetooth.scan" => Self::BluetoothScan,
            "play.fx.map" => Self::PlayFxMap,
            "play.faders.tile.map" => Self::FaderTileMap,
            _ => return None,
        })
    }
}

impl FactoryLoadKind {
    pub(crate) fn wire_name(self) -> &'static str {
        match self {
            Self::SynthPreset => "synth.preset",
            Self::FmPreset => "fm.preset",
            Self::PluckPreset => "pluck.preset",
            Self::SampleKit => "sample.kit",
            Self::DrumKit => "drum.kit",
        }
    }

    fn from_wire_name(name: &str) -> Option<Self> {
        Some(match name {
            "synth.preset" => Self::SynthPreset,
            "fm.preset" => Self::FmPreset,
            "pluck.preset" => Self::PluckPreset,
            "sample.kit" => Self::SampleKit,
            "drum.kit" => Self::DrumKit,
            _ => return None,
        })
    }
}

/// `signal.instrument.N`, `signal.bus.N` or `signal.global`, without the `signal.` prefix.
fn parse_signal_source(source: &str) -> Option<SignalSource> {
    if source == "global" {
        return Some(SignalSource::Master);
    }
    let (kind, index) = source.split_once('.')?;
    let index = index.parse().ok()?;
    match kind {
        "instrument" => Some(SignalSource::Instrument(index)),
        "bus" => Some(SignalSource::Bus(index)),
        _ => None,
    }
}

/// A `sample.*` action: an instrument slot, a sample slot and, for browser rows, a folder or
/// file path that may itself contain colons.
fn parse_sample_action(family: &str, rest: &str) -> Option<PlatformAction> {
    let mut parts = rest.splitn(3, ':');
    let instrument_slot = parts.next()?.parse().ok()?;
    let sample_slot = parts.next()?.parse().ok()?;
    let path = parts.next();
    Some(match (family, path) {
        ("sample.assign", None) => PlatformAction::SampleAssign {
            instrument_slot,
            sample_slot,
        },
        ("sample.up", None) => PlatformAction::SampleUp {
            instrument_slot,
            sample_slot,
        },
        ("sample.favorite.set", None) => PlatformAction::SampleFavoriteSet {
            instrument_slot,
            sample_slot,
        },
        ("sample.favorite.remove", None) => PlatformAction::SampleFavoriteRemove {
            instrument_slot,
            sample_slot,
        },
        ("sample.open", Some(dir)) => PlatformAction::SampleOpen {
            instrument_slot,
            sample_slot,
            dir: dir.into(),
        },
        ("sample.enter", Some(dir)) => PlatformAction::SampleEnter {
            instrument_slot,
            sample_slot,
            dir: dir.into(),
        },
        ("sample.pick", Some(path)) => PlatformAction::SamplePick {
            instrument_slot,
            sample_slot,
            path: path.into(),
        },
        _ => return None,
    })
}

fn index_pair(rest: &str) -> Option<(usize, usize)> {
    let (first, second) = rest.split_once(':')?;
    Some((first.parse().ok()?, second.parse().ok()?))
}
