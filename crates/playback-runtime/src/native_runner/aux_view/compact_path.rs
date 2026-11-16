use crate::native_menu::NativeMenuItem;

/// A manual target's `owner/page`: the object that owns the row by its short id (`I1`, `B2`,
/// `Master`, `L3`, `LFO2`, `CC1`, `Play`, `SYS`) and the page that directly holds it. `chain` runs
/// from the top-level section down to the row; `behavior_of` names a layer's behavior, which is
/// the page of every Build target.
pub(super) fn compact_path(
    chain: &[&NativeMenuItem],
    behavior_of: impl Fn(usize) -> Option<String>,
) -> Option<String> {
    let (section, rest) = chain.split_first()?;
    let groups = &rest[..rest.len().saturating_sub(1)];
    let owner_at = groups
        .iter()
        .position(|group| owner_id(&group.label).is_some());
    let (owner, owner_group) = match owner_at {
        Some(index) => (owner_id(&groups[index].label)?, Some(groups[index])),
        None => (section_owner(&section.label).into(), None),
    };
    let below = owner_at.map_or(groups, |index| &groups[index + 1..]);
    let page = if section.label == "Build" && owner.starts_with('L') {
        owner[1..]
            .parse::<usize>()
            .ok()
            .and_then(|layer| behavior_of(layer - 1))
    } else {
        below
            .last()
            .map(|parent| page_label(parent, below, owner_group))
    };
    Some(match page {
        Some(page) => format!("{owner}/{page}"),
        None => owner,
    })
}

/// The short id a group label names, when the group is an owner: `I1: Synth fxb3` gives `I1`.
fn owner_id(label: &str) -> Option<String> {
    if label == "Global FX" {
        return Some("Master".into());
    }
    let id = short(label);
    let digits = |prefix: &str| {
        id.strip_prefix(prefix)
            .is_some_and(|rest| !rest.is_empty() && rest.bytes().all(|byte| byte.is_ascii_digit()))
    };
    ["LFO", "CC", "I", "B", "L"]
        .into_iter()
        .any(digits)
        .then(|| id.to_owned())
}

fn section_owner(section: &str) -> &str {
    match section {
        "Play" => "Play",
        "System" => "SYS",
        other => other,
    }
}

/// A page's short label. A slot page is named by what it holds (`Slot 2: Reverb` gives
/// `Reverb`, with the slot number while another slot of the owner holds the same), and a Link
/// axis keeps its axis in front (`X Filter Cutoff`).
fn page_label(
    parent: &NativeMenuItem,
    below: &[&NativeMenuItem],
    owner_group: Option<&NativeMenuItem>,
) -> String {
    let page = match parent.label.split_once(": ") {
        Some((slot, held)) if slot.starts_with("Slot ") => {
            let twins = owner_group.map_or(0, |owner| {
                owner
                    .children
                    .iter()
                    .filter(|child| {
                        child.label.starts_with("Slot ")
                            && child.label.split_once(": ").map(|(_, other)| other) == Some(held)
                    })
                    .count()
            });
            if twins > 1 {
                format!("{held} {}", &slot["Slot ".len()..])
            } else {
                held.to_owned()
            }
        }
        _ => short(&parent.label).to_owned(),
    };
    let axis = below[..below.len() - 1]
        .iter()
        .find_map(|group| group.label.strip_suffix(" Axis"));
    match axis {
        Some(axis) => format!("{axis} {page}"),
        None => page,
    }
}

fn short(label: &str) -> &str {
    label.split_once(": ").map_or(label, |(short, _)| short)
}
