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
    if hold_at_end {
        let offset = toast.offset.min(chars.len() - width);
        return chars[offset..offset + width].iter().collect();
    }
    let span = chars.len() + 3;
    let offset = toast.offset % span;
    let mut padded = chars;
    padded.extend([' ', ' ', ' ']);
    padded.extend(toast.message.chars());
    padded.iter().skip(offset).take(width).collect()
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
}
