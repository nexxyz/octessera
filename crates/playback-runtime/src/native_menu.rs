use crate::protocol::SyncSource;
use platform_core::{BUS_COUNT as FX_BUS_COUNT, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_COUNT};

mod types;

pub use types::*;

const FX_BUS_SLOT_OPTIONS: &[&str] = &[
    "none",
    "tremolo",
    "delay",
    "vibrato",
    "chorus",
    "flanger",
    "filter_lfo",
    "wah",
    "reverb",
    "glitch",
    "auto_pan",
    "duck",
    "saturator",
    "distortion",
    "bitcrusher",
    "compressor",
    "eq",
];
const GLOBAL_FX_SLOT_OPTIONS: &[&str] = &[
    "none",
    "vinyl",
    "eq",
    "compressor",
    "saturator",
    "distortion",
];
const MENU_BODY_ROWS: usize = 7;

impl NativeMenuModel {
    pub fn new(config: NativeMenuConfig) -> Self {
        let numeric_display_mode = config.numeric_display_mode.clone();
        Self {
            root: build_root(config),
            state: NativeMenuState::default(),
            numeric_display_mode,
        }
    }

    pub fn rebuild(&mut self, config: NativeMenuConfig) {
        self.numeric_display_mode = config.numeric_display_mode.clone();
        self.root = build_root(config);
        let siblings_len = self.current_siblings().len();
        if siblings_len == 0 {
            self.state.cursor = 0;
        } else if self.state.cursor >= siblings_len {
            self.state.cursor = siblings_len - 1;
        }
    }

    pub fn turn(&mut self, delta: i8) {
        let siblings_len = self.current_siblings().len();
        if siblings_len == 0 || delta == 0 {
            return;
        }
        if self.state.editing {
            match &mut self.current_item_mut().value {
                NativeMenuValue::Enum { options, selected } => {
                    let max = options.len().saturating_sub(1);
                    let next =
                        ((*selected as isize) + delta as isize).clamp(0, max as isize) as usize;
                    *selected = next;
                }
                NativeMenuValue::Number {
                    value,
                    min,
                    max,
                    step,
                } => {
                    let next = (*value + i32::from(delta) * *step).clamp(*min, *max);
                    *value = next;
                }
                NativeMenuValue::Bool { value } => {
                    *value = delta > 0;
                }
                NativeMenuValue::Text {
                    value,
                    max_len,
                    cursor,
                } => {
                    turn_text_value(value, *max_len, cursor, delta);
                }
                _ => {}
            }
            return;
        }
        let mut next = self.state.cursor as isize;
        let max = siblings_len.saturating_sub(1) as isize;
        let mut attempts = 0usize;
        loop {
            next = (next + delta as isize).clamp(0, max);
            attempts += 1;
            let idx = next as usize;
            if !self.current_siblings()[idx].label.is_empty() || attempts >= siblings_len {
                self.state.cursor = idx;
                break;
            }
        }
    }

    pub fn press(&mut self) -> Option<NativeMenuAction> {
        if self.current_siblings().is_empty() {
            return None;
        }
        let current = self.current_item().clone();
        match current.value {
            NativeMenuValue::Group => {
                self.state.stack.push(self.state.cursor);
                self.state.cursor = 0;
                None
            }
            NativeMenuValue::Action(action) => Some(action),
            NativeMenuValue::Enum { .. }
            | NativeMenuValue::Number { .. }
            | NativeMenuValue::Bool { .. } => {
                self.state.editing = !self.state.editing;
                None
            }
            NativeMenuValue::Text {
                value,
                max_len,
                cursor: _,
            } => {
                if self.state.editing {
                    self.advance_text_cursor(max_len);
                } else {
                    let cursor = value.len().min(max_len);
                    if let NativeMenuValue::Text { cursor: target, .. } =
                        &mut self.current_item_mut().value
                    {
                        *target = cursor;
                    }
                    self.state.editing = true;
                }
                None
            }
        }
    }

    pub fn back(&mut self) {
        if self.state.editing {
            self.state.editing = false;
            return;
        }
        if let Some(cursor) = self.state.stack.pop() {
            self.state.cursor = cursor;
        }
    }

    pub fn delete_text_char(&mut self) -> bool {
        if !self.state.editing {
            return false;
        }
        let NativeMenuValue::Text { value, cursor, .. } = &mut self.current_item_mut().value else {
            return false;
        };
        let cursor_pos = (*cursor).min(value.len());
        if cursor_pos == 0 {
            return true;
        }
        value.remove(cursor_pos - 1);
        *cursor = cursor_pos - 1;
        true
    }

    fn advance_text_cursor(&mut self, max_len: usize) {
        if let NativeMenuValue::Text { value, cursor, .. } = &mut self.current_item_mut().value {
            *cursor = (*cursor + 1)
                .min(max_len)
                .min(value.len().saturating_add(1));
        }
    }

    pub fn snapshot(&self) -> NativeMenuSnapshot {
        let siblings = self.current_siblings();
        let mut lines = Vec::new();
        let mut colors = Vec::new();
        let mut bar_values = Vec::new();
        let section_color = section_color_from_path(&self.path_label());
        let root_level = self.state.stack.is_empty();
        let mut selected_row = None;
        let body_rows = MENU_BODY_ROWS;
        if siblings.is_empty() {
            return NativeMenuSnapshot {
                path: self.path_label(),
                lines: vec!["(empty)".into()],
                colors: vec![section_color],
                bar_values: vec![None],
                selected_row: Some(0),
                selected_action: None,
            };
        }
        let cursor = self.state.cursor.min(siblings.len().saturating_sub(1));
        let mut start = cursor;
        let mut end = cursor + 1;
        let mut row_count = format_item_lines(
            &siblings[cursor],
            true,
            self.state.editing,
            &self.numeric_display_mode,
        )
        .len();
        while row_count < body_rows && (start > 0 || end < siblings.len()) {
            let mut grew = false;
            if start > 0 {
                let prev_rows = format_item_lines(
                    &siblings[start - 1],
                    false,
                    false,
                    &self.numeric_display_mode,
                )
                .len();
                if row_count + prev_rows <= body_rows || end >= siblings.len() {
                    start -= 1;
                    row_count += prev_rows;
                    grew = true;
                }
            }
            if row_count >= body_rows {
                break;
            }
            if end < siblings.len() {
                let next_rows =
                    format_item_lines(&siblings[end], false, false, &self.numeric_display_mode)
                        .len();
                if row_count + next_rows <= body_rows || start == 0 {
                    end += 1;
                    row_count += next_rows;
                    grew = true;
                }
            }
            if !grew {
                break;
            }
        }
        for (index, item) in siblings.iter().enumerate().skip(start).take(end - start) {
            let selected = index == self.state.cursor;
            if selected {
                selected_row = Some(lines.len());
            }
            let item_lines = format_item_lines(
                item,
                selected,
                selected && self.state.editing,
                &self.numeric_display_mode,
            );
            let item_line_count = item_lines.len();
            lines.extend(item_lines);
            bar_values.extend(format_item_bar_values(
                item,
                item_line_count,
                &self.numeric_display_mode,
            ));
            colors.push(if root_level {
                section_color_for_label(&item.label)
            } else {
                section_color
            });
            for _ in 1..item_line_count {
                colors.push(if root_level {
                    section_color_for_label(&item.label)
                } else {
                    section_color
                });
            }
        }
        lines.truncate(body_rows);
        colors.truncate(body_rows);
        bar_values.truncate(body_rows);
        if lines.is_empty() {
            lines.push("(empty)".into());
            colors.push(section_color);
            bar_values.push(None);
        }
        NativeMenuSnapshot {
            path: self.path_label(),
            lines,
            colors,
            bar_values,
            selected_row,
            selected_action: match &self.current_item().value {
                NativeMenuValue::Action(action) => Some(action.clone()),
                _ => None,
            },
        }
    }

    pub fn selected_behavior(&self) -> Option<String> {
        self.find_value("Behavior")
    }

    pub fn selected_algorithm_step_pulses(&self) -> Option<u32> {
        self.find_value("Step Rate")
            .and_then(|value| note_unit_to_pulses(&value))
    }

    pub fn selected_sync_source(&self) -> Option<SyncSource> {
        match self
            .find_key_value("midiSyncMode")
            .or_else(|| self.find_value("Sync"))?
            .as_str()
        {
            "external" => Some(SyncSource::External),
            _ => Some(SyncSource::Internal),
        }
    }

    pub fn selected_master_volume(&self) -> Option<u8> {
        self.find_number("Master Vol")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn value_for_key(&self, key: &str) -> Option<String> {
        self.find_key_value(key)
    }

    pub fn number_for_key(&self, key: &str) -> Option<i32> {
        self.find_key_number(key)
    }

    pub fn selected_display_brightness(&self) -> Option<u8> {
        self.find_number("Display Brightness")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_button_brightness(&self) -> Option<u8> {
        self.find_number("Button Brightness")
            .map(|value| value.clamp(0, 100) as u8)
    }

    pub fn selected_dance_mode(&self) -> Option<String> {
        self.find_key_value("danceMode")
    }

    pub fn current_label(&self) -> Option<&str> {
        let siblings = self.current_siblings();
        if siblings.is_empty() {
            return None;
        }
        Some(
            siblings[self.state.cursor.min(siblings.len().saturating_sub(1))]
                .label
                .as_str(),
        )
    }

    pub fn current_key(&self) -> Option<&str> {
        let siblings = self.current_siblings();
        siblings
            .get(self.state.cursor.min(siblings.len().saturating_sub(1)))
            .and_then(|item| item.key.as_deref())
    }

    pub fn current_help_target(&self) -> Option<NativeMenuHelpTarget> {
        let siblings = self.current_siblings();
        let item = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))?;
        Some(menu_help_target(&self.current_item_path(), item))
    }

    #[cfg(test)]
    pub fn help_targets(&self) -> Vec<NativeMenuHelpTarget> {
        let mut targets = Vec::new();
        collect_help_targets(&self.root, "Menu".into(), &mut targets);
        targets
    }

    pub fn current_binding_target(&self) -> (Option<String>, Option<NativeMenuAction>) {
        let siblings = self.current_siblings();
        let Some(item) = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))
        else {
            return (None, None);
        };
        match (&item.key, &item.value) {
            (Some(key), NativeMenuValue::Enum { .. })
            | (Some(key), NativeMenuValue::Number { .. })
            | (Some(key), NativeMenuValue::Bool { .. }) => (Some(key.clone()), None),
            (Some(_), NativeMenuValue::Text { .. }) => (None, None),
            (_, NativeMenuValue::Action(action)) => (None, Some(action.clone())),
            _ => (None, None),
        }
    }

    pub fn current_param_binding(&self) -> Option<NativeParamBindingSpec> {
        let siblings = self.current_siblings();
        let item = siblings.get(self.state.cursor.min(siblings.len().saturating_sub(1)))?;
        let key = item.key.clone()?;
        param_binding_from_item_key(item, key)
    }

    pub fn turn_key(&mut self, key: &str, delta: i8) -> bool {
        turn_key_in_item(&mut self.root, key, delta)
    }

    fn path_label(&self) -> String {
        let mut node = &self.root;
        let mut labels = Vec::new();
        for idx in &self.state.stack {
            let children = &node.children;
            if let Some(next) = children.get(*idx) {
                labels.push(next.label.clone());
                node = next;
            }
        }
        if labels.is_empty() {
            "MENU".into()
        } else {
            abbreviate_path(&labels.join("/"))
        }
    }

    fn current_item_path(&self) -> String {
        let mut node = &self.root;
        let mut labels = vec!["Menu".to_string()];
        for idx in &self.state.stack {
            let children = &node.children;
            if let Some(next) = children.get(*idx) {
                labels.push(next.label.clone());
                node = next;
            }
        }
        labels.push(self.current_item().label.clone());
        canonicalize_help_path(&labels.join(" > "))
    }

    fn current_siblings(&self) -> &Vec<NativeMenuItem> {
        &self.current_node().children
    }

    fn current_node(&self) -> &NativeMenuItem {
        let mut node = &self.root;
        for idx in &self.state.stack {
            node = &node.children[*idx.min(&node.children.len().saturating_sub(1))];
        }
        node
    }

    fn current_item(&self) -> &NativeMenuItem {
        let siblings = self.current_siblings();
        &siblings[self.state.cursor.min(siblings.len().saturating_sub(1))]
    }

    fn current_item_mut(&mut self) -> &mut NativeMenuItem {
        let mut node = &mut self.root;
        for idx in self.state.stack.clone() {
            let bounded = idx.min(node.children.len().saturating_sub(1));
            node = &mut node.children[bounded];
        }
        let idx = self.state.cursor.min(node.children.len().saturating_sub(1));
        &mut node.children[idx]
    }

    fn find_value(&self, label: &str) -> Option<String> {
        find_item(&self.root, label).and_then(|item| match &item.value {
            NativeMenuValue::Enum { options, selected } => options.get(*selected).cloned(),
            NativeMenuValue::Bool { value } => Some(if *value {
                "true".into()
            } else {
                "false".into()
            }),
            NativeMenuValue::Text { value, .. } => Some(value.clone()),
            _ => None,
        })
    }

    fn find_number(&self, label: &str) -> Option<i32> {
        find_item(&self.root, label).and_then(|item| match &item.value {
            NativeMenuValue::Number { value, .. } => Some(*value),
            _ => None,
        })
    }

    fn find_key_value(&self, key: &str) -> Option<String> {
        find_item_by_key(&self.root, key).and_then(|item| match &item.value {
            NativeMenuValue::Enum { options, selected } => options.get(*selected).cloned(),
            NativeMenuValue::Bool { value } => Some(if *value {
                "true".into()
            } else {
                "false".into()
            }),
            NativeMenuValue::Text { value, .. } => Some(value.clone()),
            _ => None,
        })
    }

    fn find_key_number(&self, key: &str) -> Option<i32> {
        find_item_by_key(&self.root, key).and_then(|item| match &item.value {
            NativeMenuValue::Number { value, .. } => Some(*value),
            _ => None,
        })
    }
}

