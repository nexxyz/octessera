use super::*;

fn snapshot(tiles: &[u8]) -> GridSnapshot {
    GridSnapshot {
        width: 4,
        height: 2,
        tiles: tiles.iter().map(|tile| *tile != 0).collect(),
        trigger_types: None,
    }
}

fn snapshot_with_triggers(tiles: &[u8], trigger_types: Vec<TileTriggerType>) -> GridSnapshot {
    GridSnapshot {
        width: 4,
        height: 2,
        tiles: tiles.iter().map(|tile| *tile != 0).collect(),
        trigger_types: Some(trigger_types),
    }
}

fn profile(tick: TickStrategy) -> InterpretationProfile {
    InterpretationProfile {
        id: "menu_profile".into(),
        event: InterpretationEventProfile { enabled: true },
        state: InterpretationStateProfile {
            enabled: true,
            tick,
        },
        x: AxisStrategy::ScaleStep { step: 1 },
        y: AxisStrategy::ScaleStep { step: 2 },
    }
}

#[test]
fn extracts_transitions() {
    let previous = snapshot(&[0, 1, 0, 0, 1, 1, 0, 0]);
    let next = snapshot(&[1, 1, 0, 0, 0, 1, 1, 0]);
    assert_eq!(
        extract_transitions(&previous, &next),
        vec![
            TileTransition {
                x: 0,
                y: 0,
                kind: TileTransitionKind::Activate
            },
            TileTransition {
                x: 0,
                y: 1,
                kind: TileTransitionKind::Deactivate
            },
            TileTransition {
                x: 2,
                y: 1,
                kind: TileTransitionKind::Activate
            },
        ]
    );
}

#[test]
fn interprets_scan_row_with_degree() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 1, 0, 1, 1, 0, 0]);
    let intents = interpret_grid(
        &previous,
        &next,
        1,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: false,
            restart_pitch: false,
        }),
    );
    assert_eq!(
        intents,
        vec![
            TileTriggerIntent {
                x: 0,
                y: 0,
                kind: TileTriggerKind::Activate,
                degree: 0
            },
            TileTriggerIntent {
                x: 2,
                y: 0,
                kind: TileTriggerKind::Activate,
                degree: 2
            },
            TileTriggerIntent {
                x: 0,
                y: 1,
                kind: TileTriggerKind::Activate,
                degree: 2
            },
            TileTriggerIntent {
                x: 1,
                y: 1,
                kind: TileTriggerKind::Activate,
                degree: 3
            },
            TileTriggerIntent {
                x: 0,
                y: 1,
                kind: TileTriggerKind::Scanned,
                degree: 2
            },
            TileTriggerIntent {
                x: 1,
                y: 1,
                kind: TileTriggerKind::Scanned,
                degree: 3
            },
            TileTriggerIntent {
                x: 2,
                y: 1,
                kind: TileTriggerKind::ScannedEmpty,
                degree: 4
            },
            TileTriggerIntent {
                x: 3,
                y: 1,
                kind: TileTriggerKind::ScannedEmpty,
                degree: 5
            },
        ]
    );
}

#[test]
fn reverse_scan_row_starts_from_last_row() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 0, 0, 0, 0, 0, 1]);
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: true,
            restart_pitch: false,
        }),
    );
    assert!(intents
        .iter()
        .any(|intent| intent.x == 3 && intent.y == 1 && intent.kind == TileTriggerKind::Scanned));
    assert!(!intents
        .iter()
        .any(|intent| intent.x == 0 && intent.y == 0 && intent.kind == TileTriggerKind::Scanned));
}

#[test]
fn sectioned_scan_row_limits_output_to_current_section() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 1, 1, 1, 0, 0, 0, 0]);
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::ScanRowActive {
            sections: Some(2),
            reverse: false,
            restart_pitch: false,
        }),
    );
    let scanned = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                TileTriggerKind::Scanned | TileTriggerKind::ScannedEmpty
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(scanned.len(), 2);
    assert!(scanned.iter().all(|intent| intent.y == 0 && intent.x < 2));
}

#[test]
fn sectioned_scan_column_limits_output_to_current_section_and_reverse() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 0, 0, 0, 1, 0, 0]);
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::ScanColumnActive {
            sections: Some(2),
            reverse: true,
            restart_pitch: false,
        }),
    );
    let scanned = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                TileTriggerKind::Scanned | TileTriggerKind::ScannedEmpty
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(scanned.len(), 1);
    assert_eq!((scanned[0].x, scanned[0].y), (3, 1));
}

#[test]
fn zero_sized_scan_snapshot_is_safe() {
    let empty = GridSnapshot {
        width: 0,
        height: 0,
        tiles: Vec::new(),
        trigger_types: None,
    };
    assert!(interpret_grid(
        &empty,
        &empty,
        12,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: false,
            restart_pitch: false,
        }),
    )
    .is_empty());
}

