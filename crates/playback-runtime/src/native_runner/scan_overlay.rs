pub(super) fn scan_section_count(value: u8, size: usize) -> usize {
    match value {
        2 | 4 | 8 => usize::from(value).min(size),
        _ => 1,
    }
}

pub(super) fn scan_index_for_overlay(tick: usize, span: usize, reverse: bool) -> usize {
    if span == 0 {
        return 0;
    }
    let index = tick % span;
    if reverse {
        span - 1 - index
    } else {
        index
    }
}