fn find_item<'a>(node: &'a NativeMenuItem, label: &str) -> Option<&'a NativeMenuItem> {
    if node.label == label {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_item(child, label) {
            return Some(found);
        }
    }
    None
}

fn find_item_by_key<'a>(node: &'a NativeMenuItem, key: &str) -> Option<&'a NativeMenuItem> {
    if node.key.as_deref() == Some(key) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_item_by_key(child, key) {
            return Some(found);
        }
    }
    None
}

fn param_binding_from_item_key(
    item: &NativeMenuItem,
    key: String,
) -> Option<NativeParamBindingSpec> {
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key,
            label: Some(item.label.clone()),
            kind: "bool".into(),
            min: None,
            max: None,
            step: None,
            options: vec![],
            invert: false,
        }),
        _ => None,
    }
}

#[cfg(test)]
fn collect_help_targets(
    item: &NativeMenuItem,
    path: String,
    targets: &mut Vec<NativeMenuHelpTarget>,
) {
    for child in &item.children {
        if child.label.is_empty() {
            continue;
        }
        let child_path = canonicalize_help_path(&format!("{path} > {}", child.label));
        targets.push(menu_help_target(&child_path, child));
        if !child.children.is_empty() {
            collect_help_targets(child, child_path, targets);
        }
    }
}

fn menu_help_target(path: &str, item: &NativeMenuItem) -> NativeMenuHelpTarget {
    let (key, kind) = match &item.value {
        NativeMenuValue::Group => (String::new(), "group"),
        NativeMenuValue::Enum { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "enum",
        ),
        NativeMenuValue::Number { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "number",
        ),
        NativeMenuValue::Bool { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "bool",
        ),
        NativeMenuValue::Text { .. } => (
            item.key
                .as_ref()
                .map(|key| format!("key:{}", canonicalize_help_key(key)))
                .unwrap_or_default(),
            "text",
        ),
        NativeMenuValue::Action(action) => (menu_action_help_key(action), "action"),
    };
    NativeMenuHelpTarget {
        path: path.to_string(),
        key,
        kind: kind.into(),
        label: item.label.clone(),
    }
}

fn canonicalize_help_path(path: &str) -> String {
    let parts = path
        .split(" > ")
        .map(|part| {
            if part.starts_with('P') && part.contains(':') {
                "P*: *".into()
            } else if part.starts_with('I') && part.contains(':') {
                if part == path.rsplit(" > ").next().unwrap_or(part) {
                    let number = part
                        .chars()
                        .skip(1)
                        .take_while(|ch| ch.is_ascii_digit())
                        .collect::<String>();
                    format!("Instrument {number}")
                } else {
                    "Instrument *".into()
                }
            } else if part.starts_with('B') && part.contains(':') {
                "B*: *".into()
            } else {
                part.to_string()
            }
        })
        .collect::<Vec<_>>();
    parts.join(" > ")
}

