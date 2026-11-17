use super::{changed_tile_rows, compose_frame_with_stride, HdmiError};
use playback_runtime::{NativeHdmiMode, NativeHdmiPresentation};
use std::fmt;
use std::mem;
use std::time::{Duration, Instant};

pub(crate) const HDMI_RETRY_INTERVAL: Duration = Duration::from_secs(1);
const LEASE_VT: u32 = 1;
const CONSOLE_VT: u32 = 2;

#[derive(Clone, Copy)]
pub(crate) struct FramebufferGeometry {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) stride: usize,
    pub(crate) bytes_per_pixel: usize,
}

pub(crate) trait FramebufferHandle: Send {
    fn geometry(&self) -> FramebufferGeometry;
    fn unblank(&mut self) -> Result<(), std::io::Error>;
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), std::io::Error>;
    fn write_rows(
        &mut self,
        first_offset: usize,
        stride: usize,
        rows: usize,
        row: &[u8],
    ) -> Result<(), std::io::Error>;
}

pub(crate) trait TtyHandle: Send {
    fn set_graphics(&mut self) -> Result<(), std::io::Error>;
    fn set_text(&mut self) -> Result<(), std::io::Error>;
    fn activate(&mut self, vt: u32) -> Result<(), std::io::Error>;
}

pub(crate) trait HdmiIo: Send {
    fn open_framebuffer(&mut self, path: &str) -> Result<Box<dyn FramebufferHandle>, HdmiError>;
    fn open_tty(&mut self, path: &str) -> Result<Box<dyn TtyHandle>, HdmiError>;
}

pub(crate) struct HdmiRenderOutcome {
    pub(crate) applied: bool,
    pub(crate) retry_at: Option<Instant>,
}

impl HdmiRenderOutcome {
    fn applied() -> Self {
        Self {
            applied: true,
            retry_at: None,
        }
    }

    fn waiting(retry_at: Instant) -> Self {
        Self {
            applied: false,
            retry_at: Some(retry_at),
        }
    }
}

enum DeviceState {
    Terminal,
    Waiting,
    Graphics(GraphicsLease),
}

struct GraphicsLease {
    framebuffer: Box<dyn FramebufferHandle>,
    tty: Box<dyn TtyHandle>,
    geometry: FramebufferGeometry,
    needs_restore: bool,
    drawn: Option<DrawnGrid>,
}

struct DrawnGrid {
    rgb: Vec<u8>,
    show_gridlines: bool,
}

