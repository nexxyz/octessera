use super::*;
use std::ffi::CString;
use std::io::Write;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
use std::cell::Cell;

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
thread_local! {
    static ATOMIC_FAILURE: Cell<u8> = const { Cell::new(0) };
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
#[derive(Clone, Copy)]
pub(crate) enum AtomicFailure {
    Write = 1,
    Sync = 2,
    Rename = 3,
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn inject_atomic_failure(failure: AtomicFailure) {
    ATOMIC_FAILURE.with(|value| value.set(failure as u8));
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
fn maybe_fail(failure: AtomicFailure) -> Result<(), String> {
    ATOMIC_FAILURE.with(|value| {
        if value.get() == failure as u8 {
            value.set(0);
            Err(format!("injected atomic {} failure", failure as u8))
        } else {
            Ok(())
        }
    })
}

struct AtomicTemp<'a> {
    directory: &'a HandoffDirectory,
    name: CString,
    committed: bool,
}

impl Drop for AtomicTemp<'_> {
    fn drop(&mut self) {
        if !self.committed {
            unsafe { libc::unlinkat(self.directory.fd(), self.name.as_ptr(), 0) };
        }
    }
}

pub(super) fn atomic_write(
    directory: &HandoffDirectory,
    name: &str,
    mode: u32,
    bytes: &[u8],
    max: usize,
    no_replace: bool,
) -> Result<bool, String> {
    if bytes.len() > max {
        return Err(format!("OLED handoff {name} exceeds {max} bytes"));
    }
    let temp = format!(".{name}.tmp-{}", random_request_id()?);
    let mut file = create_named(directory, &temp, mode)?;
    let from = CString::new(temp).expect("temporary handoff name");
    let mut cleanup = AtomicTemp {
        directory,
        name: from.clone(),
        committed: false,
    };
    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    maybe_fail(AtomicFailure::Write)?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    maybe_fail(AtomicFailure::Sync)?;
    file.sync_all().map_err(|error| error.to_string())?;
    let to = CString::new(name).expect("handoff name");
    #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
    maybe_fail(AtomicFailure::Rename)?;
    let rename_result = if no_replace {
        unsafe {
            libc::syscall(
                libc::SYS_renameat2,
                directory.fd(),
                from.as_ptr(),
                directory.fd(),
                to.as_ptr(),
                1_u32,
            )
        }
    } else {
        unsafe { libc::renameat(directory.fd(), from.as_ptr(), directory.fd(), to.as_ptr()).into() }
    };
    if rename_result != 0 {
        let error = io_error();
        if no_replace && error.raw_os_error() == Some(libc::EEXIST) {
            return Ok(false);
        }
        return Err(format!("cannot publish OLED handoff {name}: {error}"));
    }
    directory
        .file
        .sync_all()
        .map_err(|error| format!("cannot sync OLED handoff directory: {error}"))?;
    cleanup.committed = true;
    Ok(true)
}
