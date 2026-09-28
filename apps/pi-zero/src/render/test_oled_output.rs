use super::oled_output::OledRenderDevice;
use octessera_hal::OledSsd1351;

pub(crate) enum TestOledOutput {
    Fake(FakeOledOutput),
    Real(OledSsd1351),
}

pub(crate) fn fake_oled_output() -> TestOledOutput {
    TestOledOutput::Fake(FakeOledOutput { failed_writes: 0 })
}

pub(crate) fn fake_oled_output_failing_writes(count: usize) -> TestOledOutput {
    TestOledOutput::Fake(FakeOledOutput {
        failed_writes: count,
    })
}

pub(crate) fn real_oled_output(oled: OledSsd1351) -> TestOledOutput {
    TestOledOutput::Real(oled)
}

impl TestOledOutput {
    pub(crate) fn display_off(&mut self) -> Result<(), String> {
        match self {
            Self::Fake(_) => Ok(()),
            Self::Real(oled) => oled.display_off(),
        }
    }

    pub(crate) fn detach_preserving(&mut self) -> Result<(), String> {
        match self {
            Self::Fake(_) => Ok(()),
            Self::Real(oled) => oled.detach_preserving(),
        }
    }

    pub(crate) fn reacquire_existing(&mut self) -> Result<(), String> {
        match self {
            Self::Fake(_) => Ok(()),
            Self::Real(oled) => oled.reacquire_existing(),
        }
    }
}

impl OledRenderDevice for TestOledOutput {
    fn display_on(&mut self) -> Result<(), String> {
        match self {
            Self::Fake(_) => Ok(()),
            Self::Real(oled) => oled.display_on(),
        }
    }

    fn write_frame(&mut self, frame: &[u8]) -> Result<(), String> {
        match self {
            Self::Fake(fake) => {
                if frame.len() != super::OLED_FRAME_BYTES {
                    return Err(format!("invalid OLED frame length: {}", frame.len()));
                }
                if fake.failed_writes > 0 {
                    fake.failed_writes -= 1;
                    return Err("test OLED write rejected".into());
                }
                Ok(())
            }
            Self::Real(oled) => oled.write_frame(frame),
        }
    }

    fn display_off(&mut self) -> Result<(), String> {
        match self {
            Self::Fake(_) => Ok(()),
            Self::Real(oled) => oled.display_off(),
        }
    }
}

pub(crate) struct FakeOledOutput {
    failed_writes: usize,
}