fn canonicalize_help_key(key: &str) -> String {
    let parts = key.split('.').collect::<Vec<_>>();
    parts
        .iter()
        .enumerate()
        .map(|(index, part)| {
            if part.chars().all(|ch| ch.is_ascii_digit())
                && !(index >= 4
                    && parts.get(index - 2) == Some(&"paramMods")
                    && matches!(parts.get(index - 1), Some(&"x") | Some(&"y")))
            {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn menu_action_help_key(action: &NativeMenuAction) -> String {
    match action {
        NativeMenuAction::ResetBehavior => "action:reset_behavior".into(),
        NativeMenuAction::BehaviorAction(action_type) => {
            format!("action:behavior_action:{action_type}")
        }
        NativeMenuAction::SetParamBinding { .. } => "action:param_bind".into(),
        NativeMenuAction::ClearParamBinding { .. } => "action:param_clear".into(),
        NativeMenuAction::SetAuxClick { .. } => "action:aux_click_set_target".into(),
        NativeMenuAction::CloneInstrument { .. } => "action:instrument_clone".into(),
        NativeMenuAction::ResetInstrument { .. } => "action:instrument_reset".into(),
        NativeMenuAction::PlatformEffect(effect) => match effect.as_str() {
            "preset.saveAs" => "action:preset_save".into(),
            "preset.saveCurrent" => "action:preset_save_current".into(),
            "preset.refresh" => "action:refresh_presets".into(),
            "preset.renameApply" => "action:preset_rename_apply".into(),
            "default.save" => "action:default_save".into(),
            "default.load" => "action:default_load".into(),
            "factory.load" => "action:factory_load".into(),
            "midi.panic" => "action:midi_panic".into(),
            "dance.fx.map" => "action:fx_assign_enter".into(),
            value if value.starts_with("preset.load:") => "action:preset_load:*".into(),
            value if value.starts_with("preset.delete:") => "action:preset_delete:*".into(),
            value if value.starts_with("preset.renamePick:") => {
                "action:preset_rename_pick:*".into()
            }
            value if value.starts_with("midi.out:") || value.starts_with("midi.output:") => {
                if value == "midi.out:" || value == "midi.output:" {
                    "action:midi_select_output:null".into()
                } else {
                    "action:midi_select_output:*".into()
                }
            }
            value if value.starts_with("midi.in:") || value.starts_with("midi.input:") => {
                if value == "midi.in:" || value == "midi.input:" {
                    "action:midi_select_input:null".into()
                } else {
                    "action:midi_select_input:*".into()
                }
            }
            value if value.starts_with("sample.open:") => "action:sample_browser_open".into(),
            value if value.starts_with("sample.up:") => "action:sample_browser_up".into(),
            value if value.starts_with("sample.pick:") => "action:sample_browser_pick".into(),
            value if value.starts_with("sample.preview:") => "action:sample_preview".into(),
            value if value.starts_with("sample.assign:") => "action:sample_assign_enter".into(),
            value if value.starts_with("synth.preset:") => "action:synth_preset_load".into(),
            value if value.starts_with("trigger.probability.assign:") => {
                "action:trigger_probability_assign_enter".into()
            }
            value => format!("action:{value}"),
        },
    }
}

fn turn_key_in_item(item: &mut NativeMenuItem, key: &str, delta: i8) -> bool {
    if item.key.as_deref() == Some(key) {
        match &mut item.value {
            NativeMenuValue::Enum { options, selected } => {
                let max = options.len().saturating_sub(1);
                *selected = ((*selected as isize) + delta as isize).clamp(0, max as isize) as usize;
                return true;
            }
            NativeMenuValue::Number {
                value,
                min,
                max,
                step,
            } => {
                *value = (*value + i32::from(delta) * *step).clamp(*min, *max);
                return true;
            }
            NativeMenuValue::Bool { value } => {
                if delta != 0 {
                    *value = !*value;
                }
                return true;
            }
            NativeMenuValue::Text {
                value,
                max_len,
                cursor,
            } => {
                turn_text_value(value, *max_len, cursor, delta);
                return true;
            }
            _ => {}
        }
    }
    item.children
        .iter_mut()
        .any(|child| turn_key_in_item(child, key, delta))
}

fn turn_text_value(value: &mut String, max_len: usize, cursor: &mut usize, delta: i8) {
    const CHARSET: &str = " ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-";
    if max_len == 0 || delta == 0 {
        return;
    }
    let cursor_pos = (*cursor).min(max_len.saturating_sub(1));
    while value.len() <= cursor_pos {
        value.push(' ');
    }
    value.truncate(max_len);
    let current = value.as_bytes().get(cursor_pos).copied().unwrap_or(b' ') as char;
    let current_index = CHARSET.find(current).unwrap_or(0) as isize;
    let next_index =
        (current_index + isize::from(delta)).rem_euclid(CHARSET.len() as isize) as usize;
    let next = CHARSET.as_bytes()[next_index] as char;
    value.replace_range(cursor_pos..cursor_pos + 1, &next.to_string());
    while value.ends_with(' ') {
        value.pop();
    }
    *cursor = cursor_pos.min(value.len());
}

fn format_item_lines(
    item: &NativeMenuItem,
    selected: bool,
    editing: bool,
    numeric_display_mode: &str,
) -> Vec<String> {
    if item.label.is_empty() {
        return vec![String::new()];
    }
    match &item.value {
        NativeMenuValue::Group => vec![format!("> {}", item.label)],
        NativeMenuValue::Action(_) => vec![format!("!{}", item.label)],
        NativeMenuValue::Enum {
            options,
            selected: current,
        } => format_param_lines(
            &item.label,
            format_display_value(
                item.key.as_deref(),
                options.get(*current).cloned().unwrap_or_default(),
            ),
            selected,
            editing,
        ),
        NativeMenuValue::Number { value, .. } => format_param_lines(
            &item.label,
            if should_use_number_bar(item.key.as_deref().unwrap_or_default())
                && numeric_display_mode == "bar"
            {
                String::new()
            } else {
                format_display_value(item.key.as_deref(), *value)
            },
            selected,
            editing,
        ),
        NativeMenuValue::Bool { value } => format_param_lines(
            &item.label,
            if *value { "On" } else { "Off" },
            selected,
            editing,
        ),
        NativeMenuValue::Text { value, cursor, .. } => {
            format_text_lines(&item.label, value, *cursor, selected, editing)
        }
    }
}

fn format_item_bar_values(
    item: &NativeMenuItem,
    item_line_count: usize,
    numeric_display_mode: &str,
) -> Vec<Option<NativeMenuBarValue>> {
    if numeric_display_mode == "numbers" {
        return vec![None; item_line_count];
    }
    let NativeMenuValue::Number {
        value, min, max, ..
    } = item.value
    else {
        return vec![None; item_line_count];
    };
    let Some(key) = item.key.as_deref() else {
        return vec![None; item_line_count];
    };
    if !should_use_number_bar(key) {
        return vec![None; item_line_count];
    }
    let range = (max - min).max(1);
    let frac_pct = ((((value - min).clamp(0, range) as f32 / range as f32) * 100.0).round()) as u8;
    let bar = Some(NativeMenuBarValue {
        frac_pct,
        num_chars: if numeric_display_mode == "bar" {
            0
        } else {
            bar_number_chars(key, min, max)
        },
        style: if is_marker_bar_key(key) {
            Some("marker".into())
        } else {
            None
        },
    });
    if item_line_count > 1 {
        vec![None, bar]
    } else {
        vec![None]
    }
}

fn should_use_number_bar(key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    if key_lower.ends_with("channel")
        || key_lower.ends_with("selectedslot")
        || key_lower.ends_with("activepartindex")
        || key_lower.ends_with("startingnote")
        || key_lower.ends_with("lowestnote")
        || key_lower.ends_with("highestnote")
    {
        return false;
    }
    key == "masterVolume"
        || key == "transport.bpm"
        || key == "screenSleepSeconds"
        || key.contains(".params.")
        || key_lower.ends_with("pct")
        || key_lower.ends_with("percent")
        || key_lower.ends_with("ms")
        || key_lower.ends_with("hz")
        || key_lower.ends_with("db")
        || key_lower.ends_with("semis")
        || key_lower.ends_with("semitones")
        || key_lower.ends_with("cents")
        || key_lower.ends_with("panpos")
        || key_lower.ends_with("volume")
        || key_lower.ends_with("basevelocity")
        || key_lower.ends_with("velocity")
        || key_lower.ends_with("high")
        || key_lower.ends_with("medium")
        || key_lower.ends_with("low")
        || key_lower.ends_with("durationms")
        || key_lower.ends_with("gainpct")
        || key_lower.ends_with("velocitysensitivitypct")
        || key_lower.ends_with("levelpct")
        || key_lower.ends_with("pulsewidthpct")
        || key_lower.ends_with("detunecents")
        || key_lower.ends_with("cutoffhz")
        || key_lower.ends_with("resonance")
        || key_lower.ends_with("envamountpct")
        || key_lower.ends_with("keytrackingpct")
        || key_lower.ends_with("attackms")
        || key_lower.ends_with("decayms")
        || key_lower.ends_with("sustainpct")
        || key_lower.ends_with("releasems")
        || key_lower.ends_with("notelengthms")
        || key_lower.ends_with("velocityscalepct")
        || key_lower.ends_with("steps")
        || key_lower.ends_with("from")
        || key_lower.ends_with("to")
        || key_lower.ends_with("gridoffset")
        || key_lower.ends_with("randomcellspertick")
        || key_lower.ends_with("randomtickinterval")
        || key_lower.ends_with("spawnstep")
        || key_lower.ends_with("seedinterval")
        || key_lower.ends_with("randomseedcells")
        || key_lower.ends_with("firethreshold")
        || key_lower.ends_with("maxants")
        || key_lower.ends_with("autospawninterval")
        || key_lower.ends_with("spawninterval")
        || key_lower.ends_with("maxballs")
        || key_lower.ends_with("lifespan")
        || key_lower.ends_with("maxradius")
        || key_lower.ends_with("autopulseinterval")
        || key_lower.ends_with("autodropinterval")
        || key_lower.ends_with("splashradius")
}

fn bar_number_chars(key: &str, min: i32, max: i32) -> usize {
    [min, (min + max) / 2, max]
        .into_iter()
        .map(|value| format_display_value(Some(key), value).len())
        .max()
        .unwrap_or(0)
}

fn format_display_value(key: Option<&str>, value: impl ToString) -> String {
    let raw = value.to_string();
    let Some(key) = key else {
        return raw;
    };
    if key.ends_with("panPos") {
        return format_pan_position(raw.parse::<i32>().unwrap_or(16));
    }
    if key.ends_with("pitch.lowestNote")
        || key.ends_with("pitch.highestNote")
        || key.ends_with("pitch.startingNote")
    {
        return format_note_with_midi(raw.parse::<i32>().unwrap_or(60));
    }
    if key.ends_with("pitch.scale") {
        return format_scale_name(&raw);
    }
    if key.contains(".params.") {
        return format_fx_param_display(key, raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Pct") || key.ends_with("Percent") {
        return format!("{}%", raw.parse::<i32>().unwrap_or(0));
    }
    if key.ends_with("Ms") {
        let ms = raw.parse::<i32>().unwrap_or(0);
        if ms.abs() >= 1000 {
            return format!("{:.1}s", ms as f32 / 1000.0);
        }
        return format!("{ms}ms");
    }
    raw
}

fn format_fx_param_display(key: &str, value: i32) -> String {
    if key.ends_with(".decay") {
        return format_reverb_decay_seconds(value as f64 / 1000.0);
    }
    if key.ends_with("Hz") {
        return format_fixed_unit(value as f64 / 100.0, "Hz");
    }
    if key.ends_with("Db") || key.ends_with("GainDb") || key.ends_with("thresholdDb") {
        return format!("{:+.1}dB", value as f64 / 2.0);
    }
    if key.ends_with("feedback")
        || key.ends_with("threshold")
        || key.ends_with("clip")
        || key.ends_with("q")
        || key.ends_with("midQ")
    {
        return format_fixed(value as f64 / 100.0, 2);
    }
    if key.ends_with("drive") || key.ends_with("depthMs") || key.ends_with("baseMs") {
        return format_fixed(value as f64 / 10.0, 1);
    }
    if key.ends_with("ratio") {
        return format_fixed(value as f64 / 2.0, 1);
    }
    if key.ends_with("Pct") {
        return format!("{value}%");
    }
    if key.ends_with("Ms") {
        if value.abs() >= 1000 {
            return format!("{:.1}s", value as f32 / 1000.0);
        }
        return format!("{value}ms");
    }
    value.to_string()
}

fn format_fixed_unit(value: f64, unit: &str) -> String {
    format!("{}{unit}", format_fixed(value, 2))
}

fn format_fixed(value: f64, digits: usize) -> String {
    let text = format!("{value:.digits$}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn format_reverb_decay_seconds(value: f64) -> String {
    let feedback = value.clamp(0.0, 0.995);
    if feedback <= 0.0 {
        return "0.0s".into();
    }
    let average_delay_seconds = ((1557.0 + 1617.0 + 1491.0 + 1422.0) / 4.0) / 44_100.0;
    let seconds = (-3.0 * average_delay_seconds) / feedback.log10();
    format!("{seconds:.1}s")
}

fn is_marker_bar_key(key: &str) -> bool {
    let key_lower = key.to_ascii_lowercase();
    key_lower.ends_with("panpos")
        || key_lower.ends_with("semis")
        || key_lower.ends_with("semitones")
        || key_lower.ends_with("cents")
        || key_lower.ends_with("detunecents")
        || key_lower.ends_with("envamountpct")
}

fn format_scale_name(value: &str) -> String {
    match value {
        "chromatic" => "Chromatic",
        "major" => "Major",
        "natural_minor" => "Natural Minor",
        "dorian" => "Dorian",
        "mixolydian" => "Mixolydian",
        "major_pentatonic" => "Maj Pentatonic",
        "minor_pentatonic" => "Min Pentatonic",
        "harmonic_minor" => "Harm Minor",
        _ => value,
    }
    .into()
}

fn format_note_with_midi(note: i32) -> String {
    let note = note.clamp(0, 127);
    let names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let name = names[(note % 12) as usize];
    let octave = note / 12 - 1;
    format!("{name}{octave} ({note})")
}

fn format_pan_position(value: i32) -> String {
    let pos = value.clamp(0, 32);
    let distance = pos - 16;
    if distance == 0 {
        "C".into()
    } else if distance < 0 {
        format!("L{}", distance.abs().min(15))
    } else {
        format!("R{}", distance.min(15))
    }
}

fn format_param_lines(
    label: &str,
    value: impl Into<String>,
    selected: bool,
    editing: bool,
) -> Vec<String> {
    if selected {
        vec![
            format!("  {label}:"),
            format!(" {}{}", if editing { "*" } else { " " }, value.into()),
        ]
    } else {
        vec![format!("  {label}")]
    }
}

fn format_text_lines(
    label: &str,
    value: &str,
    _cursor: usize,
    selected: bool,
    editing: bool,
) -> Vec<String> {
    let display = if value.is_empty() { "(empty)" } else { value };
    if selected {
        let marker = if editing { "*" } else { " " };
        vec![
            format!("  {label}:"),
            format!(" {marker}{}", clip_menu_value(display, 22)),
        ]
    } else {
        vec![format!("  {label}")]
    }
}

fn clip_menu_value(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.into();
    }
    if width <= 3 {
        return value.chars().take(width).collect();
    }
    format!("{}...", value.chars().take(width - 3).collect::<String>())
}

fn abbreviate_path(path: &str) -> String {
    let mut out = Vec::new();
    for segment in path.split('/') {
        out.push(match segment {
            "Menu" => "MENU".to_string(),
            "System" => "SYS".to_string(),
            other => other.to_string(),
        });
    }
    out.join("/")
}

fn section_color_from_path(path: &str) -> u16 {
    let first = path.split('/').next().unwrap_or("MENU");
    section_color_for_label(first)
}

fn section_color_for_label(label: &str) -> u16 {
    if label.starts_with("L1:") || label == "L1: Life" {
        return 0x8ED1;
    }
    if label.starts_with("L2:") || label == "L2: Sense" {
        return 0x8D5C;
    }
    if label.starts_with("L3:") || label == "L3: Voice" {
        return 0xC59B;
    }
    if label.starts_with("L4:") || label == "L4: Dance" {
        return 0xFFFF;
    }
    if label == "System" || label == "SYS" || label == "MENU" {
        return 0xB50D;
    }
    0xFFFF
}

fn note_unit_to_pulses(value: &str) -> Option<u32> {
    match value {
        "1/16" => Some(6),
        "1/8" => Some(12),
        "1/4" => Some(24),
        "1/2" => Some(48),
        "1/1" => Some(96),
        _ => None,
    }
}

fn build_root(config: NativeMenuConfig) -> NativeMenuItem {
    let sync_index = if config.sync_source == SyncSource::External {
        1
    } else {
        0
    };
    let instrument_options = config.instrument_labels.to_vec();
    NativeMenuItem {
        label: "Menu".into(),
        key: None,
        value: NativeMenuValue::Group,
        children: vec![
            NativeMenuItem {
                label: "L1: Life".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: config
                    .part_labels
                    .iter()
                    .map(|label| NativeMenuItem {
                        label: label.clone(),
                        key: None,
                        value: NativeMenuValue::Group,
                        children: config.l1_items.clone(),
                    })
                    .collect(),
            },
            NativeMenuItem {
                label: "L2: Sense".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: std::iter::once(aux_mappings_group(&config))
                    .chain(config.part_labels.iter().enumerate().map(|(index, label)| {
                        l2_part_group(
                            index,
                            label.clone(),
                            &instrument_options,
                            config.sense_parts.get(index),
                            &config,
                        )
                    }))
                    .collect(),
            },
            NativeMenuItem {
                label: "L3: Voice".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: vec![
                    NativeMenuItem {
                        label: "Instruments".into(),
                        key: None,
                        value: NativeMenuValue::Group,
                        children: config
                            .instrument_labels
                            .iter()
                            .enumerate()
                            .map(|(index, label)| {
                                let kind = config
                                    .instrument_types
                                    .get(index)
                                    .map(String::as_str)
                                    .unwrap_or("synth");
                                instrument_group(InstrumentMenuConfig {
                                    index,
                                    label: label.clone(),
                                    name: config
                                        .instrument_names
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or(kind),
                                    kind,
                                    auto_name: config
                                        .instrument_auto_names
                                        .get(index)
                                        .copied()
                                        .unwrap_or(true),
                                    note_behavior: config
                                        .instrument_note_behaviors
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or("oneshot"),
                                    route: config
                                        .instrument_routes
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or("direct"),
                                    volume: config
                                        .instrument_volumes
                                        .get(index)
                                        .copied()
                                        .unwrap_or(100),
                                    pan_pos: config
                                        .instrument_pan_positions
                                        .get(index)
                                        .copied()
                                        .unwrap_or(16),
                                    sample_slot: config
                                        .instrument_sample_slots
                                        .get(index)
                                        .copied()
                                        .unwrap_or(0),
                                    synth_config: config.instrument_synth_configs.get(index),
                                    synth_osc1_waveform: config
                                        .instrument_synth_osc1_waveforms
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or("saw"),
                                    synth_osc2_waveform: config
                                        .instrument_synth_osc2_waveforms
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or("square"),
                                    synth_filter_type: config
                                        .instrument_synth_filter_types
                                        .get(index)
                                        .map(String::as_str)
                                        .unwrap_or("lowpass"),
                                    synth_filter_cutoff: config
                                        .instrument_synth_filter_cutoffs
                                        .get(index)
                                        .copied()
                                        .unwrap_or(8000),
                                    synth_gain_pct: config
                                        .instrument_synth_gain_pct
                                        .get(index)
                                        .copied()
                                        .unwrap_or(80),
                                    synth_filter_resonance: config
                                        .instrument_synth_filter_resonance
                                        .get(index)
                                        .copied()
                                        .unwrap_or(20),
                                    sample_tune_semis: config
                                        .instrument_sample_tune_semis
                                        .get(index)
                                        .copied()
                                        .unwrap_or(0),
                                    sample_gain_pct: config
                                        .instrument_sample_gain_pct
                                        .get(index)
                                        .copied()
                                        .unwrap_or(100),
                                    sample_base_velocity: config
                                        .instrument_sample_base_velocity
                                        .get(index)
                                        .copied()
                                        .unwrap_or(100),
                                    sample_amp_velocity_sensitivity_pct: config
                                        .instrument_sample_amp_velocity_sensitivity_pct
                                        .get(index)
                                        .copied()
                                        .unwrap_or(100),
                                    sample_velocity_levels_enabled: config
                                        .instrument_sample_velocity_levels_enabled
                                        .get(index)
                                        .copied()
                                        .unwrap_or(false),
                                    sample_velocity_high: config
                                        .instrument_sample_velocity_high
                                        .get(index)
                                        .copied()
                                        .unwrap_or(120),
                                    sample_velocity_medium: config
                                        .instrument_sample_velocity_medium
                                        .get(index)
                                        .copied()
                                        .unwrap_or(85),
                                    sample_velocity_low: config
                                        .instrument_sample_velocity_low
                                        .get(index)
                                        .copied()
                                        .unwrap_or(45),
                                    sample_amp_env: config.instrument_sample_amp_envs.get(index),
                                    sample_filter: config.instrument_sample_filters.get(index),
                                    sample_filter_env: config
                                        .instrument_sample_filter_envs
                                        .get(index),
                                    midi_enabled: config
                                        .instrument_midi_enabled
                                        .get(index)
                                        .copied()
                                        .unwrap_or(false),
                                    midi_channel: config
                                        .instrument_midi_channels
                                        .get(index)
                                        .copied()
                                        .unwrap_or(1),
                                    midi_velocity: config
                                        .instrument_midi_velocity
                                        .get(index)
                                        .copied()
                                        .unwrap_or(100),
                                    midi_duration_ms: config
                                        .instrument_midi_duration_ms
                                        .get(index)
                                        .copied()
                                        .unwrap_or(120),
                                    sample_browser: config.sample_browser.as_ref(),
                                })
                            })
                            .collect(),
                    },
                    fx_buses_group(&config.fx_buses),
                    global_fx_group(&config.global_fx_slots, &config.global_fx_params),
                ],
            },
            dance_group(&config),
            NativeMenuItem {
                label: "".into(),
                key: None,
                value: NativeMenuValue::Group,
                children: vec![],
            },
            system_group(&config, sync_index),
        ],
    }
}

fn group(label: impl Into<String>, children: Vec<NativeMenuItem>) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: None,
        value: NativeMenuValue::Group,
        children,
    }
}

fn enum_item(
    label: impl Into<String>,
    key: impl Into<String>,
    options: Vec<&str>,
    selected: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Enum {
            options: options.into_iter().map(String::from).collect(),
            selected,
        },
        children: vec![],
    }
}

fn number_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: i32,
    min: i32,
    max: i32,
    step: i32,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Number {
            value,
            min,
            max,
            step,
        },
        children: vec![],
    }
}

