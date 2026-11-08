use super::*;

/// The factory patch with its Raindrops layer emptied (Life, two Sequencers, a Looper and a gated
/// Twinkle over three `none` layers) with swing,
/// Link delays and retrigs, an arp, shared and seeded probability and a scanning `none` layer.
fn mixed_patch_runner() -> NativeRunner {
    let mut payload: Value =
        serde_json::from_str(include_str!("../../../../../config/default.json")).unwrap();
    payload["runtimeConfig"]["layers"][5] = payload["runtimeConfig"]["layers"][6].clone();
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    runner.apply_config_payload(payload).unwrap();
    runner.midi_active = true;
    for (index, behavior_id) in runner.layer_behavior_ids.iter().enumerate() {
        assert_eq!(behavior_id == "none", index >= 5, "factory layer {index}");
    }
    for (key, value) in [("transport.swingA", 4), ("transport.swingB", 3)] {
        if runner.menu.set_number_value_for_key(key, value) {
            runner.apply_or_schedule_menu_key(key).unwrap();
        }
    }
    assert_eq!((runner.transport.swing_a, runner.transport.swing_b), (4, 3));
    runner.link_layers[0].activate_timing = LinkEventTiming {
        delay_steps: 1,
        retrigger_count: 1,
    };
    runner.link_layers[0].trigger_probability_mode = "custom".into();
    runner.link_layers[0].trigger_probability_high_pct = 70;
    runner.trigger_probability_maps[0] = vec!["high".into(); GRID_WIDTH * GRID_HEIGHT];
    runner.link_layers[1].arp.mode = "up".into();
    runner.link_layers[3].seeded = true;
    runner.link_layers[3].scanned_timing = LinkEventTiming {
        delay_steps: 2,
        retrigger_count: 0,
    };
    let scanning_none = &mut runner.link_layers[6];
    scanning_none.scan_mode = "scanning".into();
    scanning_none.scan_unit = "1/8".into();
    scanning_none.scanned_empty_slot = 1;
    scanning_none.scanned_empty_action = "note_on".into();
    scanning_none.trigger_probability_mode = "custom".into();
    scanning_none.trigger_probability_low_pct = 50;
    runner.trigger_probability_maps[6] = vec!["low".into(); GRID_WIDTH * GRID_HEIGHT];
    runner
}

/// A `none` layer that scans with every scan action off still ticks, sets up and routes each
/// step, yet plays nothing and draws no probability at full chance.
fn scan_none_layers_silently(runner: &mut NativeRunner) {
    for index in 5..LAYER_COUNT {
        if runner.link_layers[index].scan_mode == "scanning" {
            continue;
        }
        let step_pulses = runner.layer_step_rate_pulses(index);
        let layer = &mut runner.link_layers[index];
        layer.scan_mode = "scanning".into();
        layer.scan_unit = "1/8".into();
        layer.scanned_action = "none".into();
        layer.scanned_empty_action = "none".into();
        assert_eq!(layer.trigger_probability_mode, "full");
        assert_eq!(runner.step_pulses_for_layer(index), step_pulses);
    }
}

fn music(messages: &[RunnerMessage]) -> Vec<&RunnerMessage> {
    messages
        .iter()
        .filter(|message| {
            matches!(
                message,
                RunnerMessage::MusicalEvents { .. }
                    | RunnerMessage::TileHits { .. }
                    | RunnerMessage::MidiEvents { .. }
                    | RunnerMessage::SequencedOffset { .. }
            )
        })
        .collect()
}

/// Every musical, tile, MIDI and swing message per pulse step from Start, plus the per-layer step
/// state at the end. A delayed note waits on layer 5, layer 7 starts scanning audibly mid-bar 5,
/// and layer 5 becomes the active layer at bar 7.
fn eight_bar_trace(mut runner: NativeRunner) -> String {
    let start = runner.send(HostMessage::MidiRealtimeStart).unwrap();
    let mut lines = vec![format!("0 {:?}", music(&start))];
    runner.delayed_link_events[5].push(DelayedRoutedEvents {
        remaining_steps: 3,
        events: RoutedMusicalEvents {
            audio: vec![platform_core::MusicalEvent::NoteOn {
                channel: 0,
                note: 50,
                velocity: 90,
                duration_ms: Some(100),
            }],
            ..RoutedMusicalEvents::default()
        },
    });
    let mut pulse = 0_u32;
    let mut late_scanner_started = false;
    for step in [1_u32, 1, 2, 1, 3, 6, 1, 1, 24, 2, 4, 2].iter().cycle() {
        if pulse >= 8 * 96 {
            break;
        }
        if pulse >= 4 * 96 + 36 && !late_scanner_started {
            late_scanner_started = true;
            let late_scanner = &mut runner.link_layers[7];
            late_scanner.scan_mode = "scanning".into();
            late_scanner.scan_unit = "1/16".into();
            late_scanner.scan_axis = "columns".into();
            late_scanner.x_pitch_enabled = true;
            late_scanner.x_pitch_steps = 1;
            late_scanner.scanned_empty_slot = 0;
            late_scanner.scanned_empty_action = "note_on".into();
        }
        if pulse >= 6 * 96 && runner.active_layer_index == 0 {
            runner.select_active_layer(5).unwrap();
        }
        pulse += step;
        let messages = runner
            .send(HostMessage::TransportPulseStep {
                pulses: *step,
                source: SyncSource::Internal,
                at_ppqn_pulse: None,
                request_snapshot: Some(false),
            })
            .unwrap();
        let music = music(&messages);
        if !music.is_empty() {
            lines.push(format!("{pulse} {music:?}"));
        }
    }
    lines.push(format!(
        "end ticks={:?} accumulators={:?} delayed={:?}",
        runner.transport.layer_ticks,
        runner.transport.layer_pulse_accumulators,
        runner
            .delayed_link_events
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>()
    ));
    lines.join("\n")
}

#[test]
pub(crate) fn idle_none_layers_play_what_the_full_step_path_plays() {
    let idle = eight_bar_trace(mixed_patch_runner());
    let mut full_path = mixed_patch_runner();
    scan_none_layers_silently(&mut full_path);
    let full_path = eight_bar_trace(full_path);
    assert!(idle.contains("note: 50, velocity: 90"), "{idle}");
    assert_eq!(idle, full_path);
}
