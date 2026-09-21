use super::{next_event, platform_request, test_adapter};
use playback_runtime::{HostAdapter, RuntimeAudioCommand, RuntimePlatformEffect};
use rodio_engine_source::EngineEvent;
use std::sync::Arc;

fn sampler_replacement(path: &str, generation: u64) -> RuntimePlatformEffect {
    RuntimePlatformEffect::AudioCommand {
        command: RuntimeAudioCommand::SetInstrumentSlot {
            instrument_slot: 0,
            generation,
            config: serde_json::json!({
                "type": "sampler",
                "sample": { "slots": [{ "path": path }] }
            }),
        },
    }
}

fn next_sampler_samples(
    rx: &mut rodio_engine_source::EngineEventReceiver,
    generation: u64,
) -> Arc<[f32]> {
    match next_event(rx) {
        EngineEvent::SetPreparedInstrumentOwner {
            instrument_slot: 0,
            generation: actual_generation,
            sample_bank: Some(bank),
            ..
        } => {
            assert_eq!(actual_generation, generation);
            bank.slots
                .into_iter()
                .next()
                .expect("sampler replacement must have a slot")
                .buffer
                .expect("sampler replacement must carry a sample")
                .samples
        }
        _ => panic!("expected atomic sampler owner event"),
    }
}

#[test]
fn sampler_replacement_uses_one_owner_event_with_one_structural_slot_free() {
    let (mut adapter, mut rx) = test_adapter();
    for epoch in 0..63 {
        adapter
            .audio
            .engine_tx
            .send(EngineEvent::MomentaryFxStop { epoch })
            .unwrap();
    }
    adapter
        .handle_platform_effect(&platform_request(sampler_replacement(
            "samples/Drum/kick/Kick2.wav",
            2,
        )))
        .unwrap();
    for _ in 0..63 {
        assert!(matches!(
            next_event(&mut rx),
            EngineEvent::MomentaryFxStop { .. }
        ));
    }
    assert!(!next_sampler_samples(&mut rx, 2).is_empty());
    assert!(rx.try_recv().is_err());
}

#[test]
fn sampler_assignment_changes_atomic_owner_bank_content() {
    let (mut adapter, mut rx) = test_adapter();
    adapter
        .handle_platform_effect(&platform_request(sampler_replacement(
            "samples/Drum/kick/Kick1.wav",
            1,
        )))
        .unwrap();
    let first = next_sampler_samples(&mut rx, 1);
    adapter
        .handle_platform_effect(&platform_request(sampler_replacement(
            "samples/Drum/kick/Kick2.wav",
            2,
        )))
        .unwrap();
    let second = next_sampler_samples(&mut rx, 2);
    assert_ne!(first.as_ref(), second.as_ref());
}
