//! Presses a direct layer did not play when they came (Note Preview off) play at its next step:
//! it plays each pressed tile's change since before the press, so a tile that ends where it
//! started plays nothing, and a press that turns off a sounding tile still ends its note.

use crate::interpretation::{
    GridSnapshot, InterpretationProfile, TileTriggerIntent, TileTriggerKind,
};
use crate::interpretation_scan::compute_degree;

/// The step's `intents`, in their order, then the net change to `next` of each tile a press
/// changed since the last step (`unplayed` before the press, `before` the grid the step started
/// from). On such a tile the net change wins: the step's own event stays only where it plays
/// that change, or a Stable on a tile lit throughout, and a tile that ends where it started
/// plays nothing else.
pub(super) fn fold_unplayed_presses(
    unplayed: &GridSnapshot,
    before: &GridSnapshot,
    next: &GridSnapshot,
    profile: &InterpretationProfile,
    intents: Vec<TileTriggerIntent>,
) -> Vec<TileTriggerIntent> {
    let width = next.width.max(1);
    let pressed = |index: usize| unplayed.tiles.get(index) != before.tiles.get(index);
    let net = |index: usize| {
        let was = unplayed.tiles.get(index) == Some(&true);
        match (was, next.tiles.get(index) == Some(&true)) {
            (false, true) => Some(TileTriggerKind::Activate),
            (true, false) => Some(TileTriggerKind::Deactivate),
            (true, true) => Some(TileTriggerKind::Stable),
            (false, false) => None,
        }
    };
    let mut folded = intents
        .into_iter()
        .filter(|intent| {
            let index = intent.y * width + intent.x;
            !pressed(index) || net(index) == Some(intent.kind)
        })
        .collect::<Vec<_>>();
    for index in (0..next.tiles.len()).filter(|index| pressed(*index)) {
        let kind = match net(index) {
            Some(kind @ (TileTriggerKind::Activate | TileTriggerKind::Deactivate)) => kind,
            _ => continue,
        };
        let (x, y) = (index % width, index / width);
        if folded
            .iter()
            .any(|intent| (intent.x, intent.y, intent.kind) == (x, y, kind))
        {
            continue;
        }
        folded.push(TileTriggerIntent {
            x,
            y,
            kind,
            degree: compute_degree(next.width, next.height, x, y, profile),
        });
    }
    folded
}
