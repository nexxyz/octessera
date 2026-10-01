use playback_runtime::{NativeHdmiMode, NativeHdmiPresentation};
use serde_json::Value;
use std::fmt;
use std::io;

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
#[derive(Debug)]
pub enum HdmiError {
    Io {
        operation: &'static str,
        path: String,
        source: io::Error,
    },
    InvalidGeometry {
        path: String,
        width: u32,
        height: u32,
        stride: u32,
        bits_per_pixel: u32,
    },
    UnsupportedFormat {
        path: String,
        bits_per_pixel: u32,
        red: (u32, u32),
        green: (u32, u32),
        blue: (u32, u32),
        transp: (u32, u32),
    },
}

impl fmt::Display for HdmiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                source,
            } => write!(formatter, "{operation} framebuffer {path}: {source}"),
            Self::InvalidGeometry {
                path,
                width,
                height,
                stride,
                bits_per_pixel,
            } => write!(
                formatter,
                "invalid framebuffer geometry for {path}: {width}x{height}, stride {stride}, {bits_per_pixel} bpp"
            ),
            Self::UnsupportedFormat {
                path,
                bits_per_pixel,
                red,
                green,
                blue,
                transp,
            } => write!(
                formatter,
                "unsupported framebuffer format for {path}: {bits_per_pixel} bpp, red {red:?}, green {green:?}, blue {blue:?}, transparency {transp:?}"
            ),
        }
    }
}

impl std::error::Error for HdmiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::InvalidGeometry { .. } | Self::UnsupportedFormat { .. } => None,
        }
    }
}

#[path = "hdmi_linux_device.rs"]
pub(crate) mod device;

#[cfg(target_os = "linux")]
#[path = "hdmi_linux.rs"]
mod imp;

#[cfg(not(target_os = "linux"))]
mod imp {
    use super::*;
    use std::io;

    struct UnsupportedIo;

    impl device::HdmiIo for UnsupportedIo {
        fn open_framebuffer(
            &mut self,
            path: &str,
        ) -> Result<Box<dyn device::FramebufferHandle>, HdmiError> {
            Err(HdmiError::Io {
                operation: "open",
                path: path.to_owned(),
                source: io::Error::from_raw_os_error(libc::ENOSYS),
            })
        }

        fn open_tty(&mut self, path: &str) -> Result<Box<dyn device::TtyHandle>, HdmiError> {
            Err(HdmiError::Io {
                operation: "open",
                path: path.to_owned(),
                source: io::Error::from_raw_os_error(libc::ENOSYS),
            })
        }
    }

    pub struct HdmiFramebuffer {
        device: device::HdmiDevice,
    }

    impl HdmiFramebuffer {
        pub fn new() -> Self {
            Self {
                device: device::HdmiDevice::new(Box::new(UnsupportedIo), "/dev/fb0", "/dev/tty1"),
            }
        }

        #[cfg(test)]
        pub(crate) fn from_device(device: device::HdmiDevice) -> Self {
            Self { device }
        }

        pub(crate) fn has_pending_retry(&self) -> bool {
            self.device.has_pending_retry()
        }

        pub(crate) fn render(
            &mut self,
            snapshot: &Value,
            now: std::time::Instant,
        ) -> device::HdmiRenderOutcome {
            self.device
                .render(snapshot, hdmi_mode(snapshot) == Some("none"), now)
        }

        pub(crate) fn render_typed(
            &mut self,
            presentation: &NativeHdmiPresentation,
            now: std::time::Instant,
        ) -> device::HdmiRenderOutcome {
            self.device.render_typed(presentation, now)
        }
    }
}

pub use imp::HdmiFramebuffer;

