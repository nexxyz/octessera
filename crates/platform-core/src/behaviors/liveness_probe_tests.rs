use super::{get_native_behavior, list_native_behavior_ids, NativeBehavior};
use crate::behavior::{BehaviorContext, CellTriggerType};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Default)]
struct BehaviorLivenessMetrics {
    min_visible: usize,
    max_visible: usize,
    last_visible: usize,
    unique_signatures: usize,
    total_changed_cells: usize,
    activate_triggers: usize,
    deactivate_triggers: usize,
    stable_triggers: usize,
}

impl BehaviorLivenessMetrics {
    fn classification(&self) -> &'static str {
        if self.max_visible == 0 {
            "blank"
        } else if self.unique_signatures <= 1 {
            "static"
        } else if self.unique_signatures <= 4 || self.total_changed_cells <= 4 {
            "minimal"
        } else {
            "live"
        }
    }
}

fn collect_behavior_liveness(behavior: NativeBehavior, ticks: usize) -> BehaviorLivenessMetrics {
    let mut context = BehaviorContext::new(120.0);
    let mut state = behavior.init(Value::Null).unwrap();
    let mut signatures = HashSet::new();
    let mut previous_cells = None;
    let mut metrics = BehaviorLivenessMetrics {
        min_visible: usize::MAX,
        ..Default::default()
    };

    for step in 0..=ticks {
        let model = behavior.render_model(&state).unwrap();
        let visible = model.cells.iter().filter(|cell| **cell).count();
        metrics.min_visible = metrics.min_visible.min(visible);
        metrics.max_visible = metrics.max_visible.max(visible);
        metrics.last_visible = visible;

        if let Some(previous_cells) = &previous_cells {
            metrics.total_changed_cells += model
                .cells
                .iter()
                .zip(previous_cells)
                .filter(|(current, previous)| current != previous)
                .count();
        }
        previous_cells = Some(model.cells.clone());
        signatures.insert(model.cells);

        if let Some(trigger_types) = model.trigger_types {
            for trigger_type in trigger_types {
                match trigger_type {
                    CellTriggerType::Activate => metrics.activate_triggers += 1,
                    CellTriggerType::Deactivate => metrics.deactivate_triggers += 1,
                    CellTriggerType::Stable => metrics.stable_triggers += 1,
                    CellTriggerType::Scanned | CellTriggerType::None => {}
                }
            }
        }

        if step < ticks {
            state = behavior.on_tick(state, &mut context).unwrap();
        }
    }

    metrics.unique_signatures = signatures.len();
    metrics
}

#[test]
#[ignore]
fn native_behavior_default_liveness_probe() {
    println!(
        "behavior\tmin_visible\tmax_visible\tlast_visible\tunique_signatures\ttotal_changed_cells\tactivate\tdeactivate\tstable\tclassification"
    );

    for id in list_native_behavior_ids() {
        let behavior = get_native_behavior(id).unwrap();
        let metrics = collect_behavior_liveness(behavior, 128);
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            id,
            metrics.min_visible,
            metrics.max_visible,
            metrics.last_visible,
            metrics.unique_signatures,
            metrics.total_changed_cells,
            metrics.activate_triggers,
            metrics.deactivate_triggers,
            metrics.stable_triggers,
            metrics.classification()
        );
    }
}