impl DrawnGrid {
    fn of(presentation: &NativeHdmiPresentation) -> Self {
        Self {
            rgb: presentation.grid.rgb.clone(),
            show_gridlines: presentation.show_gridlines,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum LogState {
    Quiet,
    Waiting,
    Active,
}

pub(crate) struct HdmiDevice {
    io: Box<dyn HdmiIo>,
    framebuffer_path: String,
    tty_path: String,
    state: DeviceState,
    retry_at: Option<Instant>,
    log_state: LogState,
    console_active: bool,
}

impl HdmiDevice {
    pub(crate) fn new(
        io: Box<dyn HdmiIo>,
        framebuffer_path: impl Into<String>,
        tty_path: impl Into<String>,
    ) -> Self {
        Self {
            io,
            framebuffer_path: framebuffer_path.into(),
            tty_path: tty_path.into(),
            state: DeviceState::Terminal,
            retry_at: None,
            log_state: LogState::Quiet,
            console_active: false,
        }
    }

    pub(crate) fn has_pending_retry(&self) -> bool {
        self.retry_at.is_some()
    }

    pub(crate) fn render(
        &mut self,
        presentation: &NativeHdmiPresentation,
        now: Instant,
    ) -> HdmiRenderOutcome {
        super::assert_typed_grid(presentation);
        if presentation.mode == NativeHdmiMode::Terminal {
            return self.restore_terminal(now);
        }
        if let Some(retry_at) = self.retry_at {
            if now < retry_at {
                return HdmiRenderOutcome::waiting(retry_at);
            }
        }

        let state = mem::replace(&mut self.state, DeviceState::Terminal);
        match state {
            DeviceState::Graphics(lease) if !lease.needs_restore => {
                self.render_on_existing_lease(lease, presentation, now)
            }
            DeviceState::Graphics(lease) => {
                if !self.restore_lease(lease, now) {
                    return HdmiRenderOutcome::waiting(self.retry_deadline(now));
                }
                self.acquire_and_render(presentation, now)
            }
            DeviceState::Terminal | DeviceState::Waiting => {
                self.acquire_and_render(presentation, now)
            }
        }
    }

    fn restore_terminal(&mut self, now: Instant) -> HdmiRenderOutcome {
        let state = mem::replace(&mut self.state, DeviceState::Terminal);
        if let DeviceState::Graphics(lease) = state {
            self.retry_at = None;
            if !self.restore_lease(lease, now) {
                return HdmiRenderOutcome::waiting(self.retry_deadline(now));
            }
        }
        if self.console_active {
            self.retry_at = None;
            return HdmiRenderOutcome::applied();
        }
        if let Some(retry_at) = self.retry_at {
            if now < retry_at {
                return HdmiRenderOutcome::waiting(retry_at);
            }
        }
        let activated = self.io.open_tty(&self.tty_path).and_then(|mut tty| {
            tty.activate(CONSOLE_VT)
                .map_err(|error| io_error("activate console VT from", &self.tty_path, error))
        });
        match activated {
            Ok(()) => {
                self.console_active = true;
                self.retry_at = None;
                HdmiRenderOutcome::applied()
            }
            Err(error) => self.wait_for_retry(now, error),
        }
    }

    fn acquire_and_render(
        &mut self,
        presentation: &NativeHdmiPresentation,
        now: Instant,
    ) -> HdmiRenderOutcome {
        let framebuffer = match self.io.open_framebuffer(&self.framebuffer_path) {
            Ok(framebuffer) => framebuffer,
            Err(error) => return self.wait_for_retry(now, error),
        };
        let geometry = framebuffer.geometry();
        let Some(frame) = compose_frame(presentation, geometry) else {
            drop(framebuffer);
            return self.wait_for_retry(now, "HDMI frame composition produced no frame");
        };
        let tty = match self.io.open_tty(&self.tty_path) {
            Ok(tty) => tty,
            Err(error) => {
                drop(framebuffer);
                return self.wait_for_retry(now, error);
            }
        };
        let mut lease = GraphicsLease {
            framebuffer,
            tty,
            geometry,
            needs_restore: false,
            drawn: None,
        };
        if let Err(error) = lease.tty.activate(LEASE_VT) {
            drop(lease.framebuffer);
            drop(lease.tty);
            return self.wait_for_retry(now, io_error("activate", &self.tty_path, error));
        }
        self.console_active = false;
        if let Err(error) = lease.tty.set_graphics() {
            drop(lease.framebuffer);
            drop(lease.tty);
            return self.wait_for_retry(now, io_error("set KD_GRAPHICS", &self.tty_path, error));
        }
        if let Err(error) = lease.framebuffer.unblank() {
            return self.finish_failed_graphics(
                now,
                lease,
                io_error("unblank", &self.framebuffer_path, error),
            );
        }
        if let Err(error) = lease.framebuffer.write_frame(&frame) {
            return self.finish_failed_graphics(
                now,
                lease,
                io_error("write", &self.framebuffer_path, error),
            );
        }
        lease.drawn = Some(DrawnGrid::of(presentation));
        self.state = DeviceState::Graphics(lease);
        self.retry_at = None;
        self.mark_active();
        HdmiRenderOutcome::applied()
    }

    fn render_on_existing_lease(
        &mut self,
        mut lease: GraphicsLease,
        presentation: &NativeHdmiPresentation,
        now: Instant,
    ) -> HdmiRenderOutcome {
        let written = match lease.drawn.take() {
            Some(drawn) if drawn.show_gridlines == presentation.show_gridlines => {
                write_changed_tiles(
                    lease.framebuffer.as_mut(),
                    lease.geometry,
                    &drawn.rgb,
                    presentation,
                )
            }
            _ => match compose_frame(presentation, lease.geometry) {
                Some(frame) => lease.framebuffer.write_frame(&frame),
                None => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "HDMI frame composition produced no frame",
                )),
            },
        };
        if let Err(error) = written {
            return self.finish_failed_graphics(
                now,
                lease,
                io_error("write", &self.framebuffer_path, error),
            );
        }
        lease.drawn = Some(DrawnGrid::of(presentation));
        self.state = DeviceState::Graphics(lease);
        self.retry_at = None;
        HdmiRenderOutcome::applied()
    }

    fn finish_failed_graphics(
        &mut self,
        now: Instant,
        lease: GraphicsLease,
        error: HdmiError,
    ) -> HdmiRenderOutcome {
        if self.restore_lease(lease, now) {
            self.wait_for_retry(now, error)
        } else {
            HdmiRenderOutcome::waiting(self.retry_deadline(now))
        }
    }

    fn restore_lease(&mut self, mut lease: GraphicsLease, now: Instant) -> bool {
        let text_result = lease.tty.set_text();
        let unblank_result = lease.framebuffer.unblank();
        if let Err(error) = text_result {
            lease.needs_restore = true;
            self.state = DeviceState::Graphics(lease);
            self.mark_waiting(io_error("set KD_TEXT", &self.tty_path, error));
            self.retry_at = Some(now + HDMI_RETRY_INTERVAL);
            return false;
        }
        if let Err(error) = unblank_result {
            lease.needs_restore = true;
            self.state = DeviceState::Graphics(lease);
            self.mark_waiting(io_error("unblank", &self.framebuffer_path, error));
            self.retry_at = Some(now + HDMI_RETRY_INTERVAL);
            return false;
        }
        let GraphicsLease {
            framebuffer, tty, ..
        } = lease;
        drop(framebuffer);
        drop(tty);
        self.state = DeviceState::Terminal;
        self.retry_at = None;
        true
    }

    fn wait_for_retry(&mut self, now: Instant, error: impl fmt::Display) -> HdmiRenderOutcome {
        self.state = DeviceState::Waiting;
        self.mark_waiting(error);
        let retry_at = now + HDMI_RETRY_INTERVAL;
        self.retry_at = Some(retry_at);
        HdmiRenderOutcome::waiting(retry_at)
    }

    fn retry_deadline(&mut self, now: Instant) -> Instant {
        self.retry_at
            .get_or_insert(now + HDMI_RETRY_INTERVAL)
            .to_owned()
    }

    fn mark_waiting(&mut self, error: impl fmt::Display) {
        if self.log_state != LogState::Waiting {
            eprintln!("pi HDMI framebuffer waiting: {error}");
            self.log_state = LogState::Waiting;
        }
    }

    fn mark_active(&mut self) {
        if self.log_state != LogState::Active {
            eprintln!("pi HDMI framebuffer active");
            self.log_state = LogState::Active;
        }
    }
}

impl Drop for HdmiDevice {
    fn drop(&mut self) {
        let state = mem::replace(&mut self.state, DeviceState::Terminal);
        let DeviceState::Graphics(mut lease) = state else {
            return;
        };
        let _ = lease.tty.set_text();
        let _ = lease.framebuffer.unblank();
        let GraphicsLease {
            framebuffer, tty, ..
        } = lease;
        drop(framebuffer);
        drop(tty);
    }
}

fn compose_frame(
    presentation: &NativeHdmiPresentation,
    geometry: FramebufferGeometry,
) -> Option<Vec<u8>> {
    compose_frame_with_stride(
        presentation,
        geometry.width,
        geometry.height,
        geometry.stride,
        geometry.bytes_per_pixel,
    )
}

fn write_changed_tiles(
    framebuffer: &mut dyn FramebufferHandle,
    geometry: FramebufferGeometry,
    previous_rgb: &[u8],
    presentation: &NativeHdmiPresentation,
) -> Result<(), std::io::Error> {
    let tiles = changed_tile_rows(
        previous_rgb,
        presentation,
        geometry.width,
        geometry.height,
        geometry.stride,
        geometry.bytes_per_pixel,
    )
    .ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "HDMI tile layout unavailable",
        )
    })?;
    for tile in tiles {
        framebuffer.write_rows(tile.first_offset, geometry.stride, tile.rows, &tile.row)?;
    }
    Ok(())
}

fn io_error(operation: &'static str, path: &str, source: std::io::Error) -> HdmiError {
    HdmiError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests;
