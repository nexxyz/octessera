use super::*;
use playback_runtime::{NativeGridPresentation, NativeHdmiMode, NativeHdmiPresentation};
use std::io;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeControl {
    events: Vec<String>,
    writes: Vec<Vec<u8>>,
    framebuffer_available: bool,
    tty_available: bool,
    fail_graphics: usize,
    fail_text: usize,
    fail_unblank: usize,
    fail_write: usize,
}

type SharedControl = Arc<Mutex<FakeControl>>;

struct FakeIo {
    control: SharedControl,
}

struct FakeFramebuffer {
    control: SharedControl,
}

struct FakeTty {
    control: SharedControl,
}

impl FakeControl {
    fn event(&mut self, event: impl Into<String>) {
        self.events.push(event.into());
    }

    fn fail(counter: &mut usize) -> io::Result<()> {
        if *counter == 0 {
            Ok(())
        } else {
            *counter -= 1;
            Err(io::Error::from_raw_os_error(libc::EIO))
        }
    }
}

impl HdmiIo for FakeIo {
    fn open_framebuffer(&mut self, _path: &str) -> Result<Box<dyn FramebufferHandle>, HdmiError> {
        let mut control = self.control.lock().unwrap();
        if !control.framebuffer_available {
            return Err(HdmiError::Io {
                operation: "open",
                path: "/dev/fb0".into(),
                source: io::Error::from_raw_os_error(libc::ENOENT),
            });
        }
        control.event("open_fb");
        control.event("fb_var");
        control.event("fb_fix");
        drop(control);
        Ok(Box::new(FakeFramebuffer {
            control: Arc::clone(&self.control),
        }))
    }

    fn open_tty(&mut self, _path: &str) -> Result<Box<dyn TtyHandle>, HdmiError> {
        let mut control = self.control.lock().unwrap();
        if !control.tty_available {
            return Err(HdmiError::Io {
                operation: "open",
                path: "/dev/tty1".into(),
                source: io::Error::from_raw_os_error(libc::ENOENT),
            });
        }
        control.event("open_tty");
        drop(control);
        Ok(Box::new(FakeTty {
            control: Arc::clone(&self.control),
        }))
    }
}

impl FramebufferHandle for FakeFramebuffer {
    fn geometry(&self) -> FramebufferGeometry {
        FramebufferGeometry {
            width: 64,
            height: 64,
            stride: 64 * 4,
            bytes_per_pixel: 4,
        }
    }

    fn unblank(&mut self) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        control.event("fb_unblank");
        FakeControl::fail(&mut control.fail_unblank)
    }

    fn write_frame(&mut self, frame: &[u8]) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        control.event("write");
        if control.fail_write > 0 {
            control.fail_write -= 1;
            return Err(io::Error::from_raw_os_error(libc::EIO));
        }
        control.writes.push(frame.to_vec());
        Ok(())
    }
}

impl Drop for FakeFramebuffer {
    fn drop(&mut self) {
        self.control.lock().unwrap().event("close_fb");
    }
}

impl TtyHandle for FakeTty {
    fn set_graphics(&mut self) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        control.event("kd_graphics");
        FakeControl::fail(&mut control.fail_graphics)
    }

    fn set_text(&mut self) -> io::Result<()> {
        let mut control = self.control.lock().unwrap();
        control.event("kd_text");
        FakeControl::fail(&mut control.fail_text)
    }
}

impl Drop for FakeTty {
    fn drop(&mut self) {
        self.control.lock().unwrap().event("close_tty");
    }
}

fn fake_device() -> (HdmiDevice, SharedControl) {
    let control = Arc::new(Mutex::new(FakeControl {
        framebuffer_available: true,
        tty_available: true,
        ..Default::default()
    }));
    let device = HdmiDevice::new(
        Box::new(FakeIo {
            control: Arc::clone(&control),
        }),
        "/dev/fb0",
        "/dev/tty1",
    );
    (device, control)
}

fn presentation(color: [u8; 3], mode: NativeHdmiMode) -> NativeHdmiPresentation {
    NativeHdmiPresentation {
        mode,
        show_gridlines: false,
        cycle_measures: 4,
        source_layer_index: 0,
        source_behavior_id: "life".into(),
        grid: NativeGridPresentation {
            width: 8,
            height: 8,
            rgb: color.repeat(64),
            active: vec![false; 64],
        },
    }
}

#[test]
fn failed_write_retries_and_none_mode_restores_text() {
    let (mut device, control) = fake_device();
    control.lock().unwrap().fail_write = 1;
    let red = presentation([255, 0, 0], NativeHdmiMode::LiveGrid);
    let now = Instant::now();
    let next = device.render(&red, now).retry_at.unwrap();
    assert!(device.render(&red, next).applied);
    assert_eq!(control.lock().unwrap().writes.len(), 1);
    assert!(
        device
            .render(&presentation([0, 0, 0], NativeHdmiMode::Terminal), next)
            .applied
    );
    assert!(events(&control).ends_with(&["close_fb".into(), "close_tty".into()]));
}

