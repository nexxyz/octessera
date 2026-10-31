use super::OledFramePublication;
use super::{HardwareRenderCache, HardwareRenderTargets};
use crate::render_loop_queue::{NativeSceneCommand, NativeSceneCompletion};
use playback_runtime::NativeHardwarePresentation;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

#[cfg(test)]
mod tests;

pub(crate) struct LatestPresentation {
    pub(crate) hardware: Box<NativeHardwarePresentation>,
    pub(crate) oled: OledFramePublication,
    pub(crate) completion: Option<mpsc::Sender<NativeSceneCompletion>>,
}

pub(crate) fn prepare_native_scene(
    command: Box<NativeSceneCommand>,
    cache: &HardwareRenderCache,
) -> Option<LatestPresentation> {
    let NativeSceneCommand {
        scene,
        metrics,
        error,
        completion,
    } = *command;
    let generation = scene.generation();
    let prepared = (|| {
        let hardware = scene.into_hardware_presentation(metrics, error);
        let pixels = playback_runtime::oled_frame::render_oled_frame(&hardware.oled);
        let revision = super::oled_output::physical_oled_revision(&pixels, cache)?;
        let master_contrast = playback_runtime::oled_frame::oled_master_contrast_step(
            hardware.oled.display_brightness,
        );
        let oled = OledFramePublication::native_pixels(revision, pixels, master_contrast)?;
        Ok(LatestPresentation {
            hardware: Box::new(hardware),
            oled,
            completion: Some(completion.clone()),
        })
    })();
    match prepared {
        Ok(latest) => Some(latest),
        Err(error) => {
            let _ = completion.send(NativeSceneCompletion {
                generation,
                result: Err(error),
                frame_revision: None,
            });
            None
        }
    }
}

impl LatestPresentation {
    pub(crate) fn revision(&self) -> u64 {
        self.oled.revision()
    }

    pub(crate) fn oled_is_accepted(&self, cache: &HardwareRenderCache) -> bool {
        super::oled_output::oled_publication_is_accepted(&self.oled, self.hardware.leds.off, cache)
    }

    pub(crate) fn shared_oled_frame(&self) -> (u64, Arc<[u8]>) {
        (self.oled.revision(), self.oled.shared_pixels())
    }

    pub(crate) fn force_oled(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
    ) -> Result<(), String> {
        super::oled_output::force_oled_typed(
            &mut targets.oled,
            self.hardware.leds.off,
            &self.oled,
            cache,
        )
    }

    pub(crate) fn render_leds(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
        now: Instant,
    ) -> Option<Instant> {
        super::native_leds::render_native_leds_at(targets, &self.hardware, cache, now)
    }

    pub(crate) fn render_oled_leds(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
        now: Instant,
    ) -> Option<Instant> {
        let leds = super::native_leds::render_native_leds_at(targets, &self.hardware, cache, now);
        let frame = super::oled_output::render_oled_typed_if_changed(
            &mut targets.oled,
            self.hardware.leds.off,
            &self.oled,
            cache,
            now,
        );
        super::next_deadline(leds, frame)
    }

    pub(crate) fn render_hdmi(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
        now: Instant,
    ) -> Option<Instant> {
        super::render_hdmi_if_changed(&mut targets.hdmi, &self.hardware.hdmi, cache, now)
    }

    pub(crate) fn complete_native(&mut self, result: Result<(), String>) {
        let revision = self.revision();
        if let Some(sender) = self.completion.take() {
            let frame_revision = result.as_ref().ok().map(|_| revision);
            let _ = sender.send(NativeSceneCompletion {
                generation: self.hardware.generation,
                result,
                frame_revision,
            });
        }
    }

    pub(crate) fn submit_recording_frame(
        &self,
        audio: &crate::audio::AudioService,
    ) -> Result<(), String> {
        let (revision, pixels) = self.shared_oled_frame();
        audio.submit_accepted_oled_frame_shared(revision, pixels)
    }

    pub(crate) fn submit_recording_leds(
        &self,
        audio: &crate::audio::AudioService,
    ) -> Result<(), String> {
        match recording_led_colors(&self.hardware) {
            Some(colors) => audio.submit_led_colors(colors),
            None => Ok(()),
        }
    }
}

fn recording_led_colors(
    hardware: &NativeHardwarePresentation,
) -> Option<media_recording::LedColors> {
    media_recording::LedColors::from_grid_rgb(
        &hardware.leds.grid.rgb,
        hardware.neo_key.colors,
        hardware.leds.off,
    )
}