pub fn compose_frame_with_stride(
    snapshot: &Value,
    width: usize,
    height: usize,
    stride: usize,
    bytes_per_pixel: usize,
) -> Option<Vec<u8>> {
    if hdmi_mode(snapshot) == Some("none") {
        return None;
    }
    let grid = snapshot.get("hdmi").and_then(|hdmi| hdmi.get("grid"))?;
    let rgb = grid.get("rgb").and_then(Value::as_array)?;
    let show_gridlines = snapshot
        .get("hdmi")
        .and_then(|hdmi| hdmi.get("showGridlines"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    compose_grid_frame_with_stride(
        width,
        height,
        stride,
        bytes_per_pixel,
        show_gridlines,
        |index| {
            [
                u8_at(rgb, index * 3),
                u8_at(rgb, index * 3 + 1),
                u8_at(rgb, index * 3 + 2),
            ]
        },
    )
}

pub fn compose_frame_with_stride_typed(
    presentation: &NativeHdmiPresentation,
    width: usize,
    height: usize,
    stride: usize,
    bytes_per_pixel: usize,
) -> Option<Vec<u8>> {
    assert_typed_grid(presentation);
    if presentation.mode == NativeHdmiMode::None {
        return None;
    }
    let rgb = &presentation.grid.rgb;
    compose_grid_frame_with_stride(
        width,
        height,
        stride,
        bytes_per_pixel,
        presentation.show_gridlines,
        |index| {
            let offset = index * 3;
            [rgb[offset], rgb[offset + 1], rgb[offset + 2]]
        },
    )
}

fn assert_typed_grid(presentation: &NativeHdmiPresentation) {
    assert_eq!((presentation.grid.width, presentation.grid.height), (8, 8));
    assert_eq!(presentation.grid.rgb.len(), 8 * 8 * 3);
    assert_eq!(presentation.grid.active.len(), 8 * 8);
}

fn compose_grid_frame_with_stride(
    width: usize,
    height: usize,
    stride: usize,
    bytes_per_pixel: usize,
    show_gridlines: bool,
    color_at: impl Fn(usize) -> [u8; 3],
) -> Option<Vec<u8>> {
    let minimum_stride = width.checked_mul(bytes_per_pixel)?;
    if stride < minimum_stride {
        return None;
    }
    let side = width.min(height);
    let cell = side / 8;
    if cell == 0 || (bytes_per_pixel != 2 && bytes_per_pixel != 4) {
        return None;
    }
    let square = cell * 8;
    let x0 = (width - square) / 2;
    let y0 = (height - square) / 2;
    let mut frame = vec![0_u8; stride.checked_mul(height)?];
    for gy in 0..8 {
        for gx in 0..8 {
            let index = gy * 8 + gx;
            let color = color_at(index);
            for py in 0..cell {
                for px in 0..cell {
                    if show_gridlines && (px == 0 || py == 0) {
                        continue;
                    }
                    let offset =
                        (y0 + gy * cell + py) * stride + (x0 + gx * cell + px) * bytes_per_pixel;
                    write_pixel(
                        &mut frame[offset..offset + bytes_per_pixel],
                        color,
                        bytes_per_pixel,
                    );
                }
            }
        }
    }
    Some(frame)
}

#[cfg(any(test, target_os = "linux"))]
pub(crate) fn supported_offsets(xoffset: u32, yoffset: u32) -> bool {
    xoffset == 0 && yoffset == 0
}

fn hdmi_mode(snapshot: &Value) -> Option<&str> {
    snapshot.get("hdmi")?.get("mode")?.as_str()
}

fn u8_at(values: &[Value], index: usize) -> u8 {
    values
        .get(index)
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(255) as u8
}

fn write_pixel(pixel: &mut [u8], color: [u8; 3], bytes_per_pixel: usize) {
    if bytes_per_pixel == 2 {
        let value = (u16::from(color[0] >> 3) << 11)
            | (u16::from(color[1] >> 2) << 5)
            | u16::from(color[2] >> 3);
        pixel.copy_from_slice(&value.to_ne_bytes());
    } else {
        pixel.copy_from_slice(&[color[2], color[1], color[0], 0]);
    }
}

pub fn hdmi_signature(snapshot: &Value) -> u64 {
    if hdmi_mode(snapshot) == Some("none") {
        return 0;
    }
    let bytes =
        serde_json::to_vec(snapshot.get("hdmi").unwrap_or(&Value::Null)).unwrap_or_default();
    bytes.iter().fold(0xcbf29ce484222325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    })
}

pub fn hdmi_signature_typed(presentation: &NativeHdmiPresentation) -> u64 {
    assert_typed_grid(presentation);
    if presentation.mode == NativeHdmiMode::None {
        return 0;
    }
    let mut hash = 0xcbf29ce484222325_u64;
    let flags = [
        u8::from(presentation.show_gridlines),
        presentation.cycle_measures,
    ];
    let source_index = presentation.source_layer_index.to_le_bytes();
    let mut active = [0_u8; 64];
    for (output, value) in active.iter_mut().zip(&presentation.grid.active) {
        *output = u8::from(*value);
    }
    for bytes in [
        presentation.mode.as_str().as_bytes(),
        &flags,
        &source_index,
        presentation.source_behavior_id.as_bytes(),
        &presentation.grid.rgb,
        &active,
    ] {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use playback_runtime::{NativeGridPresentation, NativeHdmiMode, NativeHdmiPresentation};

    fn typed_hdmi(rgb: Vec<u8>, mode: NativeHdmiMode, gridlines: bool) -> NativeHdmiPresentation {
        NativeHdmiPresentation {
            mode,
            show_gridlines: gridlines,
            cycle_measures: 4,
            source_layer_index: 1,
            source_behavior_id: "sequencer".into(),
            grid: NativeGridPresentation {
                width: 8,
                height: 8,
                rgb,
                active: vec![false; 64],
            },
        }
    }

    #[test]
    fn typed_hdmi_composition_matches_legacy_stride_and_gridlines() {
        let mut rgb = vec![0; 64 * 3];
        rgb[0..3].copy_from_slice(&[255, 0, 0]);
        rgb[7 * 8 * 3..7 * 8 * 3 + 3].copy_from_slice(&[0, 0, 255]);
        for gridlines in [false, true] {
            let legacy = serde_json::json!({"hdmi": {
                "mode": "cycle-behaviors", "sourceLayerIndex": 1,
                "sourceBehaviorId": "sequencer", "cycleMeasures": 4,
                "showGridlines": gridlines, "grid": {"rgb": rgb}
            }});
            let typed = typed_hdmi(rgb.clone(), NativeHdmiMode::CycleBehaviors, gridlines);
            let mut changed_legacy = legacy.clone();
            changed_legacy["hdmi"]["sourceLayerIndex"] = serde_json::json!(2);
            assert_ne!(hdmi_signature(&legacy), hdmi_signature(&changed_legacy));
            assert_eq!(
                compose_frame_with_stride_typed(&typed, 16, 8, 72, 4),
                compose_frame_with_stride(&legacy, 16, 8, 72, 4)
            );
            assert_ne!(hdmi_signature_typed(&typed), 0);
            let mut changed = typed_hdmi(rgb.clone(), NativeHdmiMode::CycleBehaviors, gridlines);
            assert_eq!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
            changed.source_layer_index = 2;
            assert_ne!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
            changed.source_layer_index = 1;
            changed.source_behavior_id = "life".into();
            assert_ne!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
            changed.source_behavior_id = "sequencer".into();
            changed.cycle_measures = 8;
            assert_ne!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
            changed.cycle_measures = 4;
            changed.mode = NativeHdmiMode::PlainGrid;
            assert_ne!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
            changed.mode = NativeHdmiMode::CycleBehaviors;
            changed.grid.active[0] = true;
            assert_ne!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
            changed.grid.active[0] = false;
            changed.show_gridlines = !gridlines;
            assert_ne!(hdmi_signature_typed(&typed), hdmi_signature_typed(&changed));
        }
        let none = typed_hdmi(rgb, NativeHdmiMode::None, false);
        assert_eq!(hdmi_signature_typed(&none), 0);
        assert!(compose_frame_with_stride_typed(&none, 16, 8, 72, 4).is_none());
    }

    #[test]
    fn none_mode_has_no_signature_or_frame() {
        let snapshot = serde_json::json!({
            "hdmi": {
                "mode": "none",
                "grid": {
                    "width": 8,
                    "height": 8,
                    "rgb": vec![255; 8 * 8 * 3],
                    "active": vec![true; 8 * 8]
                }
            }
        });

        assert_eq!(hdmi_signature(&snapshot), 0);
        assert!(compose_frame_with_stride(&snapshot, 64, 64, 256, 4).is_none());
    }

    #[test]
    fn stride_preserves_snapshot_row_order_and_padding() {
        let mut rgb = vec![0; 8 * 8 * 3];
        rgb[0..3].copy_from_slice(&[255, 0, 0]);
        let bottom_left = 7 * 8 * 3;
        rgb[bottom_left..bottom_left + 3].copy_from_slice(&[0, 0, 255]);
        let snapshot = serde_json::json!({
            "hdmi": {
                "mode": "live-grid",
                "grid": { "rgb": rgb },
                "showGridlines": false
            }
        });

        let frame = compose_frame_with_stride(&snapshot, 16, 8, 72, 4).unwrap();

        assert_eq!(&frame[4 * 4..4 * 4 + 4], &[0, 0, 255, 0]);
        assert_eq!(&frame[7 * 72 + 4 * 4..7 * 72 + 4 * 4 + 4], &[255, 0, 0, 0]);
        assert!(frame[16 * 4..72].iter().all(|byte| *byte == 0));
    }

    #[test]
    fn stride_and_pixel_format_are_required() {
        let snapshot = serde_json::json!({
            "hdmi": {
                "mode": "live-grid",
                "grid": { "rgb": vec![255; 8 * 8 * 3] }
            }
        });

        assert!(compose_frame_with_stride(&snapshot, 16, 8, 63, 4).is_none());
        assert!(compose_frame_with_stride(&snapshot, 16, 8, 64, 3).is_none());
    }

    #[test]
    fn framebuffer_offsets_are_supported_only_at_origin() {
        for (xoffset, yoffset, supported) in [
            (0, 0, true),
            (1, 0, false),
            (0, 1, false),
            (u32::MAX, u32::MAX, false),
        ] {
            assert_eq!(supported_offsets(xoffset, yoffset), supported);
        }
    }
}