fn bool_item(label: impl Into<String>, key: impl Into<String>, value: bool) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Bool { value },
        children: vec![],
    }
}

fn text_item(
    label: impl Into<String>,
    key: impl Into<String>,
    value: impl Into<String>,
    max_len: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Text {
            value: value.into(),
            max_len,
            cursor: 0,
        },
        children: vec![],
    }
}

fn action_item(
    label: impl Into<String>,
    key: impl Into<String>,
    action: NativeMenuAction,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Action(action),
        children: vec![],
    }
}

fn dance_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = vec![
        NativeMenuItem {
            label: "Dance Page".into(),
            key: Some("danceMode".into()),
            value: NativeMenuValue::Enum {
                options: vec![
                    "none".into(),
                    "mix".into(),
                    "pan".into(),
                    "fx".into(),
                    "trigger-gate".into(),
                    "xy".into(),
                ],
                selected: ["none", "mix", "pan", "fx", "trigger-gate", "xy"]
                    .iter()
                    .position(|mode| *mode == config.dance_mode)
                    .unwrap_or(0),
            },
            children: vec![],
        },
        number_item("BPM", "transport.bpm", i32::from(config.bpm), 40, 240, 1),
    ];
    match config.dance_mode.as_str() {
        "fx" => children.extend(dance_fx_page_items(config)),
        "trigger-gate" => children.push(group("Mode Grid", vec![])),
        "xy" => children.extend(xy_pad_items(config)),
        _ => {}
    }
    group("L4: Dance", children)
}

fn dance_fx_page_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let fx_types = vec!["none", "stutter", "freeze", "filter_sweep", "pitch_shift"];
    let targets = dance_fx_targets();
    let mut children = vec![
        enum_item(
            "FX Type",
            "dance.fx.type",
            fx_types.clone(),
            selected_index(&fx_types, &config.dance_fx_type),
        ),
        enum_item_from_strings(
            "Target",
            "dance.fx.target",
            targets.clone(),
            targets
                .iter()
                .position(|target| target == &config.dance_fx_target)
                .unwrap_or(0),
        ),
    ];
    match config.dance_fx_type.as_str() {
        "stutter" => {
            children.push(number_item(
                "Rate Hz",
                "dance.fx.params.rateHz",
                number_param(&config.dance_fx_params, "rateHz", 8),
                1,
                32,
                1,
            ));
            children.push(number_item(
                "Depth",
                "dance.fx.params.depthPct",
                number_param(&config.dance_fx_params, "depthPct", 100),
                0,
                100,
                1,
            ));
        }
        "freeze" => {
            children.push(number_item(
                "Release Ms",
                "dance.fx.params.releaseMs",
                number_param(&config.dance_fx_params, "releaseMs", 500),
                10,
                5000,
                10,
            ));
            children.push(number_item(
                "Mix",
                "dance.fx.params.mixPct",
                number_param(&config.dance_fx_params, "mixPct", 100),
                0,
                100,
                1,
            ));
        }
        "filter_sweep" => {
            children.push(number_item(
                "Cutoff",
                "dance.fx.params.cutoffPct",
                number_param(&config.dance_fx_params, "cutoffPct", 50),
                0,
                100,
                1,
            ));
            children.push(number_item(
                "Res",
                "dance.fx.params.resonancePct",
                number_param(&config.dance_fx_params, "resonancePct", 0),
                0,
                100,
                1,
            ));
            children.push(number_item(
                "Sweep In",
                "dance.fx.params.sweepInMs",
                number_param(&config.dance_fx_params, "sweepInMs", 120),
                10,
                3000,
                10,
            ));
            children.push(number_item(
                "Sweep Out",
                "dance.fx.params.sweepOutMs",
                number_param(&config.dance_fx_params, "sweepOutMs", 180),
                10,
                3000,
                10,
            ));
        }
        "pitch_shift" => {
            children.push(number_item(
                "Semitones",
                "dance.fx.params.semitones",
                number_param(&config.dance_fx_params, "semitones", 0),
                -24,
                24,
                1,
            ));
            children.push(number_item(
                "Cents",
                "dance.fx.params.cents",
                number_param(&config.dance_fx_params, "cents", 0),
                -100,
                100,
                1,
            ));
            children.push(number_item(
                "Mix",
                "dance.fx.params.mixPct",
                number_param(&config.dance_fx_params, "mixPct", 100),
                0,
                100,
                1,
            ));
        }
        _ => {}
    }
    children.push(action_item(
        "Map to Grid",
        "dance.fx.map",
        NativeMenuAction::PlatformEffect("dance.fx.map".into()),
    ));
    children
}

fn number_param(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: i32,
) -> i32 {
    params
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .map(|value| value as i32)
        .unwrap_or(default)
}

fn dance_fx_targets() -> Vec<String> {
    let mut targets = vec!["master".to_string()];
    targets.extend((1..=FX_BUS_COUNT).map(|index| format!("fx_bus_{index}")));
    targets.extend((1..=8).map(|index| format!("instrument_{index}")));
    targets
}

fn axis_binding_label(label: &str, binding: Option<&NativeParamBindingSpec>) -> String {
    binding
        .and_then(|binding| binding.label.as_deref().or(Some(binding.key.as_str())))
        .map(|binding_label| format!("{label}: {binding_label}"))
        .unwrap_or_else(|| format!("{label}: (none)"))
}

fn parameter_picker_group(
    label: String,
    target: String,
    current: Option<&NativeParamBindingSpec>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "(none)",
        format!("{target}.none"),
        NativeMenuAction::ClearParamBinding {
            target: target.clone(),
        },
    )];
    children.extend(parameter_tree_groups(&target, config));
    if let Some(binding) = current {
        children.insert(
            1,
            action_item(
                format!(
                    "Current: {}",
                    binding.label.as_deref().unwrap_or(&binding.key)
                ),
                format!("{target}.current"),
                NativeMenuAction::SetParamBinding {
                    target: target.clone(),
                    binding: binding.clone(),
                },
            ),
        );
    }
    group(label, children)
}

fn parameter_tree_groups(target: &str, config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    let mut groups = vec![group(
        "Sound",
        vec![
            binding_action(
                "Note Length",
                "sound.noteLengthMs",
                "number",
                Some(30),
                Some(2000),
                Some(10),
                vec![],
                target,
            ),
            binding_action(
                "Velocity Scale",
                "sound.velocityScalePct",
                "number",
                Some(0),
                Some(200),
                Some(1),
                vec![],
                target,
            ),
            binding_action(
                "Voice Stealing",
                "sound.voiceStealingMode",
                "enum",
                None,
                None,
                None,
                vec!["off", "lenient", "balanced", "aggressive"],
                target,
            ),
        ],
    )];

    let behavior_params = config
        .l1_items
        .iter()
        .filter_map(|item| {
            binding_from_menu_item(
                item,
                &format!("parts.{}.l1.behaviorConfig", config.active_part_index),
            )
        })
        .map(|binding| binding_action_from_spec(binding, target))
        .collect::<Vec<_>>();
    if !behavior_params.is_empty() {
        groups.push(group("Behavior", behavior_params));
    }

    let instrument_groups = config
        .instrument_labels
        .iter()
        .enumerate()
        .map(|(index, label)| {
            group(
                label.clone(),
                vec![
                    group(
                        "Mixer",
                        vec![
                            binding_action(
                                "Volume",
                                &format!("instruments.{index}.mixer.volume"),
                                "number",
                                Some(0),
                                Some(127),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Pan",
                                &format!("instruments.{index}.mixer.panPos"),
                                "number",
                                Some(0),
                                Some(32),
                                Some(1),
                                vec![],
                                target,
                            ),
                        ],
                    ),
                    group(
                        "Synth",
                        vec![binding_action(
                            "Gain",
                            &format!("instruments.{index}.synth.amp.gainPct"),
                            "number",
                            Some(0),
                            Some(100),
                            Some(1),
                            vec![],
                            target,
                        )],
                    ),
                    group(
                        "Sample",
                        vec![
                            binding_action(
                                "Base Velocity",
                                &format!("instruments.{index}.sample.baseVelocity"),
                                "number",
                                Some(1),
                                Some(127),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Tune",
                                &format!("instruments.{index}.sample.tuneSemis"),
                                "number",
                                Some(-24),
                                Some(24),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Gain",
                                &format!("instruments.{index}.sample.amp.gainPct"),
                                "number",
                                Some(0),
                                Some(100),
                                Some(1),
                                vec![],
                                target,
                            ),
                        ],
                    ),
                    group(
                        "MIDI",
                        vec![
                            binding_action(
                                "Enabled",
                                &format!("instruments.{index}.midi.enabled"),
                                "bool",
                                None,
                                None,
                                None,
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Velocity",
                                &format!("instruments.{index}.midi.velocity"),
                                "number",
                                Some(1),
                                Some(127),
                                Some(1),
                                vec![],
                                target,
                            ),
                            binding_action(
                                "Duration",
                                &format!("instruments.{index}.midi.durationMs"),
                                "number",
                                Some(10),
                                Some(5000),
                                Some(10),
                                vec![],
                                target,
                            ),
                        ],
                    ),
                ],
            )
        })
        .collect::<Vec<_>>();
    groups.push(group("Instruments", instrument_groups));
    groups
}

