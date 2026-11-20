use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

const HANDOFF_FILE: &str = "octessera.handoff.json";
const IO_TIMEOUT: Duration = Duration::from_secs(2);

/// Removes the port file when the app ends normally; a stale file only makes the next second
/// instance show the startup error instead.
#[derive(Debug)]
pub(crate) struct HandoffPort {
    path: PathBuf,
}

impl Drop for HandoffPort {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn random_token() -> String {
    let word = || RandomState::new().build_hasher().finish();
    format!("{:016x}{:016x}", word(), word())
}

/// Lets a second instance on this store ask the running one to show itself. `show` runs on the
/// listener thread; a request without the token written next to the store lock is dropped.
pub(crate) fn listen<F>(store_dir: &Path, show: F) -> Result<HandoffPort, String>
where
    F: Fn() + Send + 'static,
{
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|error| error.to_string())?;
    let port = listener
        .local_addr()
        .map_err(|error| error.to_string())?
        .port();
    let token = random_token();
    let path = store_dir.join(HANDOFF_FILE);
    store_files::write_json_atomically(
        &path,
        &serde_json::json!({ "port": port, "token": token, "pid": std::process::id() }),
    )?;
    std::thread::Builder::new()
        .name("octessera-instance-handoff".to_string())
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                serve(&stream, &token, &show);
            }
        })
        .map_err(|error| error.to_string())?;
    Ok(HandoffPort { path })
}

fn serve(stream: &TcpStream, token: &str, show: &impl Fn()) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return;
    }
    if line.trim_end() != format!("{token} show") {
        return;
    }
    show();
    let _ = (&*stream).write_all(b"ok\n");
}

struct Handoff {
    port: u16,
    token: String,
    pid: Option<u32>,
}

fn read_handoff(store_dir: &Path) -> Option<Handoff> {
    let text = std::fs::read_to_string(store_dir.join(HANDOFF_FILE)).ok()?;
    let value = serde_json::from_str::<serde_json::Value>(&text).ok()?;
    Some(Handoff {
        port: u16::try_from(value["port"].as_u64()?).ok()?,
        token: value["token"].as_str()?.to_string(),
        pid: value["pid"]
            .as_u64()
            .and_then(|pid| u32::try_from(pid).ok()),
    })
}

/// Windows only lets the foreground process hand the foreground right on; this process is it
/// at launch, so it grants the running instance the right before asking it to focus.
#[cfg(windows)]
fn allow_foreground(pid: Option<u32>) {
    const ASFW_ANY: u32 = u32::MAX;
    #[link(name = "user32")]
    extern "system" {
        fn AllowSetForegroundWindow(process_id: u32) -> i32;
    }
    // SAFETY: plain Win32 call taking a process id by value.
    unsafe {
        AllowSetForegroundWindow(pid.unwrap_or(ASFW_ANY));
    }
}

#[cfg(not(windows))]
fn allow_foreground(_pid: Option<u32>) {}

/// True when the running instance on this store acknowledged the request to show itself.
pub(crate) fn request_show(store_dir: &Path) -> bool {
    let Some(Handoff { port, token, pid }) = read_handoff(store_dir) else {
        return false;
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    allow_foreground(pid);
    let Ok(mut stream) = TcpStream::connect_timeout(&address, IO_TIMEOUT) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    if writeln!(stream, "{token} show").is_err() {
        return false;
    }
    let mut reply = String::new();
    BufReader::new(&stream).read_line(&mut reply).is_ok() && reply == "ok\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("octessera-handoff-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create temp directory");
        dir
    }

    fn counting_listener(dir: &Path) -> (HandoffPort, Arc<AtomicUsize>) {
        let shown = Arc::new(AtomicUsize::new(0));
        let counter = shown.clone();
        let port = listen(dir, move || {
            counter.fetch_add(1, Ordering::SeqCst);
        })
        .expect("listen");
        (port, shown)
    }

    #[test]
    fn a_second_instance_makes_the_first_show_itself() {
        let dir = temp_dir("roundtrip");
        let (port, shown) = counting_listener(&dir);

        assert!(request_show(&dir));
        assert_eq!(shown.load(Ordering::SeqCst), 1);

        drop(port);
        assert!(!dir.join(HANDOFF_FILE).exists());
        assert!(!request_show(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_request_with_the_wrong_token_is_ignored() {
        let dir = temp_dir("token");
        let (_port, shown) = counting_listener(&dir);
        let text = std::fs::read_to_string(dir.join(HANDOFF_FILE)).expect("port file");
        let value: serde_json::Value = serde_json::from_str(&text).expect("json");
        let port = u16::try_from(value["port"].as_u64().expect("port")).expect("u16");

        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("connect");
        stream.write_all(b"not-the-token show\n").expect("write");
        let mut reply = String::new();
        let _ = BufReader::new(&stream).read_line(&mut reply);

        assert!(reply.is_empty());
        assert_eq!(shown.load(Ordering::SeqCst), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_port_file_carries_the_listening_pid() {
        let dir = temp_dir("pid");
        let (_port, _shown) = counting_listener(&dir);
        let handoff = read_handoff(&dir).expect("port file");
        assert_eq!(handoff.pid, Some(std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_missing_or_stale_port_file_is_not_a_handoff() {
        let dir = temp_dir("stale");
        assert!(!request_show(&dir));

        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind");
        let dead_port = listener.local_addr().expect("addr").port();
        drop(listener);
        std::fs::write(
            dir.join(HANDOFF_FILE),
            format!("{{\"port\":{dead_port},\"token\":\"t\"}}"),
        )
        .expect("write stale file");
        assert!(!request_show(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
