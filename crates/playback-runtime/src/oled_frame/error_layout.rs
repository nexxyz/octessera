use super::font::glyph_rows;
use super::model::OledRuntimeErrorMetadata;

pub const ERROR_ROW_COUNT: usize = 7;
pub const ERROR_ROW_WIDTH: usize = 18;

const DOMAIN_PREFIX: &str = "DOMAIN ";
const CODE_PREFIX: &str = "CODE ";
const OP_PREFIX: &str = "OP ";
const MESSAGE_PREFIX: &str = "MSG ";
const CONTINUATION_PREFIX: &str = "    ";
const DOMAIN_WIDTH: usize = 11;
const CODE_WIDTH: usize = 13;
const OP_WIDTH: usize = 15;
const MESSAGE_WIDTH: usize = 14;
const MESSAGE_ROW_COUNT: usize = 4;

pub fn runtime_error_rows(error: &OledRuntimeErrorMetadata) -> [String; ERROR_ROW_COUNT] {
    let message = wrap_message(&normalize_message(error.message.as_deref()));
    let mut rows = [
        prefixed(
            DOMAIN_PREFIX,
            &normalize_identifier(error.domain.as_deref(), DOMAIN_WIDTH),
        ),
        prefixed(
            CODE_PREFIX,
            &normalize_identifier(error.code.as_deref(), CODE_WIDTH),
        ),
        prefixed(
            OP_PREFIX,
            &normalize_identifier(error.operation.as_deref(), OP_WIDTH),
        ),
        prefixed(MESSAGE_PREFIX, &message[0]),
        String::new(),
        String::new(),
        String::new(),
    ];
    for (index, row) in message.iter().enumerate().skip(1) {
        if !row.is_empty() {
            rows[index + 3] = prefixed(CONTINUATION_PREFIX, row);
        }
    }
    rows
}

fn prefixed(prefix: &str, value: &str) -> String {
    let mut row = String::with_capacity(prefix.len() + value.len());
    row.push_str(prefix);
    row.push_str(value);
    row
}

fn normalize_identifier(value: Option<&str>, width: usize) -> String {
    let collapsed = collapse(value.unwrap_or("unknown"), true);
    let value = if collapsed.is_empty() {
        "unknown".to_owned()
    } else {
        collapsed
    };
    truncate_prefix(&supported_text(&value), width)
}

fn normalize_message(value: Option<&str>) -> String {
    let collapsed = collapse(value.unwrap_or("needs attention"), false);
    let value = if collapsed.is_empty() {
        "needs attention".to_owned()
    } else {
        collapsed
    };
    supported_text(&value)
}

fn collapse(value: &str, underscores_are_spaces: bool) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_control()
                || character.is_whitespace()
                || (underscores_are_spaces && character == '_')
            {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn supported_text(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character == ' ' || glyph_rows(character) != [0; 7] {
                character
            } else {
                '?'
            }
        })
        .collect()
}

fn truncate_prefix(value: &str, width: usize) -> String {
    if value.chars().count() <= width {
        return value.to_owned();
    }
    let mut result = value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}

fn wrap_message(value: &str) -> [String; MESSAGE_ROW_COUNT] {
    let mut rows = Vec::with_capacity(MESSAGE_ROW_COUNT);
    let mut current = String::new();
    let mut truncated = false;

    'words: for word in value.split_whitespace() {
        if word.chars().count() > MESSAGE_WIDTH {
            if !current.is_empty() {
                rows.push(std::mem::take(&mut current));
                if rows.len() == MESSAGE_ROW_COUNT {
                    truncated = true;
                    break 'words;
                }
            }
            let mut characters = word.chars();
            loop {
                let chunk = characters.by_ref().take(MESSAGE_WIDTH).collect::<String>();
                if chunk.is_empty() {
                    break;
                }
                if characters.clone().next().is_some() {
                    if rows.len() == MESSAGE_ROW_COUNT {
                        truncated = true;
                        break 'words;
                    }
                    rows.push(chunk);
                } else {
                    current = chunk;
                    break;
                }
            }
            continue;
        }

        let next_width = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if next_width > MESSAGE_WIDTH {
            rows.push(std::mem::take(&mut current));
            current = word.to_owned();
            if rows.len() == MESSAGE_ROW_COUNT {
                truncated = true;
                break;
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(word);
        }
    }

    if !truncated && !current.is_empty() {
        if rows.len() < MESSAGE_ROW_COUNT {
            rows.push(current);
        } else {
            truncated = true;
        }
    }
    while rows.len() < MESSAGE_ROW_COUNT {
        rows.push(String::new());
    }
    if truncated {
        let last_index = rows.iter().rposition(|row| !row.is_empty()).unwrap_or(0);
        rows[last_index] = with_ellipsis(&rows[last_index], MESSAGE_WIDTH);
    }
    std::array::from_fn(|index| rows[index].clone())
}

fn with_ellipsis(value: &str, width: usize) -> String {
    let mut result = value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>();
    result.push_str("...");
    result
}