fn binding_from_menu_item(
    item: &NativeMenuItem,
    behavior_prefix: &str,
) -> Option<NativeParamBindingSpec> {
    let key = item.key.as_ref()?.strip_prefix("behavior.")?;
    match &item.value {
        NativeMenuValue::Number { min, max, step, .. } => Some(NativeParamBindingSpec {
            key: format!("{behavior_prefix}.{key}"),
            label: Some(item.label.clone()),
            kind: "number".into(),
            min: Some(*min),
            max: Some(*max),
            step: Some(*step),
            options: vec![],
            invert: false,
        }),
        NativeMenuValue::Enum { options, .. } => Some(NativeParamBindingSpec {
            key: format!("{behavior_prefix}.{key}"),
            label: Some(item.label.clone()),
            kind: "enum".into(),
            min: None,
            max: None,
            step: None,
            options: options.clone(),
            invert: false,
        }),
        NativeMenuValue::Bool { .. } => Some(NativeParamBindingSpec {
            key: format!("{behavior_prefix}.{key}"),
            label: Some(item.label.clone()),
            kind: "bool".into(),
            min: None,
            max: None,
            step: None,
            options: vec![],
            invert: false,
        }),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn binding_action(
    label: &str,
    key: &str,
    kind: &str,
    min: Option<i32>,
    max: Option<i32>,
    step: Option<i32>,
    options: Vec<&str>,
    target: &str,
) -> NativeMenuItem {
    binding_action_from_spec(
        NativeParamBindingSpec {
            key: key.into(),
            label: Some(label.into()),
            kind: kind.into(),
            min,
            max,
            step,
            options: options.into_iter().map(str::to_string).collect(),
            invert: false,
        },
        target,
    )
}

fn binding_action_from_spec(binding: NativeParamBindingSpec, target: &str) -> NativeMenuItem {
    action_item(
        binding.label.clone().unwrap_or_else(|| binding.key.clone()),
        format!("{target}.{}", binding.key),
        NativeMenuAction::SetParamBinding {
            target: target.into(),
            binding,
        },
    )
}

fn xy_pad_items(config: &NativeMenuConfig) -> Vec<NativeMenuItem> {
    vec![
        parameter_picker_group(
            axis_binding_label("X Axis", config.xy_x_binding.as_ref()),
            "xy:x".into(),
            config.xy_x_binding.as_ref(),
            config,
        ),
        parameter_picker_group(
            axis_binding_label("Y Axis", config.xy_y_binding.as_ref()),
            "xy:y".into(),
            config.xy_y_binding.as_ref(),
            config,
        ),
        bool_item("Invert X", "dance.xy.invertX", config.xy_invert_x),
        bool_item("Invert Y", "dance.xy.invertY", config.xy_invert_y),
        enum_item(
            "Release",
            "dance.xy.release",
            vec!["sample-hold", "reset-center"],
            selected_index(&["sample-hold", "reset-center"], &config.xy_release),
        ),
    ]
}

fn system_group(config: &NativeMenuConfig, sync_index: usize) -> NativeMenuItem {
    group(
        "System",
        vec![
            group(
                "Saves",
                vec![
                    group(
                        "Library",
                        vec![
                            group(
                                "Save As",
                                vec![
                                    text_item(
                                        "Name",
                                        "system.draftName",
                                        config.preset_draft_name.clone(),
                                        32,
                                    ),
                                    action_item(
                                        "Save",
                                        "preset.saveAs.save",
                                        NativeMenuAction::PlatformEffect("preset.saveAs".into()),
                                    ),
                                ],
                            ),
                            action_item(
                                "Save Current",
                                "preset.saveCurrent",
                                NativeMenuAction::PlatformEffect("preset.saveCurrent".into()),
                            ),
                            preset_action_group("Load", "preset.load", &config.preset_names),
                            preset_rename_group(config),
                            preset_action_group("Delete", "preset.delete", &config.preset_names),
                            action_item(
                                "Refresh List",
                                "preset.refresh",
                                NativeMenuAction::PlatformEffect("preset.refresh".into()),
                            ),
                        ],
                    ),
                    group(
                        "Default",
                        vec![
                            action_item(
                                "Save Default",
                                "default.save",
                                NativeMenuAction::PlatformEffect("default.save".into()),
                            ),
                            action_item(
                                "Load Default",
                                "default.load",
                                NativeMenuAction::PlatformEffect("default.load".into()),
                            ),
                            bool_item("Auto Save", "autoSaveDefault", config.auto_save_default),
                        ],
                    ),
                    group(
                        "Factory",
                        vec![action_item(
                            "Load Fact. Default",
                            "factory.load",
                            NativeMenuAction::PlatformEffect("factory.load".into()),
                        )],
                    ),
                ],
            ),
            group(
                "Sound",
                vec![
                    number_item(
                        "Master Vol",
                        "masterVolume",
                        i32::from(config.master_volume),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "Note Length",
                        "sound.noteLengthMs",
                        i32::from(config.note_length_ms),
                        30,
                        2000,
                        10,
                    ),
                    number_item(
                        "Velocity Scale",
                        "sound.velocityScalePct",
                        i32::from(config.velocity_scale_pct),
                        0,
                        200,
                        5,
                    ),
                    enum_item(
                        "Velocity Curve",
                        "sound.velocityCurve",
                        vec!["linear", "soft", "hard"],
                        selected_index(&["linear", "soft", "hard"], &config.velocity_curve),
                    ),
                    enum_item(
                        "Voice Stealing",
                        "sound.voiceStealingMode",
                        vec!["off", "lenient", "balanced", "aggressive"],
                        selected_index(
                            &["off", "lenient", "balanced", "aggressive"],
                            &config.voice_stealing_mode,
                        ),
                    ),
                ],
            ),
            group(
                "MIDI",
                vec![
                    bool_item("Enabled", "midiEnabled", config.midi_enabled),
                    action_item(
                        "Panic",
                        "midi.panic",
                        NativeMenuAction::PlatformEffect("midi.panic".into()),
                    ),
                    midi_ports_group("MIDI Out", "midi.output", &config.midi_outputs),
                    midi_ports_group("MIDI In", "midi.input", &config.midi_inputs),
                    group(
                        "Sync & Clock",
                        vec![
                            enum_item_from_strings(
                                "Sync",
                                "midiSyncMode",
                                vec!["internal".into(), "external".into()],
                                sync_index,
                            ),
                            bool_item(
                                "Clock Out",
                                "midi.clockOutEnabled",
                                config.midi_clock_out_enabled,
                            ),
                            bool_item(
                                "Clock In",
                                "midi.clockInEnabled",
                                config.midi_clock_in_enabled,
                            ),
                            bool_item(
                                "Respond Start/Stop",
                                "midi.respondToStartStop",
                                config.midi_respond_to_start_stop,
                            ),
                        ],
                    ),
                ],
            ),
            group(
                "UI",
                vec![
                    bool_item("Ghost Cells", "ghostCells", config.ghost_cells),
                    bool_item(
                        "Input Events While Paused",
                        "inputEventsWhilePaused",
                        config.input_events_while_paused,
                    ),
                    enum_item(
                        "Numeric Display",
                        "numericDisplayMode",
                        vec!["bar", "numbers", "bar+numbers"],
                        selected_index(
                            &["bar", "numbers", "bar+numbers"],
                            &config.numeric_display_mode,
                        ),
                    ),
                    number_item(
                        "Screen Sleep",
                        "screenSleepSeconds",
                        i32::from(config.screen_sleep_seconds),
                        0,
                        600,
                        10,
                    ),
                    number_item(
                        "Display Brightness",
                        "displayBrightness",
                        i32::from(config.display_brightness),
                        10,
                        100,
                        5,
                    ),
                    number_item(
                        "Grid Brightness",
                        "gridBrightness",
                        i32::from(config.grid_brightness),
                        10,
                        100,
                        5,
                    ),
                    number_item(
                        "Button Brightness",
                        "buttonBrightness",
                        i32::from(config.button_brightness),
                        10,
                        100,
                        5,
                    ),
                ],
            ),
        ],
    )
}

fn preset_action_group(label: &str, action_prefix: &str, names: &[String]) -> NativeMenuItem {
    let children = if names.is_empty() {
        vec![action_item(
            "(none)",
            format!("{action_prefix}.none"),
            NativeMenuAction::PlatformEffect("preset.refresh".into()),
        )]
    } else {
        names
            .iter()
            .map(|name| {
                action_item(
                    name.clone(),
                    format!("{action_prefix}.{name}"),
                    NativeMenuAction::PlatformEffect(format!("{action_prefix}:{name}")),
                )
            })
            .collect()
    };
    group(label, children)
}

fn preset_rename_group(config: &NativeMenuConfig) -> NativeMenuItem {
    let mut children = if config.preset_names.is_empty() {
        vec![action_item(
            "(none)",
            "preset.rename.none",
            NativeMenuAction::PlatformEffect("preset.refresh".into()),
        )]
    } else {
        config
            .preset_names
            .iter()
            .map(|name| {
                action_item(
                    name.clone(),
                    format!("preset.renamePick.{name}"),
                    NativeMenuAction::PlatformEffect(format!("preset.renamePick:{name}")),
                )
            })
            .collect()
    };
    if config.preset_rename_source.is_some() {
        children.push(text_item(
            "New Name",
            "system.draftName",
            config.preset_draft_name.clone(),
            32,
        ));
        children.push(action_item(
            "Apply",
            "preset.rename.apply",
            NativeMenuAction::PlatformEffect("preset.renameApply".into()),
        ));
    }
    group("Rename", children)
}

fn midi_ports_group(
    label: &str,
    action_prefix: &str,
    ports: &[(String, String)],
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "Disconnect",
        format!("{action_prefix}.none"),
        NativeMenuAction::PlatformEffect(format!("{action_prefix}:")),
    )];
    children.extend(ports.iter().map(|(id, name)| {
        action_item(
            name.clone(),
            format!("{action_prefix}.{id}"),
            NativeMenuAction::PlatformEffect(format!("{action_prefix}:{id}")),
        )
    }));
    group(label, children)
}

fn aux_mappings_group(config: &NativeMenuConfig) -> NativeMenuItem {
    group(
        "Aux Mappings",
        (0..4)
            .map(|index| {
                let binding = config.aux_bindings.get(index).cloned().unwrap_or_default();
                group(
                    format!("Aux {}", index + 1),
                    vec![
                        parameter_picker_group(
                            axis_binding_label("Turn", binding.turn.as_ref()),
                            format!("aux:{index}:turn"),
                            binding.turn.as_ref(),
                            config,
                        ),
                        aux_click_picker_group(index, binding.click.as_ref(), config),
                    ],
                )
            })
            .collect(),
    )
}

fn aux_click_picker_group(
    index: usize,
    current: Option<&NativeMenuAction>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let mut children = vec![action_item(
        "(none)",
        format!("aux{}.click.none", index + 1),
        NativeMenuAction::SetAuxClick {
            index,
            action: None,
        },
    )];
    if let Some(action) = current {
        children.push(action_item(
            "Current",
            format!("aux{}.click.current", index + 1),
            NativeMenuAction::SetAuxClick {
                index,
                action: Some(Box::new(action.clone())),
            },
        ));
    }
    let behavior_actions = config
        .l1_items
        .iter()
        .filter_map(|item| match &item.value {
            NativeMenuValue::Action(NativeMenuAction::BehaviorAction(action)) => Some(action_item(
                item.label.clone(),
                format!("aux{}.click.behavior.{action}", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::BehaviorAction(action.clone()))),
                },
            )),
            _ => None,
        })
        .collect::<Vec<_>>();
    if !behavior_actions.is_empty() {
        children.push(group("Behavior", behavior_actions));
    }
    children.push(group(
        "Sample Assign",
        config
            .instrument_labels
            .iter()
            .enumerate()
            .map(|(instrument, label)| {
                action_item(
                    label.clone(),
                    format!("aux{}.click.sample.{instrument}", index + 1),
                    NativeMenuAction::SetAuxClick {
                        index,
                        action: Some(Box::new(NativeMenuAction::PlatformEffect(format!(
                            "sample.assign:{instrument}:0"
                        )))),
                    },
                )
            })
            .collect(),
    ));
    children.push(group(
        "Actions",
        vec![
            action_item(
                "Map FX",
                format!("aux{}.click.fx_map", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::PlatformEffect(
                        "dance.fx.map".into(),
                    ))),
                },
            ),
            action_item(
                "Reset Behavior",
                format!("aux{}.click.reset", index + 1),
                NativeMenuAction::SetAuxClick {
                    index,
                    action: Some(Box::new(NativeMenuAction::ResetBehavior)),
                },
            ),
        ],
    ));
    let label = current
        .map(|_| "Click: mapped".to_string())
        .unwrap_or_else(|| "Click: (none)".into());
    group(label, children)
}

fn selected_index(options: &[&str], value: &str) -> usize {
    options
        .iter()
        .position(|option| *option == value)
        .unwrap_or(0)
}

fn slot_option_selected(slot: usize, option_count: usize) -> usize {
    if slot == usize::MAX {
        0
    } else {
        (slot + 1).min(option_count.saturating_sub(1))
    }
}

fn default_sense_part_config() -> NativeSensePartConfig {
    NativeSensePartConfig {
        scan_mode: "immediate".into(),
        scan_axis: "rows".into(),
        scan_unit: "1/8".into(),
        scan_direction: "forward".into(),
        scan_sections: 1,
        scanned_slot: 0,
        scanned_action: "note_on".into(),
        scanned_empty_slot: usize::MAX,
        scanned_empty_action: "none".into(),
        event_enabled: true,
        activate_slot: 0,
        activate_action: "note_on".into(),
        stable_slot: 0,
        stable_action: "note_on".into(),
        deactivate_slot: 0,
        deactivate_action: "note_on".into(),
        trigger_probability_mode: "full".into(),
        trigger_probability_low_pct: 0,
        trigger_probability_high_pct: 100,
        state_notes_enabled: true,
        lowest_note: 24,
        highest_note: 84,
        starting_note: 60,
        scale: "chromatic".into(),
        root: "C".into(),
        out_of_range: "wrap".into(),
        x_pitch_enabled: true,
        x_pitch_steps: 1,
        x_pitch_restart_each_section: false,
        y_pitch_enabled: true,
        y_pitch_steps: 3,
        y_pitch_restart_each_section: false,
        x_from: 0,
        x_to: 7,
        x_velocity: value_lane_config(1, 127),
        x_filter_cutoff: value_lane_config(20, 127),
        x_filter_resonance: value_lane_config(10, 90),
        y_from: 0,
        y_to: 7,
        y_velocity: value_lane_config(1, 127),
        y_filter_cutoff: value_lane_config(20, 127),
        y_filter_resonance: value_lane_config(10, 90),
    }
}

fn value_lane_config(from: u8, to: u8) -> NativeValueLaneConfig {
    NativeValueLaneConfig {
        enabled: false,
        from,
        to,
        grid_offset: 0,
        curve: "linear".into(),
    }
}

