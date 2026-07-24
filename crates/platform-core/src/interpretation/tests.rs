use super::*;

fn snapshot(cells: &[u8]) -> GridSnapshot {
    GridSnapshot {
        width: 4,
        height: 2,
        cells: cells.iter().map(|cell| *cell != 0).collect(),
        trigger_types: None,
    }
}

fn snapshot_with_triggers(cells: &[u8], trigger_types: Vec<CellTriggerType>) -> GridSnapshot {
    GridSnapshot {
        width: 4,
        height: 2,
        cells: cells.iter().map(|cell| *cell != 0).collect(),
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
            CellTransition {
                x: 0,
                y: 0,
                kind: CellTransitionKind::Activate
            },
            CellTransition {
                x: 0,
                y: 1,
                kind: CellTransitionKind::Deactivate
            },
            CellTransition {
                x: 2,
                y: 1,
                kind: CellTransitionKind::Activate
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
        }),
    );
    assert_eq!(
        intents,
        vec![
            CellTriggerIntent {
                x: 0,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 0
            },
            CellTriggerIntent {
                x: 2,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 2
            },
            CellTriggerIntent {
                x: 0,
                y: 1,
                kind: CellTriggerKind::Activate,
                degree: 2
            },
            CellTriggerIntent {
                x: 1,
                y: 1,
                kind: CellTriggerKind::Activate,
                degree: 3
            },
            CellTriggerIntent {
                x: 0,
                y: 1,
                kind: CellTriggerKind::Scanned,
                degree: 2
            },
            CellTriggerIntent {
                x: 1,
                y: 1,
                kind: CellTriggerKind::Scanned,
                degree: 3
            },
            CellTriggerIntent {
                x: 2,
                y: 1,
                kind: CellTriggerKind::ScannedEmpty,
                degree: 4
            },
            CellTriggerIntent {
                x: 3,
                y: 1,
                kind: CellTriggerKind::ScannedEmpty,
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
        }),
    );
    assert!(intents
        .iter()
        .any(|intent| intent.x == 3 && intent.y == 1 && intent.kind == CellTriggerKind::Scanned));
    assert!(!intents
        .iter()
        .any(|intent| intent.x == 0 && intent.y == 0 && intent.kind == CellTriggerKind::Scanned));
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
        }),
    );
    let scanned = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
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
        }),
    );
    let scanned = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
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
        cells: Vec::new(),
        trigger_types: None,
    };
    assert!(interpret_grid(
        &empty,
        &empty,
        12,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: false,
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
        }),
    );
    let state_intents = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(state_intents.len(), 2);
    assert!(state_intents.iter().any(|intent| intent.x == 1
        && intent.y == 0
        && intent.kind == CellTriggerKind::ScannedEmpty));
    assert!(state_intents
        .iter()
        .any(|intent| intent.x == 1 && intent.y == 1 && intent.kind == CellTriggerKind::Scanned));
}

#[test]
fn scan_row_emits_scanned_empty_for_dead_cells() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 1, 0, 0, 0, 0, 0]);
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: false,
        }),
    );
    let state_intents = intents
        .iter()
        .filter(|intent| {
            matches!(
                intent.kind,
                CellTriggerKind::Scanned | CellTriggerKind::ScannedEmpty
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(state_intents.len(), 4);
    assert!(state_intents.iter().any(|intent| intent.x == 1
        && intent.y == 0
        && intent.kind == CellTriggerKind::ScannedEmpty));
}

#[test]
fn whole_grid_active_emits_only_live_cells() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 0, 1, 0, 0, 0, 1, 0]);
    let intents = interpret_grid(&previous, &next, 0, &profile(TickStrategy::WholeGridActive));
    let state_intents = intents
        .iter()
        .filter(|intent| intent.kind == CellTriggerKind::Scanned)
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
            CellTriggerType::None,
            CellTriggerType::Stable,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
        ],
    );
    let next = snapshot_with_triggers(
        &[0, 1, 1, 0, 0, 0, 0, 0],
        vec![
            CellTriggerType::None,
            CellTriggerType::Activate,
            CellTriggerType::Deactivate,
            CellTriggerType::Stable,
            CellTriggerType::Scanned,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
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
            CellTriggerIntent {
                x: 1,
                y: 0,
                kind: CellTriggerKind::Activate,
                degree: 1
            },
            CellTriggerIntent {
                x: 2,
                y: 0,
                kind: CellTriggerKind::Deactivate,
                degree: 2
            }
        ]
    );
}

#[test]
fn event_candidates_ignore_persistent_stale_render_trigger_types() {
    let stale_triggers = vec![
        CellTriggerType::Activate,
        CellTriggerType::Deactivate,
        CellTriggerType::Stable,
        CellTriggerType::None,
        CellTriggerType::None,
        CellTriggerType::None,
        CellTriggerType::None,
        CellTriggerType::None,
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
            CellTriggerType::Activate,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
        ],
    );
    let next = snapshot_with_triggers(
        &[2, 0, 0, 0, 0, 0, 0, 0],
        vec![
            CellTriggerType::Activate,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
            CellTriggerType::None,
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
    assert_eq!(intents[0].kind, CellTriggerKind::Activate);
}
