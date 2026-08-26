use super::model::OledRuntimeErrorMetadata;
use super::text_layout::{
    fit_line_ellipsis, force_line_ellipsis, normalize_text, wrap_text, RUNTIME_ERROR_BODY_RECT,
};

pub const ERROR_METADATA_ROWS: usize = 3;
pub const ERROR_ROW_COUNT: usize = RUNTIME_ERROR_BODY_RECT.rows();
pub const ERROR_ROW_WIDTH: usize = RUNTIME_ERROR_BODY_RECT.columns();

pub fn runtime_error_rows(error: &OledRuntimeErrorMetadata) -> [String; ERROR_ROW_COUNT] {
    let mut rows = std::array::from_fn(|_| String::new());
    rows[0] = metadata_row("DOMAIN ", error.domain.as_deref(), ERROR_ROW_WIDTH);
    rows[1] = metadata_row("CODE ", error.code.as_deref(), ERROR_ROW_WIDTH);
    rows[2] = metadata_row("OP ", error.operation.as_deref(), ERROR_ROW_WIDTH);

    let message = normalize_text(error.message.as_deref().unwrap_or("needs attention"));
    let message = if message.is_empty() {
        "needs attention"
    } else {
        message.as_str()
    };
    let message_width = ERROR_ROW_WIDTH.saturating_sub(4);
    let message_rows = wrap_text(message, message_width);
    let overflow = message_rows.len() > ERROR_ROW_COUNT - ERROR_METADATA_ROWS;
    for (index, message_row) in message_rows
        .into_iter()
        .take(ERROR_ROW_COUNT - ERROR_METADATA_ROWS)
        .enumerate()
    {
        let prefix = if index == 0 { "MSG " } else { "    " };
        rows[index + ERROR_METADATA_ROWS] =
            if overflow && index + 1 == ERROR_ROW_COUNT - ERROR_METADATA_ROWS {
                format!(
                    "{prefix}{}",
                    force_line_ellipsis(&message_row, message_width)
                )
            } else {
                format!("{prefix}{message_row}")
            };
    }
    rows
}

fn metadata_row(prefix: &str, value: Option<&str>, width: usize) -> String {
    let value = normalize_text(&value.unwrap_or("unknown").replace('_', " "));
    let value = if value.is_empty() {
        "unknown"
    } else {
        value.as_str()
    };
    fit_line_ellipsis(&format!("{prefix}{value}"), width)
}