fn l2_part_group(
    index: usize,
    label: String,
    instrument_options: &[String],
    sense: Option<&NativeSensePartConfig>,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let prefix = format!("parts.{index}.l2");
    let instrument_options = if instrument_options.is_empty() {
        vec!["none".to_string()]
    } else {
        let mut options = vec!["none".to_string()];
        options.extend(instrument_options.iter().cloned());
        options
    };
    let default_sense = default_sense_part_config();
    let sense = sense.unwrap_or(&default_sense);
    let mut scanning_children = vec![enum_item(
        "Scan Mode",
        format!("{prefix}.scanMode"),
        vec!["immediate", "scanning"],
        selected_index(&["immediate", "scanning"], &sense.scan_mode),
    )];
    if sense.scan_mode == "scanning" {
        scanning_children.extend(vec![
            enum_item(
                "Scan Axis",
                format!("{prefix}.scanAxis"),
                vec!["rows", "columns"],
                selected_index(&["rows", "columns"], &sense.scan_axis),
            ),
            enum_item(
                "Scan Unit",
                format!("{prefix}.scanUnit"),
                vec!["1/16", "1/8", "1/4", "1/2", "1/1"],
                selected_index(&["1/16", "1/8", "1/4", "1/2", "1/1"], &sense.scan_unit),
            ),
            enum_item(
                "Scan Direction",
                format!("{prefix}.scanDirection"),
                vec!["forward", "reverse"],
                selected_index(&["forward", "reverse"], &sense.scan_direction),
            ),
            enum_item(
                "Sections",
                format!("{prefix}.scanSections"),
                vec!["1", "2", "4", "8"],
                selected_index(&["1", "2", "4", "8"], &sense.scan_sections.to_string()),
            ),
            enum_item_from_strings(
                "Instrument",
                format!("{prefix}.mapping.scanned.slot"),
                instrument_options.clone(),
                slot_option_selected(sense.scanned_slot, instrument_options.len()),
            ),
            enum_item(
                "Action",
                format!("{prefix}.mapping.scanned.action"),
                vec!["none", "note_on", "note_off"],
                selected_index(&["none", "note_on", "note_off"], &sense.scanned_action),
            ),
            enum_item_from_strings(
                "Empty Instrument",
                format!("{prefix}.mapping.scanned_empty.slot"),
                instrument_options.clone(),
                slot_option_selected(sense.scanned_empty_slot, instrument_options.len()),
            ),
            enum_item(
                "Empty Action",
                format!("{prefix}.mapping.scanned_empty.action"),
                vec!["none", "note_on", "note_off"],
                selected_index(
                    &["none", "note_on", "note_off"],
                    &sense.scanned_empty_action,
                ),
            ),
        ]);
    }
    group(
        label,
        vec![
            group("Scanning", scanning_children),
            group(
                "Events",
                vec![
                    bool_item(
                        "Event Triggers",
                        format!("{prefix}.eventEnabled"),
                        sense.event_enabled,
                    ),
                    bool_item(
                        "State Notes",
                        format!("{prefix}.stateNotesEnabled"),
                        sense.state_notes_enabled,
                    ),
                    enum_item_from_strings(
                        "Activate Instrument",
                        format!("{prefix}.mapping.activate.slot"),
                        instrument_options.clone(),
                        slot_option_selected(sense.activate_slot, instrument_options.len()),
                    ),
                    enum_item(
                        "Activate Action",
                        format!("{prefix}.mapping.activate.action"),
                        vec!["none", "note_on", "note_off"],
                        selected_index(&["none", "note_on", "note_off"], &sense.activate_action),
                    ),
                    enum_item_from_strings(
                        "Stable Instrument",
                        format!("{prefix}.mapping.stable.slot"),
                        instrument_options.clone(),
                        slot_option_selected(sense.stable_slot, instrument_options.len()),
                    ),
                    enum_item(
                        "Stable Action",
                        format!("{prefix}.mapping.stable.action"),
                        vec!["none", "note_on", "note_off"],
                        selected_index(&["none", "note_on", "note_off"], &sense.stable_action),
                    ),
                    enum_item_from_strings(
                        "Deactivate Instrument",
                        format!("{prefix}.mapping.deactivate.slot"),
                        instrument_options.clone(),
                        slot_option_selected(sense.deactivate_slot, instrument_options.len()),
                    ),
                    enum_item(
                        "Deactivate Action",
                        format!("{prefix}.mapping.deactivate.action"),
                        vec!["none", "note_on", "note_off"],
                        selected_index(&["none", "note_on", "note_off"], &sense.deactivate_action),
                    ),
                ],
            ),
            group(
                "Trigger Prob.",
                vec![
                    enum_item(
                        "Mode",
                        format!("{prefix}.triggerProbabilityMode"),
                        vec!["zero", "custom", "full"],
                        selected_index(
                            &["zero", "custom", "full"],
                            &sense.trigger_probability_mode,
                        ),
                    ),
                    number_item(
                        "Low Prob",
                        format!("{prefix}.triggerProbabilityLowPct"),
                        i32::from(sense.trigger_probability_low_pct),
                        0,
                        100,
                        1,
                    ),
                    number_item(
                        "High Prob",
                        format!("{prefix}.triggerProbabilityHighPct"),
                        i32::from(sense.trigger_probability_high_pct),
                        0,
                        100,
                        1,
                    ),
                    action_item(
                        "Map Probability Grid",
                        format!("{prefix}.triggerProbability.map"),
                        NativeMenuAction::PlatformEffect(format!(
                            "trigger.probability.assign:{index}"
                        )),
                    ),
                ],
            ),
            group(
                "Mappings",
                vec![
                    param_mod_axis_group(index, "X Axis", "x", config),
                    param_mod_axis_group(index, "Y Axis", "y", config),
                ],
            ),
            group(
                "Note Mapping",
                vec![
                    number_item(
                        "Lowest Note",
                        format!("{prefix}.pitch.lowestNote"),
                        i32::from(sense.lowest_note),
                        0,
                        127,
                        1,
                    ),
                    number_item(
                        "Highest Note",
                        format!("{prefix}.pitch.highestNote"),
                        i32::from(sense.highest_note),
                        0,
                        127,
                        1,
                    ),
                    number_item(
                        "Starting Note",
                        format!("{prefix}.pitch.startingNote"),
                        i32::from(sense.starting_note),
                        0,
                        127,
                        1,
                    ),
                    enum_item(
                        "Scale",
                        format!("{prefix}.pitch.scale"),
                        vec![
                            "chromatic",
                            "major",
                            "natural_minor",
                            "dorian",
                            "mixolydian",
                            "major_pentatonic",
                            "minor_pentatonic",
                            "harmonic_minor",
                        ],
                        selected_index(
                            &[
                                "chromatic",
                                "major",
                                "natural_minor",
                                "dorian",
                                "mixolydian",
                                "major_pentatonic",
                                "minor_pentatonic",
                                "harmonic_minor",
                            ],
                            &sense.scale,
                        ),
                    ),
                    enum_item(
                        "Root",
                        format!("{prefix}.pitch.root"),
                        vec![
                            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                        ],
                        selected_index(
                            &[
                                "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
                            ],
                            &sense.root,
                        ),
                    ),
                    enum_item(
                        "Out of Range",
                        format!("{prefix}.pitch.outOfRange"),
                        vec!["clamp", "wrap"],
                        selected_index(&["clamp", "wrap"], &sense.out_of_range),
                    ),
                ],
            ),
            axis_group(
                &format!("{prefix}.x"),
                "X Axis",
                AxisMenuConfig {
                    offset_limit: 7,
                    pitch_enabled: sense.x_pitch_enabled,
                    pitch_steps: sense.x_pitch_steps,
                    restart_each_section: sense.x_pitch_restart_each_section,
                    velocity: &sense.x_velocity,
                    filter_cutoff: &sense.x_filter_cutoff,
                    filter_resonance: &sense.x_filter_resonance,
                },
            ),
            axis_group(
                &format!("{prefix}.y"),
                "Y Axis",
                AxisMenuConfig {
                    offset_limit: 7,
                    pitch_enabled: sense.y_pitch_enabled,
                    pitch_steps: sense.y_pitch_steps,
                    restart_each_section: sense.y_pitch_restart_each_section,
                    velocity: &sense.y_velocity,
                    filter_cutoff: &sense.y_filter_cutoff,
                    filter_resonance: &sense.y_filter_resonance,
                },
            ),
        ],
    )
}

fn param_mod_axis_group(
    part_index: usize,
    label: &str,
    axis: &str,
    config: &NativeMenuConfig,
) -> NativeMenuItem {
    let prefix = format!("parts.{part_index}.paramMods.{axis}");
    let bindings = config
        .param_mods
        .get(part_index)
        .cloned()
        .unwrap_or_default();
    let (slot1, slot2) = if axis == "x" {
        (bindings.x[0].as_ref(), bindings.x[1].as_ref())
    } else {
        (bindings.y[0].as_ref(), bindings.y[1].as_ref())
    };
    group(
        label,
        vec![
            parameter_picker_group(
                axis_binding_label("Slot 1", slot1),
                format!("param:{part_index}:{axis}:0"),
                slot1,
                config,
            ),
            bool_item(
                "Slot 1 Invert",
                format!("{prefix}.0.invert"),
                slot1.map(|binding| binding.invert).unwrap_or(false),
            ),
            parameter_picker_group(
                axis_binding_label("Slot 2", slot2),
                format!("param:{part_index}:{axis}:1"),
                slot2,
                config,
            ),
            bool_item(
                "Slot 2 Invert",
                format!("{prefix}.1.invert"),
                slot2.map(|binding| binding.invert).unwrap_or(false),
            ),
        ],
    )
}

struct AxisMenuConfig<'a> {
    offset_limit: i32,
    pitch_enabled: bool,
    pitch_steps: i32,
    restart_each_section: bool,
    velocity: &'a NativeValueLaneConfig,
    filter_cutoff: &'a NativeValueLaneConfig,
    filter_resonance: &'a NativeValueLaneConfig,
}

fn axis_group(prefix: &str, label: &str, config: AxisMenuConfig<'_>) -> NativeMenuItem {
    let mut pitch_children = vec![bool_item(
        "Enabled",
        format!("{prefix}.pitch.enabled"),
        config.pitch_enabled,
    )];
    if config.pitch_enabled {
        pitch_children.extend(vec![
            number_item(
                "Steps",
                format!("{prefix}.pitch.steps"),
                config.pitch_steps,
                -16,
                16,
                1,
            ),
            bool_item(
                "Restart Section",
                format!("{prefix}.pitch.restartEachSection"),
                config.restart_each_section,
            ),
        ]);
    }
    group(
        label,
        vec![
            group("Pitch Steps", pitch_children),
            lane_group(
                "Velocity",
                &format!("{prefix}.velocity"),
                config.velocity,
                config.offset_limit,
            ),
            lane_group(
                "Filter Cutoff",
                &format!("{prefix}.filterCutoff"),
                config.filter_cutoff,
                config.offset_limit,
            ),
            lane_group(
                "Filter Resonance",
                &format!("{prefix}.filterResonance"),
                config.filter_resonance,
                config.offset_limit,
            ),
        ],
    )
}

fn lane_group(
    label: &str,
    prefix: &str,
    lane: &NativeValueLaneConfig,
    offset_limit: i32,
) -> NativeMenuItem {
    let mut children = vec![bool_item(
        "Enabled",
        format!("{prefix}.enabled"),
        lane.enabled,
    )];
    if lane.enabled {
        children.extend(vec![
            number_item(
                "From",
                format!("{prefix}.from"),
                i32::from(lane.from),
                0,
                127,
                1,
            ),
            number_item("To", format!("{prefix}.to"), i32::from(lane.to), 0, 127, 1),
            number_item(
                "Grid Offset",
                format!("{prefix}.gridOffset"),
                lane.grid_offset,
                -offset_limit,
                offset_limit,
                1,
            ),
            enum_item(
                "Curve",
                format!("{prefix}.curve"),
                vec!["linear", "curve"],
                selected_index(&["linear", "curve"], &lane.curve),
            ),
        ]);
    }
    group(label, children)
}

fn enum_item_from_strings(
    label: impl Into<String>,
    key: impl Into<String>,
    options: Vec<String>,
    selected: usize,
) -> NativeMenuItem {
    NativeMenuItem {
        label: label.into(),
        key: Some(key.into()),
        value: NativeMenuValue::Enum { options, selected },
        children: vec![],
    }
}

struct InstrumentMenuConfig<'a> {
    index: usize,
    label: String,
    name: &'a str,
    kind: &'a str,
    auto_name: bool,
    note_behavior: &'a str,
    route: &'a str,
    volume: u8,
    pan_pos: u8,
    sample_slot: usize,
    synth_config: Option<&'a serde_json::Value>,
    synth_osc1_waveform: &'a str,
    synth_osc2_waveform: &'a str,
    synth_filter_type: &'a str,
    synth_filter_cutoff: u16,
    synth_gain_pct: u8,
    synth_filter_resonance: u8,
    sample_tune_semis: i8,
    sample_gain_pct: u8,
    sample_base_velocity: u8,
    sample_amp_velocity_sensitivity_pct: u8,
    sample_velocity_levels_enabled: bool,
    sample_velocity_high: u8,
    sample_velocity_medium: u8,
    sample_velocity_low: u8,
    sample_amp_env: Option<&'a serde_json::Value>,
    sample_filter: Option<&'a serde_json::Value>,
    sample_filter_env: Option<&'a serde_json::Value>,
    midi_enabled: bool,
    midi_channel: u8,
    midi_velocity: u8,
    midi_duration_ms: u16,
    sample_browser: Option<&'a NativeSampleBrowserConfig>,
}

