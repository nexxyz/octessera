use super::*;
use crate::native_menu::{FactoryLoadKind, PlatformAction};
use std::path::Path;

#[test]
fn three_factory_sampler_kits_reference_library_files_in_voice_order() {
    let samples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples");
    for (id, expected) in [
        (
            "sdbkit",
            [
                "Drum/kick/sdbkit-kick.wav",
                "Drum/snare/sdbkit-snare.wav",
                "Drum/hihat closed/sdbkit-hatclsd.wav",
                "Drum/hihat open/sdbkit-hatopen.wav",
                "Drum/toms/sdbkit-lotom.wav",
                "Drum/toms/sdbkit-hitom.wav",
                "Drum/claps/sdbkit-clap.wav",
                "Drum/percussion/sdbkit-fmperc.wav",
            ],
        ),
        (
            "synthkit",
            [
                "Drum/kick/synthkit-kick.wav",
                "Drum/snare/synthkit-snare.wav",
                "Drum/hihat closed/synthkit-hatclsd.wav",
                "Drum/hihat open/synthkit-hatopen.wav",
                "Drum/toms/synthkit-lotom.wav",
                "Drum/toms/synthkit-hitom.wav",
                "Drum/claps/synthkit-clap.wav",
                "Drum/percussion/synthkit-8bit.wav",
            ],
        ),
        (
            "distkit",
            [
                "Drum/kick/distkit-kick.wav",
                "Drum/snare/distkit-snare.wav",
                "Drum/hihat closed/distkit-hatclsd.wav",
                "Drum/hihat open/distkit-hatopen.wav",
                "Drum/toms/distkit-lotom.wav",
                "Drum/toms/distkit-hitom.wav",
                "Drum/claps/distkit-clap.wav",
                "Drum/percussion/distkit-cowbell.wav",
            ],
        ),
    ] {
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner
            .execute_confirmed_action(NativeMenuAction::PlatformEffect(
                PlatformAction::FactoryInstrumentLoad {
                    kind: FactoryLoadKind::SampleKit,
                    slot: 0,
                    id: id.into(),
                },
            ))
            .unwrap();
        for (index, relative) in expected.into_iter().enumerate() {
            let path = format!("samples/{relative}");
            assert_eq!(
                runner.instruments[0].sample_paths[index].as_deref(),
                Some(path.as_str())
            );
            assert!(
                samples.join(relative).is_file(),
                "missing library sample {path}"
            );
        }
    }
}

#[test]
fn sampler_factory_load_does_not_change_sample_browser_navigation() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner
        .execute_confirmed_action(NativeMenuAction::PlatformEffect(
            PlatformAction::FactoryInstrumentLoad {
                kind: FactoryLoadKind::SampleKit,
                slot: 0,
                id: "sdbkit".into(),
            },
        ))
        .unwrap();
    runner.sample_browser = Some(NativeSampleBrowser {
        instrument_slot: 0,
        sample_slot: 0,
        dir: "samples/Drum".into(),
        entries: vec![SampleEntry {
            name: "kick".into(),
            path: "samples/Drum/kick".into(),
            is_dir: true,
        }],
    });
    let up = runner
        .execute_menu_action(NativeMenuAction::PlatformEffect(PlatformAction::SampleUp {
            instrument_slot: 0,
            sample_slot: 0,
        }))
        .unwrap();
    assert_eq!(
        up,
        Some(RuntimePlatformEffect::SampleListRequest {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "samples".into()
        })
    );
    let folder = runner
        .execute_menu_action(NativeMenuAction::PlatformEffect(
            PlatformAction::SampleEnter {
                instrument_slot: 0,
                sample_slot: 0,
                dir: "samples/Drum/kick".into(),
            },
        ))
        .unwrap();
    assert_eq!(
        folder,
        Some(RuntimePlatformEffect::SampleListRequest {
            instrument_slot: 0,
            sample_slot: 0,
            dir: "samples/Drum/kick".into()
        })
    );
    assert!(runner.sample_browser.as_ref().unwrap().entries.is_empty());
}

#[test]
fn every_factory_choice_keeps_valid_saved_config_and_drum_assignments() {
    for (kind, ids) in [
        (
            FactoryLoadKind::FmPreset,
            &["init", "soft_keys", "bell"][..],
        ),
        (
            FactoryLoadKind::PluckPreset,
            &["init", "nylon", "steel", "muted"],
        ),
        (FactoryLoadKind::DrumKit, &["default", "tight", "heavy"]),
    ] {
        for id in ids {
            let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
            runner.instruments[3].drum_config["assignments"] = json!([
                { "x": 2, "y": 5, "voice": 7, "tuneSemis": -24 },
                { "x": 2, "y": 0, "voice": 0, "tuneSemis": 24 },
            ]);
            let assignments = runner.instruments[3].drum_config["assignments"].clone();
            runner
                .execute_confirmed_action(NativeMenuAction::PlatformEffect(
                    PlatformAction::FactoryInstrumentLoad {
                        kind,
                        slot: 3,
                        id: (*id).into(),
                    },
                ))
                .unwrap();
            assert_eq!(
                runner.instruments[3].drum_config["assignments"],
                assignments
            );
            assert_eq!(
                runner.instruments[3].drum_config["voices"]
                    .as_array()
                    .unwrap()
                    .len(),
                8
            );
            let mut restored = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
            restored
                .apply_config_payload(runner.config_payload())
                .unwrap();
            assert_eq!(
                restored.instruments[3], runner.instruments[3],
                "{kind:?}:{id}"
            );
        }
    }
}

#[test]
fn factory_aux_click_actions_remain_musical_in_portable_patch() {
    for (kind, id) in [
        (FactoryLoadKind::FmPreset, "bell"),
        (FactoryLoadKind::PluckPreset, "nylon"),
        (FactoryLoadKind::SampleKit, "sdbkit"),
        (FactoryLoadKind::DrumKit, "heavy"),
    ] {
        let action = PlatformAction::FactoryInstrumentLoad {
            kind,
            slot: 0,
            id: id.into(),
        };
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        runner
            .execute_menu_action(NativeMenuAction::SetAuxClick {
                index: 0,
                action: Some(Box::new(NativeMenuAction::PlatformEffect(action.clone()))),
            })
            .unwrap();
        let patch = portable_patch_projection(&runner.config_payload()).unwrap();
        assert_eq!(
            patch["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"]["action"],
            action.as_wire()
        );
        let device = super::super::patch_device_payload::device_config_payload_from_payload(
            runner.config_payload(),
        )
        .unwrap();
        assert!(
            device["runtimeConfig"]["auxBindings"]["aux1"]["pressAction"].is_null(),
            "{action:?}"
        );
    }
}
