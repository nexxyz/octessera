use realtime_engine::synth::SignalSource;

use super::section_labels::{LINK_LABEL, PLAY_LABEL, SHAPE_LABEL, SYSTEM_LABEL};

mod wire;

#[cfg(test)]
mod tests;

/// A menu action the runner performs itself or hands to the host as a platform effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformAction {
    PresetRefresh,
    PresetSaveAs,
    PresetSaveCurrent,
    PresetRenameApply,
    PresetLoad(String),
    PresetDelete(String),
    PresetRenamePick(String),
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
    /// The Apply System dialog's save choice.
    RestartSaveChoice,
    /// The dialog shown while a restart save runs.
    RestartSaving,
    /// The Restart? dialog after a restart save.
    RestartApp,
    MidiPanic,
    /// `None` disconnects the port.
    MidiOutput(Option<String>),
    MidiInput(Option<String>),
    MidiCcLearn(usize),
    BluetoothScan,
    BluetoothToggle(String),
    BluetoothPair(String),
    BluetoothForget(String),
    PlayFxMap,
    FaderTileMap,
    TriggerProbabilityAssign(usize),
    Signal(SignalSource),
    SampleAssign {
        instrument_slot: usize,
        sample_slot: usize,
    },
    SampleOpen {
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    },
    SampleEnter {
        instrument_slot: usize,
        sample_slot: usize,
        dir: String,
    },
    SampleUp {
        instrument_slot: usize,
        sample_slot: usize,
    },
    SamplePick {
        instrument_slot: usize,
        sample_slot: usize,
        path: String,
    },
    SampleFavoriteSet {
        instrument_slot: usize,
        sample_slot: usize,
    },
    SampleFavoriteRemove {
        instrument_slot: usize,
        sample_slot: usize,
    },
    DrumAssign {
        slot: usize,
        voice: usize,
    },
    DrumPreview {
        slot: usize,
        voice: usize,
    },
    MidiDrumAssign(usize),
    FactoryInstrumentLoad {
        kind: FactoryLoadKind,
        slot: usize,
        id: String,
    },
}

/// The factory preset or kit families an instrument slot can load.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactoryLoadKind {
    SynthPreset,
    FmPreset,
    PluckPreset,
    SampleKit,
    DrumKit,
}

impl PlatformAction {
    /// The menu section that owns the action, so a hint for it takes that section's colour.
    pub(crate) fn home_section(&self) -> &'static str {
        match self {
            Self::MidiCcLearn(_) => LINK_LABEL,
            Self::PlayFxMap | Self::FaderTileMap => PLAY_LABEL,
            Self::Signal(_)
            | Self::SampleAssign { .. }
            | Self::SampleOpen { .. }
            | Self::SampleEnter { .. }
            | Self::SampleUp { .. }
            | Self::SamplePick { .. }
            | Self::SampleFavoriteSet { .. }
            | Self::SampleFavoriteRemove { .. }
            | Self::DrumAssign { .. }
            | Self::DrumPreview { .. }
            | Self::MidiDrumAssign(_)
            | Self::FactoryInstrumentLoad { .. } => SHAPE_LABEL,
            Self::PresetRefresh
            | Self::PresetSaveAs
            | Self::PresetSaveCurrent
            | Self::PresetRenameApply
            | Self::PresetLoad(_)
            | Self::PresetDelete(_)
            | Self::PresetRenamePick(_)
            | Self::DefaultSave
            | Self::DefaultLoad
            | Self::SystemSave
            | Self::SystemLoad
            | Self::FactoryLoad
            | Self::SystemClearAll
            | Self::SystemReboot
            | Self::SystemShutdown
            | Self::SystemControlsHelp
            | Self::SystemInfo
            | Self::SystemHardwareTest
            | Self::SystemConfigureWifi
            | Self::SystemBackupRestore
            | Self::SystemUpdateCheck
            | Self::SystemUpdateApply
            | Self::SystemRollback
            | Self::MaintenanceHold
            | Self::MaintenanceRelease
            | Self::UsbSdTransferStart
            | Self::UsbSdTransferStop
            | Self::RecordingStartAudio
            | Self::RecordingStartAudioOled
            | Self::RecordingStop
            | Self::RestartSaveChoice
            | Self::RestartSaving
            | Self::RestartApp
            | Self::MidiPanic
            | Self::MidiOutput(_)
            | Self::MidiInput(_)
            | Self::BluetoothScan
            | Self::BluetoothToggle(_)
            | Self::BluetoothPair(_)
            | Self::BluetoothForget(_)
            | Self::TriggerProbabilityAssign(_) => SYSTEM_LABEL,
        }
    }

    /// Whether the action writes the patch or a preset, which waits while autosave is deferred.
    pub(crate) fn is_manual_save(&self) -> bool {
        matches!(
            self,
            Self::DefaultSave
                | Self::PresetSaveAs
                | Self::PresetRenameApply
                | Self::PresetSaveCurrent
        )
    }
}
