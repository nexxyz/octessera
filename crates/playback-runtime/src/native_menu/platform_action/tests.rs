use super::{FactoryLoadKind, PlatformAction};
use realtime_engine::synth::SignalSource;
use std::collections::HashSet;
use std::mem::discriminant;

fn one_of_each() -> Vec<PlatformAction> {
    use PlatformAction::*;
    let (instrument_slot, sample_slot) = (2, 5);
    vec![
        PresetRefresh,
        PresetSaveAs,
        PresetSaveCurrent,
        PresetRenameApply,
        PresetLoad("Night: Bus".into()),
        PresetDelete("old".into()),
        PresetRenamePick("draft".into()),
        DefaultSave,
        DefaultLoad,
        SystemSave,
        SystemLoad,
        FactoryLoad,
        SystemClearAll,
        SystemReboot,
        SystemShutdown,
        SystemControlsHelp,
        SystemInfo,
        SystemHardwareTest,
        SystemConfigureWifi,
        SystemBackupRestore,
        SystemUpdateCheck,
        SystemUpdateApply,
        SystemRollback,
        MaintenanceHold,
        MaintenanceRelease,
        UsbSdTransferStart,
        UsbSdTransferStop,
        RecordingStartAudio,
        RecordingStartAudioOled,
        RecordingStop,
        RestartSaveChoice,
        RestartSaving,
        RestartApp,
        MidiPanic,
        MidiOutput(None),
        MidiOutput(Some("usb:1".into())),
        MidiInput(None),
        MidiInput(Some("din".into())),
        MidiCcLearn(3),
        BluetoothScan,
        BluetoothToggle("AA:BB:CC:DD:EE:FF".into()),
        BluetoothPair("AA:BB:CC:DD:EE:01".into()),
        BluetoothForget("AA:BB:CC:DD:EE:02".into()),
        PlayFxMap,
        FaderTileMap,
        TriggerProbabilityAssign(7),
        Signal(SignalSource::Master),
        Signal(SignalSource::Instrument(4)),
        Signal(SignalSource::Bus(1)),
        SampleAssign {
            instrument_slot,
            sample_slot,
        },
        SampleOpen {
            instrument_slot,
            sample_slot,
            dir: String::new(),
        },
        SampleEnter {
            instrument_slot,
            sample_slot,
            dir: "samples/Drum".into(),
        },
        SampleUp {
            instrument_slot,
            sample_slot,
        },
        SamplePick {
            instrument_slot,
            sample_slot,
            path: "samples/odd:name.wav".into(),
        },
        SampleFavoriteSet {
            instrument_slot,
            sample_slot,
        },
        SampleFavoriteRemove {
            instrument_slot,
            sample_slot,
        },
        DrumAssign { slot: 1, voice: 6 },
        DrumPreview { slot: 1, voice: 6 },
        MidiDrumAssign(3),
        FactoryInstrumentLoad {
            kind: FactoryLoadKind::SynthPreset,
            slot: 0,
            id: "acid_bass".into(),
        },
        FactoryInstrumentLoad {
            kind: FactoryLoadKind::FmPreset,
            slot: 1,
            id: "bell".into(),
        },
        FactoryInstrumentLoad {
            kind: FactoryLoadKind::PluckPreset,
            slot: 2,
            id: "harp".into(),
        },
        FactoryInstrumentLoad {
            kind: FactoryLoadKind::SampleKit,
            slot: 3,
            id: "drums".into(),
        },
        FactoryInstrumentLoad {
            kind: FactoryLoadKind::DrumKit,
            slot: 4,
            id: "boom".into(),
        },
    ]
}

/// Fails to compile when a variant is added, so `one_of_each` gains an example of it.
fn variant_count_guard(action: &PlatformAction) -> usize {
    use PlatformAction::*;
    match action {
        PresetRefresh
        | PresetSaveAs
        | PresetSaveCurrent
        | PresetRenameApply
        | PresetLoad(_)
        | PresetDelete(_)
        | PresetRenamePick(_)
        | DefaultSave
        | DefaultLoad
        | SystemSave
        | SystemLoad
        | FactoryLoad
        | SystemClearAll
        | SystemReboot
        | SystemShutdown
        | SystemControlsHelp
        | SystemInfo
        | SystemHardwareTest
        | SystemConfigureWifi
        | SystemBackupRestore
        | SystemUpdateCheck
        | SystemUpdateApply
        | SystemRollback
        | MaintenanceHold
        | MaintenanceRelease
        | UsbSdTransferStart
        | UsbSdTransferStop
        | RecordingStartAudio
        | RecordingStartAudioOled
        | RecordingStop
        | RestartSaveChoice
        | RestartSaving
        | RestartApp
        | MidiPanic
        | MidiOutput(_)
        | MidiInput(_)
        | MidiCcLearn(_)
        | BluetoothScan
        | BluetoothToggle(_)
        | BluetoothPair(_)
        | BluetoothForget(_)
        | PlayFxMap
        | FaderTileMap
        | TriggerProbabilityAssign(_)
        | Signal(_)
        | SampleAssign { .. }
        | SampleOpen { .. }
        | SampleEnter { .. }
        | SampleUp { .. }
        | SamplePick { .. }
        | SampleFavoriteSet { .. }
        | SampleFavoriteRemove { .. }
        | DrumAssign { .. }
        | DrumPreview { .. }
        | MidiDrumAssign(_)
        | FactoryInstrumentLoad { .. } => 56,
    }
}

#[test]
fn every_platform_action_round_trips_through_its_wire_form() {
    let actions = one_of_each();
    let variants = actions.iter().map(discriminant).collect::<HashSet<_>>();
    assert_eq!(variants.len(), variant_count_guard(&actions[0]));
    for action in actions {
        let wire = action.as_wire();
        assert_eq!(PlatformAction::parse(&wire), Some(action), "{wire}");
    }
}

#[test]
fn saved_wire_strings_keep_their_exact_bytes() {
    for wire in [
        "default.save",
        "play.fx.map",
        "play.faders.tile.map",
        "midi.panic",
        "midi.output:",
        "midi.input:usb:1",
        "midiCc.learn:2",
        "preset.load:My: Patch",
        "sample.assign:0:0",
        "sample.open:1:2:",
        "sample.pick:1:2:samples/Drum/kick.wav",
        "drum.assign:3:7",
        "trigger.probability.assign:4",
        "signal.instrument.0",
        "signal.global",
        "bluetooth.toggle:AA:BB:CC:DD:EE:FF",
        "drum.kit:0:boom",
        "sample.kit:5:drums",
    ] {
        let action = PlatformAction::parse(wire).unwrap_or_else(|| panic!("{wire}"));
        assert_eq!(action.as_wire(), wire);
    }
}

#[test]
fn unknown_or_malformed_actions_do_not_parse() {
    for wire in [
        "",
        "store.refresh",
        "sample.preview:0:0:kick.wav",
        "midi.out:",
        "system.restartApp",
        "sample.assign:0",
        "sample.assign:0:0:extra",
        "sample.pick:0:0",
        "drum.assign:x:1",
        "midiCc.learn:",
        "signal.layer.1",
        "organ.preset:0:x",
        "synth.preset:0",
    ] {
        assert_eq!(PlatformAction::parse(wire), None, "{wire}");
    }
}
