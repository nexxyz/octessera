use super::*;
use std::convert::TryFrom;
use std::fs::{File, OpenOptions};
use std::io::{self, Seek, SeekFrom, Write};
use std::os::fd::AsRawFd;

const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

#[repr(C)]
#[derive(Default)]
struct FbBitfield {
    offset: u32,
    length: u32,
    msb_right: u32,
}

#[repr(C)]
#[derive(Default)]
struct FbVarScreenInfo {
    xres: u32,
    yres: u32,
    xres_virtual: u32,
    yres_virtual: u32,
    xoffset: u32,
    yoffset: u32,
    bits_per_pixel: u32,
    grayscale: u32,
    red: FbBitfield,
    green: FbBitfield,
    blue: FbBitfield,
    transp: FbBitfield,
    nonstd: u32,
    activate: u32,
    height: u32,
    width: u32,
    accel_flags: u32,
    pixclock: u32,
    left_margin: u32,
    right_margin: u32,
    upper_margin: u32,
    lower_margin: u32,
    hsync_len: u32,
    vsync_len: u32,
    sync: u32,
    vmode: u32,
    rotate: u32,
    colorspace: u32,
    reserved: [u32; 4],
}

#[repr(C)]
#[derive(Default)]
struct FbFixScreenInfo {
    id: [libc::c_char; 16],
    smem_start: libc::c_ulong,
    smem_len: u32,
    fb_type: u32,
    type_aux: u32,
    visual: u32,
    xpanstep: u16,
    ypanstep: u16,
    ywrapstep: u16,
    line_length: u32,
    mmio_start: libc::c_ulong,
    mmio_len: u32,
    accel: u32,
    capabilities: u16,
    reserved: [u16; 2],
}

struct FramebufferGeometry {
    width: usize,
    height: usize,
    stride: usize,
    bytes_per_pixel: usize,
}

pub struct HdmiFramebuffer {
    file: Option<File>,
    path: String,
    width: usize,
    height: usize,
    stride: usize,
    bytes_per_pixel: usize,
}

impl HdmiFramebuffer {
    pub fn open_from_env() -> Result<Option<Self>, HdmiError> {
        if std::env::var("OCTESSERA_HDMI_DISABLE").ok().as_deref() == Some("1") {
            return Ok(None);
        }
        let path = std::env::var("OCTESSERA_HDMI_FB").unwrap_or_else(|_| "/dev/fb0".into());
        let (file, geometry) = open_framebuffer(&path)?;
        Ok(Some(Self::from_geometry(path, file, geometry)))
    }

    pub fn render(&mut self, snapshot: &Value) -> Result<(), HdmiError> {
        if hdmi_mode(snapshot) == Some("none") {
            return self.blank_and_close();
        }
        if self.file.is_none() {
            let (file, geometry) = open_framebuffer(&self.path)?;
            *self = Self::from_geometry(self.path.clone(), file, geometry);
        }
        let Some(frame) = compose_frame_with_stride(
            snapshot,
            self.width,
            self.height,
            self.stride,
            self.bytes_per_pixel,
        ) else {
            return Ok(());
        };
        let Some(file) = self.file.as_mut() else {
            return Ok(());
        };
        file.seek(SeekFrom::Start(0))
            .map_err(|source| HdmiError::Io {
                operation: "seek",
                path: self.path.clone(),
                source,
            })?;
        file.write_all(&frame).map_err(|source| HdmiError::Io {
            operation: "write",
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }

    fn from_geometry(path: String, file: File, geometry: FramebufferGeometry) -> Self {
        Self {
            file: Some(file),
            path,
            width: geometry.width,
            height: geometry.height,
            stride: geometry.stride,
            bytes_per_pixel: geometry.bytes_per_pixel,
        }
    }

    fn blank_and_close(&mut self) -> Result<(), HdmiError> {
        if let Some(mut file) = self.file.take() {
            let frame = vec![0_u8; self.stride * self.height];
            file.seek(SeekFrom::Start(0))
                .map_err(|source| HdmiError::Io {
                    operation: "seek",
                    path: self.path.clone(),
                    source,
                })?;
            file.write_all(&frame).map_err(|source| HdmiError::Io {
                operation: "write",
                path: self.path.clone(),
                source,
            })?;
        }
        Ok(())
    }
}

fn open_framebuffer(path: &str) -> Result<(File, FramebufferGeometry), HdmiError> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|source| HdmiError::Io {
            operation: "open",
            path: path.to_owned(),
            source,
        })?;
    let geometry = discover_geometry(&file, path)?;
    Ok((file, geometry))
}

