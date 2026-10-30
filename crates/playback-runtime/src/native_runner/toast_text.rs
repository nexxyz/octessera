use super::NativeToast;
use crate::oled_frame::TOAST_RECT;

pub(super) fn clip_display_line(line: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in line.chars().take(width) {
        out.push(ch);
    }
    out
}

pub(super) fn scrolled_toast(toast: &NativeToast, hold_at_end: bool) -> String {
    let width = TOAST_RECT.columns();
    let chars = toast.message.chars().collect::<Vec<_>>();
    if chars.len() <= width {
        return toast.message.clone();
    }
    let offset = if hold_at_end {
        toast.offset.min(chars.len() - width)
    } else {
        toast.offset % (chars.len() + 3)
    };
    let mut padded = chars.clone();
    padded.extend([' ', ' ', ' ']);
    padded.extend(chars.iter().copied());
    let mut visible = padded
        .into_iter()
        .skip(offset)
        .take(width)
        .collect::<Vec<_>>();
    if offset + width < chars.len() {
        visible.truncate(width - 2);
        visible.extend(['.', '.']);
    }
    visible.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toast_scroll_uses_the_physical_seventeen_column_boundary() {
        let width = TOAST_RECT.columns();
        let short = NativeToast {
            message: "a".repeat(width),
            offset: 0,
        };
        let long = NativeToast {
            message: "a".repeat(width + 1),
            offset: 0,
        };

        assert_eq!(scrolled_toast(&short, false).chars().count(), width);
        assert_eq!(scrolled_toast(&long, false).chars().count(), width);
    }

    #[test]
    fn cut_toast_window_ends_in_dots_until_the_message_end_is_visible() {
        let width = TOAST_RECT.columns();
        let message = "Saved Patch to the SD card";
        let toast = |offset| NativeToast {
            message: message.into(),
            offset,
        };
        let end = message.chars().count() - width;

        assert_eq!(scrolled_toast(&toast(0), false), "Saved Patch to ..");
        assert_eq!(scrolled_toast(&toast(end - 1), false), "tch to the SD c..");
        assert_eq!(scrolled_toast(&toast(end), false), "ch to the SD card");
        assert_eq!(scrolled_toast(&toast(end + 5), true), "ch to the SD card");
    }
}