fn instrument_group(config: InstrumentMenuConfig<'_>) -> NativeMenuItem {
    let InstrumentMenuConfig {
        index,
        label,
        name,
        kind,
        auto_name,
        note_behavior,
        route,
        volume,
        pan_pos,
        sample_slot,
        synth_config,
        synth_osc1_waveform,
        synth_osc2_waveform,
        synth_filter_type,
        synth_filter_cutoff,
        synth_gain_pct,
        synth_filter_resonance,
        sample_tune_semis,
        sample_gain_pct,
        sample_base_velocity,
        sample_amp_velocity_sensitivity_pct,
        sample_velocity_levels_enabled,
        sample_velocity_high,
        sample_velocity_medium,
        sample_velocity_low,
        sample_amp_env,
        sample_filter,
        sample_filter_env,
        midi_enabled,
        midi_channel,
        midi_velocity,
        midi_duration_ms,
        sample_browser,
    } = config;
    let prefix = format!("instruments.{index}");
    let type_selected = match kind {
        "none" => 0,
        "sampler" => 2,
        "midi" => 3,
        _ => 1,
    };
    let mut children = vec![
        enum_item(
            "Type",
            format!("{prefix}.type"),
            vec!["none", "synth", "sampler", "midi"],
            type_selected,
        ),
        enum_item(
            "Note Behavior",
            format!("{prefix}.noteBehavior"),
            vec!["oneshot", "hold"],
            selected_index(&["oneshot", "hold"], note_behavior),
        ),
    ];
    if kind == "synth" {
        children.push(group(
            "Synth",
            vec![
                group("Preset", vec![group("Load", synth_preset_items(index))]),
                group(
                    "Oscillator",
                    vec![
                        group(
                            "Osc 1",
                            vec![
                                enum_item(
                                    "Wave",
                                    format!("{prefix}.synth.osc1.waveform"),
                                    vec!["sine", "triangle", "saw", "square", "pulse"],
                                    selected_index(
                                        &["sine", "triangle", "saw", "square", "pulse"],
                                        synth_osc1_waveform,
                                    ),
                                ),
                                number_item(
                                    "Octave",
                                    format!("{prefix}.synth.osc1.octave"),
                                    synth_number(synth_config, &["osc1", "octave"], 0),
                                    -2,
                                    2,
                                    1,
                                ),
                                number_item(
                                    "Level",
                                    format!("{prefix}.synth.osc1.levelPct"),
                                    synth_number(synth_config, &["osc1", "levelPct"], 80),
                                    0,
                                    100,
                                    1,
                                ),
                                number_item(
                                    "Detune",
                                    format!("{prefix}.synth.osc1.detuneCents"),
                                    synth_number(synth_config, &["osc1", "detuneCents"], 0),
                                    -50,
                                    50,
                                    1,
                                ),
                                number_item(
                                    "Pulse Width",
                                    format!("{prefix}.synth.osc1.pulseWidthPct"),
                                    synth_number(synth_config, &["osc1", "pulseWidthPct"], 50),
                                    5,
                                    95,
                                    1,
                                ),
                            ],
                        ),
                        group(
                            "Osc 2",
                            vec![
                                enum_item(
                                    "Wave",
                                    format!("{prefix}.synth.osc2.waveform"),
                                    vec!["sine", "triangle", "saw", "square", "pulse"],
                                    selected_index(
                                        &["sine", "triangle", "saw", "square", "pulse"],
                                        synth_osc2_waveform,
                                    ),
                                ),
                                number_item(
                                    "Octave",
                                    format!("{prefix}.synth.osc2.octave"),
                                    synth_number(synth_config, &["osc2", "octave"], 0),
                                    -2,
                                    2,
                                    1,
                                ),
                                number_item(
                                    "Level",
                                    format!("{prefix}.synth.osc2.levelPct"),
                                    synth_number(synth_config, &["osc2", "levelPct"], 72),
                                    0,
                                    100,
                                    1,
                                ),
                                number_item(
                                    "Detune",
                                    format!("{prefix}.synth.osc2.detuneCents"),
                                    synth_number(synth_config, &["osc2", "detuneCents"], 0),
                                    -50,
                                    50,
                                    1,
                                ),
                                number_item(
                                    "Pulse Width",
                                    format!("{prefix}.synth.osc2.pulseWidthPct"),
                                    synth_number(synth_config, &["osc2", "pulseWidthPct"], 50),
                                    5,
                                    95,
                                    1,
                                ),
                            ],
                        ),
                    ],
                ),
                group(
                    "Filter",
                    vec![
                        enum_item(
                            "Type",
                            format!("{prefix}.synth.filter.type"),
                            vec!["lowpass", "highpass", "bandpass", "notch"],
                            selected_index(
                                &["lowpass", "highpass", "bandpass", "notch"],
                                synth_filter_type,
                            ),
                        ),
                        number_item(
                            "Cutoff",
                            format!("{prefix}.synth.filter.cutoffHz"),
                            cutoff_hz_to_display(i32::from(synth_filter_cutoff)),
                            0,
                            255,
                            1,
                        ),
                        number_item(
                            "Res",
                            format!("{prefix}.synth.filter.resonance"),
                            i32::from(synth_filter_resonance),
                            0,
                            255,
                            1,
                        ),
                        number_item(
                            "Env Amount",
                            format!("{prefix}.synth.filter.envAmountPct"),
                            synth_number(synth_config, &["filter", "envAmountPct"], 0),
                            -100,
                            100,
                            1,
                        ),
                        number_item(
                            "Key Tracking",
                            format!("{prefix}.synth.filter.keyTrackingPct"),
                            synth_number(synth_config, &["filter", "keyTrackingPct"], 0),
                            0,
                            100,
                            1,
                        ),
                    ],
                ),
                group(
                    "Volume",
                    vec![
                        number_item(
                            "Gain",
                            format!("{prefix}.synth.amp.gainPct"),
                            i32::from(synth_gain_pct),
                            0,
                            100,
                            1,
                        ),
                        number_item(
                            "Velocity Sens",
                            format!("{prefix}.synth.amp.velocitySensitivityPct"),
                            synth_number(synth_config, &["amp", "velocitySensitivityPct"], 100),
                            0,
                            100,
                            1,
                        ),
                    ],
                ),
                synth_env_group("Amp Env", &prefix, "ampEnv", synth_config),
                synth_env_group("Filter Env", &prefix, "filterEnv", synth_config),
            ],
        ));
    }
    if kind == "sampler" {
        let mut sampler_children = vec![
            enum_item(
                "Sample Slot",
                format!("{prefix}.sample.selectedSlot"),
                vec!["1", "2", "3", "4", "5", "6", "7", "8"],
                sample_slot.min(7),
            ),
            sample_browser_group(index, sample_slot.min(7), sample_browser),
            action_item(
                "Assign",
                format!("sample.assign.{index}.{}", sample_slot.min(7)),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.assign:{index}:{}",
                    sample_slot.min(7)
                )),
            ),
            number_item(
                "Tune",
                format!("{prefix}.sample.tuneSemis"),
                i32::from(sample_tune_semis),
                -24,
                24,
                1,
            ),
            number_item(
                "Gain",
                format!("{prefix}.sample.amp.gainPct"),
                i32::from(sample_gain_pct),
                0,
                100,
                1,
            ),
            number_item(
                "Base Velocity",
                format!("{prefix}.sample.baseVelocity"),
                i32::from(sample_base_velocity),
                1,
                127,
                1,
            ),
            bool_item(
                "Velocity Levels",
                format!("{prefix}.sample.velocityLevelsEnabled"),
                sample_velocity_levels_enabled,
            ),
            group(
                "Filter",
                vec![
                    enum_item(
                        "Type",
                        format!("{prefix}.sample.filter.type"),
                        vec!["lowpass", "highpass", "bandpass", "notch"],
                        selected_index(
                            &["lowpass", "highpass", "bandpass", "notch"],
                            sample_string(sample_filter, &["type"], "lowpass").as_str(),
                        ),
                    ),
                    number_item(
                        "Cutoff",
                        format!("{prefix}.sample.filter.cutoffHz"),
                        cutoff_hz_to_display(sample_number(sample_filter, &["cutoffHz"], 8000)),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Res",
                        format!("{prefix}.sample.filter.resonance"),
                        sample_number(sample_filter, &["resonance"], 20),
                        0,
                        255,
                        1,
                    ),
                    number_item(
                        "Env Amount",
                        format!("{prefix}.sample.filter.envAmountPct"),
                        sample_number(sample_filter, &["envAmountPct"], 0),
                        -100,
                        100,
                        1,
                    ),
                    number_item(
                        "Key Tracking",
                        format!("{prefix}.sample.filter.keyTrackingPct"),
                        sample_number(sample_filter, &["keyTrackingPct"], 0),
                        0,
                        100,
                        1,
                    ),
                ],
            ),
            number_item(
                "Velocity Sens",
                format!("{prefix}.sample.amp.velocitySensitivityPct"),
                i32::from(sample_amp_velocity_sensitivity_pct),
                0,
                100,
                1,
            ),
            sample_env_group("Amp Env", &prefix, "ampEnv", sample_amp_env),
            sample_env_group("Filter Env", &prefix, "filterEnv", sample_filter_env),
        ];
        if sample_velocity_levels_enabled {
            sampler_children.insert(
                7,
                group(
                    "Velocity Levels",
                    vec![
                        number_item(
                            "High",
                            format!("{prefix}.sample.velocityLevels.high"),
                            i32::from(sample_velocity_high),
                            1,
                            127,
                            1,
                        ),
                        number_item(
                            "Medium",
                            format!("{prefix}.sample.velocityLevels.medium"),
                            i32::from(sample_velocity_medium),
                            1,
                            127,
                            1,
                        ),
                        number_item(
                            "Low",
                            format!("{prefix}.sample.velocityLevels.low"),
                            i32::from(sample_velocity_low),
                            1,
                            127,
                            1,
                        ),
                    ],
                ),
            );
        }
        children.push(group("Sampler", sampler_children));
    }
    if kind == "midi" {
        children.push(group(
            "MIDI",
            vec![
                bool_item("Enabled", format!("{prefix}.midi.enabled"), midi_enabled),
                number_item(
                    "Channel",
                    format!("{prefix}.midi.channel"),
                    i32::from(midi_channel),
                    1,
                    16,
                    1,
                ),
                number_item(
                    "Velocity",
                    format!("{prefix}.midi.velocity"),
                    i32::from(midi_velocity),
                    1,
                    127,
                    1,
                ),
                number_item(
                    "Duration",
                    format!("{prefix}.midi.durationMs"),
                    i32::from(midi_duration_ms),
                    10,
                    2000,
                    10,
                ),
            ],
        ));
    }
    if kind == "synth" || kind == "sampler" {
        children.push(group(
            "Mixer",
            vec![
                enum_item(
                    "Route",
                    format!("{prefix}.mixer.route"),
                    vec!["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"],
                    selected_index(
                        &["direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "fx_bus_4"],
                        route,
                    ),
                ),
                number_item(
                    "Volume",
                    format!("{prefix}.mixer.volume"),
                    i32::from(volume),
                    0,
                    100,
                    1,
                ),
                number_item(
                    "Pan Pos",
                    format!("{prefix}.mixer.panPos"),
                    i32::from(pan_pos),
                    0,
                    32,
                    1,
                ),
            ],
        ));
    }
    children.push(bool_item(
        "Auto Name",
        format!("{prefix}.autoName"),
        auto_name,
    ));
    children.push(text_item("Name", format!("{prefix}.name"), name, 32));
    children.push(action_item(
        "Clone",
        format!("instruments.{index}.clone"),
        NativeMenuAction::CloneInstrument { index },
    ));
    children.push(action_item(
        "Reset",
        format!("instruments.{index}.reset"),
        NativeMenuAction::ResetInstrument { index },
    ));
    group(label, children)
}

fn synth_env_group(
    label: &str,
    prefix: &str,
    env_key: &str,
    synth_config: Option<&serde_json::Value>,
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.synth.{env_key}.attackMs"),
                synth_number(synth_config, &[env_key, "attackMs"], 5),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.synth.{env_key}.decayMs"),
                synth_number(synth_config, &[env_key, "decayMs"], 120),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.synth.{env_key}.sustainPct"),
                synth_number(synth_config, &[env_key, "sustainPct"], 70),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.synth.{env_key}.releaseMs"),
                synth_number(synth_config, &[env_key, "releaseMs"], 180),
                0,
                10000,
                5,
            ),
        ],
    )
}

fn synth_number(config: Option<&serde_json::Value>, path: &[&str], fallback: i32) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

fn sample_env_group(
    label: &str,
    prefix: &str,
    env_key: &str,
    config: Option<&serde_json::Value>,
) -> NativeMenuItem {
    group(
        label,
        vec![
            number_item(
                "Attack",
                format!("{prefix}.sample.{env_key}.attackMs"),
                sample_number(config, &["attackMs"], 5),
                0,
                5000,
                5,
            ),
            number_item(
                "Decay",
                format!("{prefix}.sample.{env_key}.decayMs"),
                sample_number(config, &["decayMs"], 120),
                0,
                5000,
                5,
            ),
            number_item(
                "Sustain",
                format!("{prefix}.sample.{env_key}.sustainPct"),
                sample_number(config, &["sustainPct"], 70),
                0,
                100,
                1,
            ),
            number_item(
                "Release",
                format!("{prefix}.sample.{env_key}.releaseMs"),
                sample_number(config, &["releaseMs"], 180),
                0,
                10000,
                5,
            ),
        ],
    )
}