fn discover_geometry(file: &File, path: &str) -> Result<FramebufferGeometry, HdmiError> {
    let mut variable = FbVarScreenInfo::default();
    let mut fixed = FbFixScreenInfo::default();
    unsafe {
        if libc::ioctl(
            file.as_raw_fd(),
            FBIOGET_VSCREENINFO,
            &mut variable as *mut FbVarScreenInfo,
        ) < 0
        {
            return Err(io_error(
                "query variable geometry for",
                path,
                std::io::Error::last_os_error(),
            ));
        }
        if libc::ioctl(
            file.as_raw_fd(),
            FBIOGET_FSCREENINFO,
            &mut fixed as *mut FbFixScreenInfo,
        ) < 0
        {
            return Err(io_error(
                "query fixed geometry for",
                path,
                std::io::Error::last_os_error(),
            ));
        }
    }

    if !supported_format(&variable) {
        return Err(HdmiError::UnsupportedFormat {
            path: path.to_owned(),
            bits_per_pixel: variable.bits_per_pixel,
            red: (variable.red.offset, variable.red.length),
            green: (variable.green.offset, variable.green.length),
            blue: (variable.blue.offset, variable.blue.length),
            transp: (variable.transp.offset, variable.transp.length),
        });
    }
    let width =
        usize::try_from(variable.xres).map_err(|_| invalid_geometry(path, &variable, &fixed))?;
    let height =
        usize::try_from(variable.yres).map_err(|_| invalid_geometry(path, &variable, &fixed))?;
    let bytes_per_pixel = usize::try_from(variable.bits_per_pixel / 8)
        .map_err(|_| invalid_geometry(path, &variable, &fixed))?;
    let stride = usize::try_from(fixed.line_length)
        .map_err(|_| invalid_geometry(path, &variable, &fixed))?;
    let minimum_stride = width
        .checked_mul(bytes_per_pixel)
        .ok_or_else(|| invalid_geometry(path, &variable, &fixed))?;
    let frame_bytes = stride
        .checked_mul(height)
        .ok_or_else(|| invalid_geometry(path, &variable, &fixed))?;
    let memory_bytes =
        usize::try_from(fixed.smem_len).map_err(|_| invalid_geometry(path, &variable, &fixed))?;
    if width == 0 || height == 0 || stride < minimum_stride || frame_bytes > memory_bytes {
        return Err(invalid_geometry(path, &variable, &fixed));
    }
    Ok(FramebufferGeometry {
        width,
        height,
        stride,
        bytes_per_pixel,
    })
}

fn supported_format(variable: &FbVarScreenInfo) -> bool {
    let bitfield = |field: &FbBitfield| field.msb_right == 0;
    if !bitfield(&variable.red)
        || !bitfield(&variable.green)
        || !bitfield(&variable.blue)
        || !bitfield(&variable.transp)
    {
        return false;
    }
    match variable.bits_per_pixel {
        16 => {
            variable.red.offset == 11
                && variable.red.length == 5
                && variable.green.offset == 5
                && variable.green.length == 6
                && variable.blue.offset == 0
                && variable.blue.length == 5
                && variable.transp.length == 0
        }
        32 => {
            variable.red.offset == 16
                && variable.red.length == 8
                && variable.green.offset == 8
                && variable.green.length == 8
                && variable.blue.offset == 0
                && variable.blue.length == 8
                && variable.transp.length == 0
        }
        _ => false,
    }
}

fn invalid_geometry(path: &str, variable: &FbVarScreenInfo, fixed: &FbFixScreenInfo) -> HdmiError {
    HdmiError::InvalidGeometry {
        path: path.to_owned(),
        width: variable.xres,
        height: variable.yres,
        stride: fixed.line_length,
        bits_per_pixel: variable.bits_per_pixel,
    }
}

fn io_error(operation: &'static str, path: &str, source: io::Error) -> HdmiError {
    HdmiError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn variable(bits_per_pixel: u32) -> FbVarScreenInfo {
        FbVarScreenInfo {
            bits_per_pixel,
            red: FbBitfield {
                offset: if bits_per_pixel == 16 { 11 } else { 16 },
                length: if bits_per_pixel == 16 { 5 } else { 8 },
                ..Default::default()
            },
            green: FbBitfield {
                offset: if bits_per_pixel == 16 { 5 } else { 8 },
                length: if bits_per_pixel == 16 { 6 } else { 8 },
                ..Default::default()
            },
            blue: FbBitfield {
                offset: 0,
                length: if bits_per_pixel == 16 { 5 } else { 8 },
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn supported_formats_are_explicit() {
        assert!(supported_format(&variable(16)));
        assert!(!supported_format(&variable(24)));

        let mut format = variable(32);
        format.blue.length = 8;
        assert!(supported_format(&format));
        format.transp.length = 8;
        assert!(!supported_format(&format));
    }
}
