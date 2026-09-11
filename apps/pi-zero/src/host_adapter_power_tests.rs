use super::*;
use crate::power_lifecycle::{
    PowerAction, PowerLifecycle, PowerLifecycleCallbacks, PowerLifecyclePhase, PowerLifecycleResult,
};
use std::path::{Path, PathBuf};

struct PowerTestCallbacks<'a> {
    adapter: &'a mut PiPlaybackHostAdapter,
    events: Vec<&'static str>,
    recording_active_at_submit: Option<bool>,
}

impl PowerLifecycleCallbacks for PowerTestCallbacks<'_> {
    fn save_recovery(&mut self) -> Result<(), String> {
        self.events.push("save");
        self.adapter.save_recovery_for_power()
    }

    fn panic_external_midi(&mut self) -> Result<(), String> {
        self.events.push("midi-panic");
        Ok(())
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        self.events.push("internal-silence");
        Ok(())
    }

    fn acknowledge_terminal(&mut self, _action: PowerAction) -> Result<(), String> {
        self.events.push("terminal-ack");
        Ok(())
    }

    fn submit_power(&mut self, _action: PowerAction) -> Result<(), String> {
        self.events.push("power-submit");
        self.recording_active_at_submit = self
            .adapter
            .audio_service()
            .map(|audio| audio.is_recording())
            .transpose()?;
        Ok(())
    }
}

#[test]
fn raspberry_power_finalizes_recording_before_power_submission() {
    let root = test_root("finalize");
    let recordings = root.join("recordings");
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(recordings);
    let mut adapter = test_adapter(audio.clone(), &root);
    audio.start_recording(1).unwrap();
    adapter.recovery_save_status = Some(Ok(()));

    let mut callbacks = PowerTestCallbacks {
        adapter: &mut adapter,
        events: Vec::new(),
        recording_active_at_submit: None,
    };
    assert_eq!(
        PowerLifecycle::default().execute(PowerAction::Reboot, &mut callbacks),
        PowerLifecycleResult::Submitted
    );
    assert_eq!(
        callbacks.events,
        vec![
            "save",
            "midi-panic",
            "internal-silence",
            "terminal-ack",
            "power-submit"
        ]
    );
    assert_eq!(callbacks.recording_active_at_submit, Some(false));
    remove_root(root);
}

#[test]
fn raspberry_power_combines_recovery_and_recording_failures_without_submission() {
    let root = test_root("failure");
    let recordings = root.join("recordings");
    let (audio, _, _, _) = crate::audio::test_service_with_recording_dir(recordings.clone());
    let mut adapter = test_adapter(audio.clone(), &root);
    audio.start_recording(1).unwrap();
    let partial = std::fs::read_dir(&recordings)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.to_string_lossy().ends_with(".partial.wav"))
        .unwrap();
    let partial_name = partial.file_name().unwrap().to_str().unwrap();
    let recording_name = partial_name.strip_suffix(".partial.wav").unwrap();
    std::fs::create_dir(partial.with_file_name(format!("{recording_name}.wav"))).unwrap();
    adapter.recovery_save_status = Some(Err("recovery failed".into()));

    let mut callbacks = PowerTestCallbacks {
        adapter: &mut adapter,
        events: Vec::new(),
        recording_active_at_submit: None,
    };
    let PowerLifecycleResult::Failed(failure) =
        PowerLifecycle::default().execute(PowerAction::Shutdown, &mut callbacks)
    else {
        panic!("combined recovery and recording failure should block power");
    };
    assert_eq!(failure.phase, PowerLifecyclePhase::RecoverySave);
    assert!(!failure.accepted);
    assert!(failure.message.contains("recovery failed"));
    assert!(failure.message.contains("recording stop failed"));
    assert_eq!(callbacks.events, vec!["save"]);
    assert!(callbacks.recording_active_at_submit.is_none());
    remove_root(root);
}

fn test_adapter(audio: crate::audio::AudioService, root: &Path) -> PiPlaybackHostAdapter {
    PiPlaybackHostAdapter::new(
        Some(audio),
        root.join("store"),
        root.join("samples"),
        Arc::new(|_| {}),
        false,
        crate::usb_config::UsbAudioOut::Jack,
    )
}

fn test_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "octessera-pi-power-{name}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn remove_root(root: PathBuf) {
    let _ = std::fs::remove_dir_all(root);
}
