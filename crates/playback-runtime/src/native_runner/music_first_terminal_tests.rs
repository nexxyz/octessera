use super::super::music_first_tests::{assert_music_only, playing_default};
use crate::{HostMessage, RunnerMessage, RuntimePlatformEffect};
use serde_json::json;

#[test]
fn playing_terminal_confirmation_publishes_shutdown_or_reboot_before_effect() {
    for label in ["Shutdown", "Reboot"] {
        let mut runner = playing_default();
        for _ in 0..4 {
            runner
                .send_music_first(HostMessage::DeviceInput {
                    input: json!({"type":"encoder_turn","id":"main","delta":1}),
                    request_snapshot: Some(false),
                })
                .unwrap();
        }
        assert_eq!(runner.menu.current_label(), Some("System"));
        let system = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_press","id":"main"}),
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_music_only(&system);
        for _ in 0..30 {
            if runner.menu.current_label() == Some(label) {
                break;
            }
            runner
                .send_music_first(HostMessage::DeviceInput {
                    input: json!({"type":"encoder_turn","id":"main","delta":1}),
                    request_snapshot: Some(false),
                })
                .unwrap();
        }
        assert_eq!(runner.menu.current_label(), Some(label));
        let opened = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_press","id":"main"}),
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_music_only(&opened);
        assert_eq!(runner.display.confirm_dialog.as_ref().unwrap().cursor, 0);
        let cancelled = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_press","id":"main"}),
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_music_only(&cancelled);
        assert!(runner.display.confirm_dialog.is_none());
        let reopened = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_press","id":"main"}),
                request_snapshot: Some(false),
            })
            .unwrap();
        assert_music_only(&reopened);
        runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_turn","id":"main","delta":1}),
                request_snapshot: Some(false),
            })
            .unwrap();
        let terminal = runner
            .send_music_first(HostMessage::DeviceInput {
                input: json!({"type":"encoder_press","id":"main"}),
                request_snapshot: Some(false),
            })
            .unwrap();
        let snapshot = terminal
            .iter()
            .position(|message| {
                matches!(message,
            RunnerMessage::Snapshot { snapshot } if snapshot["display"]["splash"] == "shutdown")
            })
            .unwrap();
        let effect = terminal.iter().position(|message| matches!(message, RunnerMessage::PlatformEffects { effects }
            if effects.iter().any(|effect| matches!(effect, RuntimePlatformEffect::Shutdown | RuntimePlatformEffect::Reboot)))).unwrap();
        let ordered_effects = terminal
            .iter()
            .flat_map(|message| match message {
                RunnerMessage::PlatformEffects { effects } => effects.iter().collect::<Vec<_>>(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>();
        let recovery = ordered_effects
            .iter()
            .position(|effect| matches!(effect, RuntimePlatformEffect::StoreSaveRecovery { .. }))
            .unwrap();
        let power = ordered_effects
            .iter()
            .position(|effect| {
                matches!(
                    effect,
                    RuntimePlatformEffect::Shutdown | RuntimePlatformEffect::Reboot
                )
            })
            .unwrap();
        assert!(recovery < power);
        assert!(snapshot < effect);
        assert!(!runner.display_scene_pending());
    }
}