fn sample_number(config: Option<&serde_json::Value>, path: &[&str], fallback: i32) -> i32 {
    let Some(mut current) = config else {
        return fallback;
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

fn cutoff_hz_to_display(hz: i32) -> i32 {
    let h = hz.clamp(80, 16_000) as f64;
    ((h / 80.0).ln() / (16_000.0_f64 / 80.0).ln() * 255.0).round() as i32
}

fn sample_string(config: Option<&serde_json::Value>, path: &[&str], fallback: &str) -> String {
    let Some(mut current) = config else {
        return fallback.into();
    };
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}

fn synth_preset_items(index: usize) -> Vec<NativeMenuItem> {
    [
        ("init", "init"),
        ("soft_pad", "soft pad"),
        ("bright_pluck", "bright pluck"),
        ("bass_mono", "bass mono"),
        ("hollow_pwm", "hollow pwm"),
        ("lead", "lead"),
        ("bell", "bell"),
        ("perc_hit", "perc hit"),
    ]
    .iter()
    .map(|(id, label)| {
        action_item(
            *label,
            format!("synth.preset.{index}.{id}"),
            NativeMenuAction::PlatformEffect(format!("synth.preset:{index}:{id}")),
        )
    })
    .collect()
}

fn sample_browser_group(
    instrument_slot: usize,
    sample_slot: usize,
    sample_browser: Option<&NativeSampleBrowserConfig>,
) -> NativeMenuItem {
    let mut children = Vec::new();
    if let Some(browser) = sample_browser {
        if browser.instrument_slot == instrument_slot && browser.sample_slot == sample_slot {
            children.push(action_item(
                "..",
                format!("sample.up.{instrument_slot}.{sample_slot}"),
                NativeMenuAction::PlatformEffect(format!(
                    "sample.up:{instrument_slot}:{sample_slot}"
                )),
            ));
            for entry in &browser.entries {
                let action = if entry.is_dir {
                    "sample.enter"
                } else {
                    "sample.pick"
                };
                children.push(action_item(
                    if entry.is_dir {
                        format!("[{}]", entry.name)
                    } else {
                        entry.name.clone()
                    },
                    format!("{action}.{instrument_slot}.{sample_slot}.{}", entry.path),
                    NativeMenuAction::PlatformEffect(format!(
                        "{action}:{instrument_slot}:{sample_slot}:{}",
                        entry.path
                    )),
                ));
            }
            if children.len() == 1 {
                children.push(action_item(
                    "(empty)",
                    format!("sample.open.{instrument_slot}.{sample_slot}"),
                    NativeMenuAction::PlatformEffect(format!(
                        "sample.open:{instrument_slot}:{sample_slot}:{}",
                        browser.dir
                    )),
                ));
            }
        }
    }
    NativeMenuItem {
        label: "Choose Sample".into(),
        key: Some(format!("sample.choose:{instrument_slot}:{sample_slot}")),
        value: NativeMenuValue::Group,
        children,
    }
}

fn fx_buses_group(config: &[NativeFxBusConfig]) -> NativeMenuItem {
    group(
        "FX Buses",
        (0..FX_BUS_COUNT)
            .map(|bus_index| {
                let prefix = format!("mixer.buses.{bus_index}");
                let bus = config
                    .get(bus_index)
                    .cloned()
                    .unwrap_or_else(default_fx_bus_config);
                group(
                    format!("B{}: {}", bus_index + 1, bus.name),
                    vec![
                        fx_slot_group(
                            "Slot 1",
                            &format!("{prefix}.slot1"),
                            &bus.slot1_type,
                            &bus.slot1_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        fx_slot_group(
                            "Slot 2",
                            &format!("{prefix}.slot2"),
                            &bus.slot2_type,
                            &bus.slot2_params,
                            FX_BUS_SLOT_OPTIONS,
                            Some(bus_index),
                        ),
                        number_item(
                            "Pan Pos",
                            format!("{prefix}.panPos"),
                            i32::from(bus.pan_pos),
                            0,
                            32,
                            1,
                        ),
                        bool_item("Auto Name", format!("{prefix}.autoName"), bus.auto_name),
                        text_item("Name", format!("{prefix}.name"), bus.name.clone(), 32),
                    ],
                )
            })
            .collect(),
    )
}

fn global_fx_group(config: &[String], params: &[serde_json::Value]) -> NativeMenuItem {
    group(
        "Global FX",
        (0..GLOBAL_FX_SLOT_COUNT)
            .map(|slot_index| {
                let prefix = format!("mixer.master.slots.{slot_index}");
                let slot_type = config.get(slot_index).map(String::as_str).unwrap_or("none");
                let slot_params = params.get(slot_index).unwrap_or(&serde_json::Value::Null);
                group(
                    format!("Slot {}", slot_index + 1),
                    fx_slot_children(
                        &prefix,
                        slot_type,
                        slot_params,
                        GLOBAL_FX_SLOT_OPTIONS,
                        None,
                    ),
                )
            })
            .collect(),
    )
}

fn fx_slot_group(
    label: impl Into<String>,
    prefix: &str,
    slot_type: &str,
    params: &serde_json::Value,
    options: &[&str],
    bus_index: Option<usize>,
) -> NativeMenuItem {
    group(
        label,
        fx_slot_children(prefix, slot_type, params, options, bus_index),
    )
}

fn fx_slot_children(
    prefix: &str,
    slot_type: &str,
    params: &serde_json::Value,
    options: &[&str],
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    let mut children = vec![enum_item(
        "Type",
        format!("{prefix}.type"),
        options.to_vec(),
        selected_index(options, slot_type),
    )];
    children.extend(fx_param_items(
        slot_type,
        &format!("{prefix}.params"),
        params,
        bus_index,
    ));
    children
}

fn fx_param_items(
    slot_type: &str,
    prefix: &str,
    params: &serde_json::Value,
    bus_index: Option<usize>,
) -> Vec<NativeMenuItem> {
    match slot_type {
        "duck" => {
            let options = duck_source_options(bus_index.unwrap_or(usize::MAX));
            vec![
                enum_item_from_strings(
                    "Source",
                    format!("{prefix}.source"),
                    options.clone(),
                    options
                        .iter()
                        .position(|option| option == &fx_param_string(params, "source", "I1"))
                        .unwrap_or(0),
                ),
                fx_number_item(
                    "Threshold",
                    prefix,
                    params,
                    "threshold",
                    0,
                    100,
                    1,
                    100.0,
                    0.08,
                ),
                fx_number_item(
                    "Amount %",
                    prefix,
                    params,
                    "amountPct",
                    0,
                    100,
                    1,
                    1.0,
                    60.0,
                ),
                fx_number_item("Attack ms", prefix, params, "attackMs", 1, 500, 1, 1.0, 8.0),
                fx_number_item(
                    "Release ms",
                    prefix,
                    params,
                    "releaseMs",
                    1,
                    5000,
                    5,
                    1.0,
                    160.0,
                ),
            ]
        }
        "delay" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 35.0),
            fx_number_item("Time ms", prefix, params, "timeMs", 1, 2000, 5, 1.0, 250.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", 0, 98, 1, 100.0, 0.35,
            ),
        ],
        "tremolo" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 5, 4000, 5, 100.0, 4.0),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 60.0),
        ],
        "saturator" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 200, 1, 10.0, 1.8),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "distortion" => vec![
            fx_number_item("Drive", prefix, params, "drive", 0, 500, 5, 10.0, 2.5),
            fx_number_item("Clip", prefix, params, "clip", 5, 200, 5, 100.0, 0.6),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "bitcrusher" => vec![
            fx_number_item("Bits", prefix, params, "bits", 1, 16, 1, 1.0, 6.0),
            fx_number_item("Rate Div", prefix, params, "rateDiv", 1, 128, 1, 1.0, 4.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vibrato" | "chorus" | "flanger" => vec![
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.8),
            fx_number_item("Depth ms", prefix, params, "depthMs", 0, 400, 1, 10.0, 6.0),
            fx_number_item("Base ms", prefix, params, "baseMs", 1, 800, 1, 10.0, 8.0),
            fx_number_item(
                "Feedback", prefix, params, "feedback", -95, 95, 1, 100.0, 0.0,
            ),
        ],
        "filter_lfo" | "wah" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item(
                "Center Hz",
                prefix,
                params,
                "centerHz",
                40,
                12000,
                20,
                1.0,
                1600.0,
            ),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 70.0),
            fx_number_item("Q", prefix, params, "q", 25, 2000, 25, 100.0, 1.0),
        ],
        "reverb" => vec![
            fx_number_item("Decay", prefix, params, "decay", 0, 995, 5, 1000.0, 0.72),
            fx_number_item("Damp", prefix, params, "damp", 0, 98, 1, 100.0, 0.35),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 30.0),
        ],
        "auto_pan" => vec![
            fx_number_item("Rate Hz", prefix, params, "rateHz", 2, 2000, 5, 100.0, 0.5),
            fx_number_item("Depth %", prefix, params, "depthPct", 0, 100, 1, 1.0, 100.0),
        ],
        "glitch" => vec![
            fx_number_item("Chance %", prefix, params, "chancePct", 0, 100, 1, 1.0, 8.0),
            fx_number_item("Slice ms", prefix, params, "sliceMs", 5, 500, 5, 1.0, 80.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "compressor" => vec![
            fx_number_item(
                "Threshold dB",
                prefix,
                params,
                "thresholdDb",
                -120,
                0,
                1,
                2.0,
                -24.0,
            ),
            fx_number_item("Ratio", prefix, params, "ratio", 2, 40, 1, 2.0, 4.0),
            fx_number_item(
                "Attack ms",
                prefix,
                params,
                "attackMs",
                1,
                200,
                1,
                1.0,
                10.0,
            ),
            fx_number_item(
                "Release ms",
                prefix,
                params,
                "releaseMs",
                5,
                2000,
                5,
                1.0,
                100.0,
            ),
            fx_number_item("Makeup dB", prefix, params, "makeupDb", 0, 48, 1, 2.0, 0.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "eq" => vec![
            fx_number_item(
                "Low Gain dB",
                prefix,
                params,
                "lowGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "Mid Gain dB",
                prefix,
                params,
                "midGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "High Gain dB",
                prefix,
                params,
                "highGainDb",
                -24,
                24,
                1,
                2.0,
                0.0,
            ),
            fx_number_item(
                "Mid Freq Hz",
                prefix,
                params,
                "midFreqHz",
                40,
                8000,
                10,
                1.0,
                1000.0,
            ),
            fx_number_item("Mid Q", prefix, params, "midQ", 25, 2000, 25, 100.0, 1.0),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        "vinyl" => vec![
            fx_number_item(
                "Saturation %",
                prefix,
                params,
                "saturationPct",
                0,
                100,
                1,
                1.0,
                15.0,
            ),
            fx_number_item(
                "Crackle %",
                prefix,
                params,
                "cracklePct",
                0,
                100,
                1,
                1.0,
                8.0,
            ),
            fx_number_item(
                "Warp Depth %",
                prefix,
                params,
                "warpDepthPct",
                0,
                100,
                1,
                1.0,
                5.0,
            ),
            fx_number_item("Mix %", prefix, params, "mixPct", 0, 100, 1, 1.0, 100.0),
        ],
        _ => vec![],
    }
}

#[expect(clippy::too_many_arguments, reason = "FX menu specs are data rows")]
fn fx_number_item(
    label: impl Into<String>,
    prefix: &str,
    params: &serde_json::Value,
    key: &str,
    min: i32,
    max: i32,
    step: i32,
    scale: f64,
    default: f64,
) -> NativeMenuItem {
    number_item(
        label,
        format!("{prefix}.{key}"),
        ((fx_param_number(params, key, default) * scale).round() as i32).clamp(min, max),
        min,
        max,
        step,
    )
}

fn fx_param_number(params: &serde_json::Value, key: &str, default: f64) -> f64 {
    params
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or(default)
}

fn fx_param_string(params: &serde_json::Value, key: &str, default: &str) -> String {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(default)
        .into()
}

fn duck_source_options(bus_index: usize) -> Vec<String> {
    let mut options: Vec<String> = (0..INSTRUMENT_COUNT)
        .map(|index| format!("I{}", index + 1))
        .collect();
    options.extend(
        (0..FX_BUS_COUNT)
            .filter(|index| *index != bus_index)
            .map(|index| format!("B{}", index + 1)),
    );
    options
}

fn default_fx_bus_config() -> NativeFxBusConfig {
    NativeFxBusConfig {
        name: "(none)".into(),
        slot1_type: "none".into(),
        slot1_params: serde_json::json!({}),
        slot2_type: "none".into(),
        slot2_params: serde_json::json!({}),
        pan_pos: 16,
        auto_name: true,
    }
}

#[cfg(test)]
mod tests;
