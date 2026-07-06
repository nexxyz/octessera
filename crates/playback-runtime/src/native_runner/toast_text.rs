use super::NativeToast;

pub(super) fn clip_display_line(line: &str, width: usize) -> String {
    let mut out = String::new();
    for ch in line.chars().take(width) {
        out.push(ch);
    }
    out
}

pub(super) fn scrolled_toast(toast: &NativeToast) -> String {
    const WIDTH: usize = 28;
    let chars = toast.message.chars().collect::<Vec<_>>();
    if chars.len() <= WIDTH {
        return toast.message.clone();
    }
    let span = chars.len() + 3;
    let offset = toast.offset % span;
    let mut padded = chars;
    padded.extend([' ', ' ', ' ']);
    padded.extend(toast.message.chars());
    padded.iter().skip(offset).take(WIDTH).collect()
}
