//! Presses a direct layer did not play when they came (Note Preview off) play at its next step:
//! a tile a press changed fires its net change from the grid before the press to the grid after
//! the step, so a press that turns off a sounding tile still ends its note.

use crate::behavior::TileTriggerType;
use crate::interpretation::{
    GridSnapshot, InterpretationProfile, TileTriggerIntent, TileTriggerKind,
};
use crate::interpretation_scan::compute_degree;

/// The step's `intents`, in their order, without those of the tiles a press changed since the
/// last step (`unplayed` before the press, `before` the grid the step started from), then each
/// such tile's net change to `next`. A tile the behavior marked itself this step (a struck
/// paddle, an accent) keeps the step's own events.
pub(super) fn fold_unplayed_presses(
    unplayed: &GridSnapshot,
    before: &GridSnapshot,
    next: &GridSnapshot,
    profile: &InterpretationProfile,
    intents: Vec<TileTriggerIntent>,
) -> Vec<TileTriggerIntent> {
    let width = next.width.max(1);
    let marked = |index: usize| {
        next.trigger_types.as_ref().is_some_and(|types| {
            matches!(
                types.get(index),
                Some(TileTriggerType::Activate | TileTriggerType::Deactivate)
            )
        })
    };
    let folds =
        |index: usize| unplayed.tiles.get(index) != before.tiles.get(index) && !marked(index);
    let mut folded = intents
        .into_iter()
        .filter(|intent| !folds(intent.y * width + intent.x))
        .collect::<Vec<_>>();
    for index in (0..next.tiles.len()).filter(|index| folds(*index)) {
        let (was, now) = (unplayed.tiles.get(index) == Some(&true), next.tiles[index]);
        let kind = match (was, now) {
            (false, true) => TileTriggerKind::Activate,
            (true, false) => TileTriggerKind::Deactivate,
            _ => continue,
        };
        let (x, y) = (index % width, index / width);
        folded.push(TileTriggerIntent {
            x,
            y,
            kind,
            degree: compute_degree(next.width, next.height, x, y, profile),
        });
    }
    folded
}
