use crate::native_menu::NativeMenuHelpTarget;
use std::sync::OnceLock;

const MENU_HELP_TSV: &str = include_str!("../../../resources/menu-help-texts.tsv");

#[derive(Clone, Debug)]
pub struct NativeHelpEntry {
    pub path: String,
    pub key: String,
    pub kind: String,
    pub title: String,
    pub line1: String,
    pub line2: String,
}

#[derive(Clone, Debug)]
pub struct NativeResolvedHelp {
    pub title: String,
    pub detail: String,
}

pub fn resolve_native_help(target: &NativeMenuHelpTarget) -> Option<NativeResolvedHelp> {
    let entry = resolve_native_help_entry(target)?;
    let detail = [entry.line1.as_str(), entry.line2.as_str()]
        .into_iter()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Some(NativeResolvedHelp {
        title: if entry.title.trim().is_empty() {
            target.label.clone()
        } else {
            entry.title
        },
        detail,
    })
}

pub fn resolve_native_help_entry(target: &NativeMenuHelpTarget) -> Option<NativeHelpEntry> {
    let mut best = None;
    let mut best_score = -1;
    for entry in menu_help_entries_cached() {
        if !is_specific_entry(entry) {
            continue;
        }
        let score = entry_score(entry, target);
        if score > best_score {
            best = Some(entry.clone());
            best_score = score;
        }
    }
    if best_score < 0 {
        None
    } else {
        best
    }
}

fn menu_help_entries_cached() -> &'static Vec<NativeHelpEntry> {
    static ENTRIES: OnceLock<Vec<NativeHelpEntry>> = OnceLock::new();
    ENTRIES.get_or_init(parse_menu_help_entries)
}

#[cfg(test)]
pub(crate) fn native_help_entries_for_tests() -> &'static Vec<NativeHelpEntry> {
    menu_help_entries_cached()
}

fn parse_menu_help_entries() -> Vec<NativeHelpEntry> {
    MENU_HELP_TSV
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with("id\t")
        })
        .filter_map(|line| {
            let cols = line.split('\t').collect::<Vec<_>>();
            if cols.len() < 7 {
                return None;
            }
            Some(NativeHelpEntry {
                path: cols[1].into(),
                key: cols[2].into(),
                kind: cols[3].into(),
                title: cols[4].into(),
                line1: cols[5].into(),
                line2: cols[6].into(),
            })
        })
        .collect()
}

fn entry_score(entry: &NativeHelpEntry, target: &NativeMenuHelpTarget) -> i32 {
    let tier = match_tier(entry, target);
    if tier < 0 {
        return -1;
    }
    let key_specificity = entry.key.replace('*', "").len() as i32;
    let path_specificity = entry.path.replace('*', "").len() as i32;
    let kind_bonus = if entry.kind.trim().is_empty() || entry.kind == "*" {
        0
    } else {
        1
    };
    500 - tier * 100 + key_specificity + path_specificity + kind_bonus
}

fn match_tier(entry: &NativeHelpEntry, target: &NativeMenuHelpTarget) -> i32 {
    let key = entry.key.trim();
    let path = entry.path.trim();
    let kind = entry.kind.trim();
    if !kind.is_empty() && kind != "*" && kind != target.kind {
        return -1;
    }
    if !key.is_empty() {
        if !glob_match(key, &target.key) {
            return -1;
        }
    } else if !path.is_empty() && !path_match(path, &target.path) {
        return -1;
    }
    if !key.is_empty() && !key.contains('*') {
        0
    } else if !key.is_empty() {
        if key == "action:*" || key == "key:*" {
            4
        } else {
            1
        }
    } else if !path.is_empty() && !path.contains('*') {
        2
    } else if !path.is_empty() {
        3
    } else {
        4
    }
}

fn path_match(pattern: &str, value: &str) -> bool {
    if glob_match(pattern, value) {
        return true;
    }
    let normalized_pattern = pattern.strip_prefix("Menu > ").unwrap_or(pattern);
    let normalized_value = value.strip_prefix("Menu > ").unwrap_or(value);
    if glob_match(normalized_pattern, normalized_value) {
        return true;
    }
    let segments = normalized_value.split(" > ").collect::<Vec<_>>();
    (1..segments.len()).any(|index| glob_match(normalized_pattern, &segments[index..].join(" > ")))
}

fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern.is_empty() || pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return pattern == value;
    }
    let mut rest = value;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(found) = rest.find(part) else {
            return false;
        };
        if index == 0 && found != 0 {
            return false;
        }
        rest = &rest[found + part.len()..];
    }
    parts
        .last()
        .is_some_and(|part| part.is_empty() || value.ends_with(part))
}

fn is_specific_entry(entry: &NativeHelpEntry) -> bool {
    let key = entry.key.trim();
    let path = entry.path.trim();
    let text = format!("{} {} {}", entry.title, entry.line1, entry.line2).to_lowercase();
    if path == "*" && key.is_empty() {
        return false;
    }
    if key == "action:*" || key == "key:*" {
        return false;
    }
    !text.contains("opens this submenu")
        && !text.contains("shows related settings")
        && !text.contains("runs this command")
        && !text.contains("adjusts a numeric value")
        && !text.contains("selects one option from a list")
        && !text.contains("edits text for this field")
        && !text.contains("no help text is available")
        && !text.trim().is_empty()
}
