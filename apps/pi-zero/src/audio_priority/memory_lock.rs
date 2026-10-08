/// Keeps the instrument's pages resident. Without this, memory pressure from
/// other processes (for example the daily apt refresh) evicts code the audio
/// callback runs, and code first used when playback starts is read from the
/// SD card inside the callback; both stall it into an underrun. Read-only code
/// and data of the binary and its libraries are loaded and locked up front;
/// everything else is locked once touched. Both board services grant
/// `LimitMEMLOCK=infinity` for this.
pub(crate) fn lock_process_memory() {
    let flags = libc::MCL_CURRENT | libc::MCL_FUTURE | libc::MCL_ONFAULT;
    if unsafe { libc::mlockall(flags) } != 0 {
        eprintln!(
            "Audio memory lock unavailable: {}",
            std::io::Error::last_os_error()
        );
        return;
    }
    let (Ok(maps), Ok(exe)) = (
        std::fs::read_to_string("/proc/self/maps"),
        std::fs::read_link("/proc/self/exe"),
    ) else {
        return;
    };
    for (start, len) in code_mappings(&maps, &exe.to_string_lossy()) {
        if unsafe { libc::mlock(start as *const libc::c_void, len) } != 0 {
            eprintln!(
                "Audio code preload incomplete: {}",
                std::io::Error::last_os_error()
            );
            return;
        }
    }
}

fn code_mappings<'a>(maps: &'a str, exe: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
    maps.lines().filter_map(move |line| {
        let mut fields = line.split_whitespace();
        let (range, perms) = (fields.next()?, fields.next()?);
        let path = fields.nth(3)?;
        let read_only = perms.starts_with('r') && perms.as_bytes().get(1) == Some(&b'-');
        if !read_only || (path != exe && !path.contains(".so")) {
            return None;
        }
        let (start, end) = range.split_once('-')?;
        let start = usize::from_str_radix(start, 16).ok()?;
        let end = usize::from_str_radix(end, 16).ok()?;
        (end > start).then_some((start, end - start))
    })
}

#[cfg(test)]
mod tests {
    use super::code_mappings;

    #[test]
    fn preload_covers_read_only_code_of_the_binary_and_libraries() {
        let maps = "\
1000-3000 r--p 00000000 b3:02 11 /opt/octessera/octessera-pi
3000-8000 r-xp 00002000 b3:02 11 /opt/octessera/octessera-pi
8000-9000 rw-p 00007000 b3:02 11 /opt/octessera/octessera-pi
9000-a000 rw-p 00000000 00:00 0 [heap]
a000-b000 r-xp 00000000 b3:02 12 /usr/lib/aarch64-linux-gnu/libasound.so.2.0.0
b000-c000 r--p 00000000 b3:02 13 /home/pi/samples/kick.wav
c000-d000 r-xp 00000000 00:00 0 [vdso]
d000-e000 ---p 00000000 00:00 0
";
        let mappings: Vec<_> = code_mappings(maps, "/opt/octessera/octessera-pi").collect();
        assert_eq!(
            mappings,
            vec![(0x1000, 0x2000), (0x3000, 0x5000), (0xa000, 0x1000)]
        );
    }
}
