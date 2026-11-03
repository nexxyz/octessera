use super::*;

#[derive(Default)]
struct WindowOled {
    writes: Vec<Option<Range<usize>>>,
}

impl OledRenderDevice for WindowOled {
    fn display_on(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn write_frame(&mut self, _frame: &[u8]) -> Result<(), String> {
        self.writes.push(None);
        Ok(())
    }

    fn write_rows(&mut self, _frame: &[u8], rows: Range<usize>) -> Result<(), String> {
        self.writes.push(Some(rows));
        Ok(())
    }

    fn display_off(&mut self) -> Result<(), String> {
        Ok(())
    }

    fn set_master_contrast(&mut self, _step: u8) -> Result<(), String> {
        Ok(())
    }
}

fn frame_with(rows: &[usize]) -> Vec<u8> {
    let mut frame = vec![0_u8; super::super::OLED_FRAME_BYTES];
    for row in rows {
        frame[row * OLED_ROW_BYTES + 7] = 0x5a;
    }
    frame
}

fn present(oled: &mut WindowOled, cache: &mut HardwareRenderCache, revision: u64, frame: Vec<u8>) {
    let publication = OledFramePublication::test_native(revision, frame);
    assert_eq!(
        render_oled_if_changed_off(oled, false, &publication, cache, Instant::now()),
        None
    );
}

#[test]
fn later_frames_write_only_the_span_of_rows_that_changed() {
    let mut oled = WindowOled::default();
    let mut cache = HardwareRenderCache::default();
    present(&mut oled, &mut cache, 1, frame_with(&[]));
    present(&mut oled, &mut cache, 2, frame_with(&[70, 80]));
    present(&mut oled, &mut cache, 3, frame_with(&[70, 80]));
    present(&mut oled, &mut cache, 4, frame_with(&[70, 120]));
    present(&mut oled, &mut cache, 5, frame_with(&[0, 127]));
    assert_eq!(oled.writes, [None, Some(70..81), Some(80..121), None]);
}

#[test]
fn a_forced_render_writes_the_whole_frame() {
    let mut oled = WindowOled::default();
    let mut cache = HardwareRenderCache::default();
    present(&mut oled, &mut cache, 1, frame_with(&[]));
    let publication = OledFramePublication::test_native(2, frame_with(&[9]));
    force_oled_render_off(&mut oled, false, &publication, &mut cache).unwrap();
    assert_eq!(oled.writes, [None, None]);
}