#[test]
fn scan_column_emits_scanned_and_scanned_empty_for_all_rows() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 0, 0, 0, 1, 0, 0]);
    let intents = interpret_grid(
        &previous,
        &next,
        1,
        &profile(TickStrategy::ScanColumnActive {
            sections: None,
            reverse: false,
            restart_pitch: false,
        }),
    );
    let state_intents = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                TileTriggerKind::Scanned | TileTriggerKind::ScannedEmpty
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(state_intents.len(), 2);
    assert!(state_intents.iter().any(|intent| intent.x == 1
        && intent.y == 0
        && intent.kind == TileTriggerKind::ScannedEmpty));
    assert!(state_intents
        .iter()
        .any(|intent| intent.x == 1 && intent.y == 1 && intent.kind == TileTriggerKind::Scanned));
}

#[test]
fn scan_row_emits_scanned_empty_for_dead_tiles() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 1, 0, 0, 0, 0, 0]);
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: false,
            restart_pitch: false,
        }),
    );
    let state_intents = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                TileTriggerKind::Scanned | TileTriggerKind::ScannedEmpty
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(state_intents.len(), 4);
    assert!(state_intents.iter().any(|intent| intent.x == 1
        && intent.y == 0
        && intent.kind == TileTriggerKind::ScannedEmpty));
}

#[test]
fn whole_grid_active_emits_only_live_tiles() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 1, 0, 0, 0, 1, 0]);
    let intents = interpret_grid(&previous, &next, 0, &profile(TickStrategy::WholeGridActive));
    let state_intents = intents
        .iter()
        .filter(|intent| intent.kind == TileTriggerKind::Scanned)
        .collect::<Vec<_>>();
    assert_eq!(state_intents.len(), 3);
    assert!(state_intents
        .iter()
        .any(|intent| intent.x == 2 && intent.y == 0));
}

#[test]
fn event_candidates_use_render_trigger_types_when_valid() {
    let previous = snapshot_with_triggers(
        &[0, 1, 0, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::None,
            TileTriggerType::Stable,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );
    let next = snapshot_with_triggers(
        &[0, 1, 1, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::None,
            TileTriggerType::Activate,
            TileTriggerType::Deactivate,
            TileTriggerType::Stable,
            TileTriggerType::Scanned,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::WholeGridTransitions),
    );
    assert_eq!(
        intents,
        vec![
            TileTriggerIntent {
                x: 1,
                y: 0,
                kind: TileTriggerKind::Activate,
                degree: 1
            },
            TileTriggerIntent {
                x: 2,
                y: 0,
                kind: TileTriggerKind::Deactivate,
                degree: 2
            }
        ]
    );
}

#[test]
fn event_candidates_ignore_persistent_stale_render_trigger_types() {
    let stale_triggers = vec![
        TileTriggerType::Activate,
        TileTriggerType::Deactivate,
        TileTriggerType::Stable,
        TileTriggerType::None,
        TileTriggerType::None,
        TileTriggerType::None,
        TileTriggerType::None,
        TileTriggerType::None,
    ];
    let previous = snapshot_with_triggers(&[1, 1, 0, 0, 0, 0, 0, 0], stale_triggers.clone());
    let next = snapshot_with_triggers(&[1, 1, 0, 0, 0, 0, 0, 0], stale_triggers);

    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::WholeGridTransitions),
    );

    assert!(intents.is_empty());
}

#[test]
fn fresh_tick_markers_preserve_nonzero_activation_intents() {
    let previous = snapshot_with_triggers(
        &[1, 0, 0, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::Activate,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );
    let next = snapshot_with_triggers(
        &[2, 0, 0, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::Activate,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );

    let intents = interpret_grid_with_marker_authority(
        &previous,
        &next,
        0,
        &profile(TickStrategy::WholeGridTransitions),
        TriggerMarkerAuthority::FreshTick,
    );

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].kind, TileTriggerKind::Activate);
}

#[test]
fn restart_pitch_counts_from_each_section_lane_on_the_split_axis() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 1, 1, 1, 1, 1, 1, 1]);
    let degrees = |tick: TickStrategy, step: usize| {
        let mut intents = interpret_grid(&previous, &next, step, &profile(tick))
            .into_iter()
            .filter(|intent| intent.kind == TileTriggerKind::Scanned)
            .map(|intent| (intent.x, intent.y, intent.degree))
            .collect::<Vec<_>>();
        intents.sort();
        intents
    };
    let columns = |restart_pitch| TickStrategy::ScanColumnActive {
        sections: Some(2),
        reverse: false,
        restart_pitch,
    };
    assert_eq!(degrees(columns(false), 5), vec![(1, 1, 3)]);
    assert_eq!(degrees(columns(true), 5), vec![(1, 1, 1)]);

    let rows = |restart_pitch| TickStrategy::ScanRowActive {
        sections: Some(2),
        reverse: false,
        restart_pitch,
    };
    assert_eq!(degrees(rows(false), 2), vec![(2, 0, 2), (3, 0, 3)]);
    assert_eq!(degrees(rows(true), 2), vec![(2, 0, 0), (3, 0, 1)]);
}
