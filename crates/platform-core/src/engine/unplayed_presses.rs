//! Presses a direct layer did not play when they came (Note Preview off) play at its next step:
//! a tile a press changed fires its net change from the grid before the press to the grid after
//! the step, so a press that turns off a sounding tile still ends its note.

use crate::interpretation::{
    GridSnapshot, InterpretationProfile, TileTriggerIntent, TileTriggerKind,
};
use crate::interpretation_scan::compute_degree;

/// The step's `intents` with every tile a press changed since the last step (`unplayed` before
/// the press, `before` the grid the step started from) replaced by its net change to `next`.
pub(super) fn fold_unplayed_presses(
    unplayed: &GridSnapshot,
    before: &GridSnapshot,
    next: &GridSnapshot,
    profile: &InterpretationProfile,
    intents: Vec<TileTriggerIntent>,
) -> Vec<TileTriggerIntent> {
    let width = next.width.max(1);
    let pressed = |index: usize| unplayed.tiles.get(index) != before.tiles.get(index);
    let mut folded = intents
        .into_iter()
        .filter(|intent| !pressed(intent.y * width + intent.x))
        .collect::<Vec<_>>();
    for index in (0..next.tiles.len()).filter(|index| pressed(*index)) {
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
    folded.sort_by_key(|intent| intent.y * width + intent.x);
    folded
}