fn events(control: &SharedControl) -> Vec<String> {
    control.lock().unwrap().events.clone()
}

#[test]
fn terminal_startup_does_not_touch_devices() {
    let (mut device, control) = fake_device();
    let outcome = device.render(
        &presentation([1, 2, 3], NativeHdmiMode::Terminal),
        Instant::now(),
    );
    assert!(outcome.applied);
    assert!(events(&control).is_empty());
}

#[test]
fn graphics_to_terminal_restores_without_black_frame() {
    let (mut device, control) = fake_device();
    let now = Instant::now();
    assert!(
        device
            .render(&presentation([255, 0, 0], NativeHdmiMode::LiveGrid), now)
            .applied
    );
    assert_eq!(
        events(&control),
        vec![
            "open_fb",
            "fb_var",
            "fb_fix",
            "open_tty",
            "kd_graphics",
            "fb_unblank",
            "write"
        ]
    );
    control.lock().unwrap().events.clear();
    assert!(
        device
            .render(&presentation([0, 0, 0], NativeHdmiMode::Terminal), now)
            .applied
    );
    assert_eq!(
        events(&control),
        vec!["kd_text", "fb_unblank", "close_fb", "close_tty"]
    );
    assert_eq!(control.lock().unwrap().writes.len(), 1);
}

#[test]
fn missing_framebuffer_retries_once_and_uses_latest_snapshot_after_hotplug() {
    let (mut device, control) = fake_device();
    control.lock().unwrap().framebuffer_available = false;
    let start = Instant::now();
    let first = device.render(&presentation([255, 0, 0], NativeHdmiMode::LiveGrid), start);
    let retry_at = first.retry_at.unwrap();
    assert_eq!(events(&control), Vec::<String>::new());
    assert!(device
        .render(
            &presentation([0, 255, 0], NativeHdmiMode::LiveGrid),
            retry_at - Duration::from_millis(1)
        )
        .retry_at
        .is_some());
    assert!(events(&control).is_empty());
    control.lock().unwrap().framebuffer_available = true;
    assert!(
        device
            .render(
                &presentation([0, 0, 255], NativeHdmiMode::LiveGrid),
                retry_at
            )
            .applied
    );
    let control = control.lock().unwrap();
    assert_eq!(control.writes.len(), 1);
    assert_eq!(&control.writes[0][0..4], &[255, 0, 0, 0]);
}

#[test]
fn post_graphics_failure_restores_text_before_waiting() {
    let (mut device, control) = fake_device();
    control.lock().unwrap().fail_unblank = 1;
    let outcome = device.render(
        &presentation([255, 0, 0], NativeHdmiMode::LiveGrid),
        Instant::now(),
    );
    assert!(!outcome.applied);
    assert_eq!(
        events(&control),
        vec![
            "open_fb",
            "fb_var",
            "fb_fix",
            "open_tty",
            "kd_graphics",
            "fb_unblank",
            "kd_text",
            "fb_unblank",
            "close_fb",
            "close_tty"
        ]
    );
}

#[test]
fn failed_restoration_retains_retryable_lease() {
    let (mut device, control) = fake_device();
    {
        let mut control = control.lock().unwrap();
        control.fail_write = 1;
        control.fail_text = 1;
    }
    let start = Instant::now();
    let first = device.render(&presentation([255, 0, 0], NativeHdmiMode::LiveGrid), start);
    assert!(!first.applied);
    assert!(!events(&control).contains(&"close_fb".into()));
    let retry_at = first.retry_at.unwrap();
    assert!(
        device
            .render(
                &presentation([0, 0, 255], NativeHdmiMode::LiveGrid),
                retry_at
            )
            .applied
    );
    let events = events(&control);
    assert!(events
        .windows(2)
        .any(|pair| pair == ["kd_text", "fb_unblank"]));
    assert!(events.contains(&"close_fb".into()));
}

#[test]
fn drop_restores_an_active_lease() {
    let (mut device, control) = fake_device();
    assert!(
        device
            .render(
                &presentation([255, 0, 0], NativeHdmiMode::LiveGrid),
                Instant::now()
            )
            .applied
    );
    drop(device);
    let events = events(&control);
    assert!(events.ends_with(&[
        "kd_text".into(),
        "fb_unblank".into(),
        "close_fb".into(),
        "close_tty".into()
    ]));
}

#[test]
fn signature_is_not_applied_before_a_successful_write() {
    let (mut device, control) = fake_device();
    control.lock().unwrap().fail_write = 1;
    let outcome = device.render(
        &presentation([255, 0, 0], NativeHdmiMode::LiveGrid),
        Instant::now(),
    );
    assert!(!outcome.applied);
    assert!(control.lock().unwrap().writes.is_empty());
}
