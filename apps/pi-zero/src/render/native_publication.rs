use super::{HardwareRenderCache, HardwareRenderTargets};
use crate::oled_frame_cache::OledFramePublication;
use crate::render_loop_queue::{NativeSceneCommand, NativeSceneCompletion};
use playback_runtime::NativeHardwarePresentation;
use serde_json::Value;
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Instant;

#[cfg(test)]
mod tests;

pub(crate) enum LatestPresentation {
    Legacy {
        snapshot: Value,
        oled: OledFramePublication,
    },
    Native {
        hardware: Box<NativeHardwarePresentation>,
        oled: OledFramePublication,
        completion: Option<mpsc::Sender<NativeSceneCompletion>>,
    },
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
        let revision = super::oled_output::physical_oled_revision(&pixels, 1, cache)?;
        let oled = OledFramePublication::native_pixels(revision, pixels)?;
        Ok(LatestPresentation::Native {
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

pub(crate) fn prepare_legacy_presentation(
    snapshot: Value,
    oled: OledFramePublication,
    cache: &HardwareRenderCache,
) -> Result<LatestPresentation, String> {
    let physical = super::oled_output::physical_oled_publication(&oled, cache)?;
    Ok(LatestPresentation::Legacy {
        snapshot,
        oled: physical,
    })
}

impl LatestPresentation {
    pub(crate) fn revision(&self) -> Option<u64> {
        match self {
            Self::Legacy { oled, .. } | Self::Native { oled, .. } => oled.revision(),
        }
    }

    pub(crate) fn oled_is_accepted(&self, cache: &HardwareRenderCache) -> bool {
        match self {
            Self::Legacy { snapshot, oled } => super::oled_output::oled_publication_is_accepted(
                oled,
                super::snapshot_display_off(snapshot),
                cache,
            ),
            Self::Native { hardware, oled, .. } => {
                super::oled_output::oled_publication_is_accepted(oled, hardware.leds.off, cache)
            }
        }
    }

    pub(crate) fn shared_oled_frame(&self) -> Option<(u64, Arc<[u8]>)> {
        let oled = match self {
            Self::Legacy { oled, .. } | Self::Native { oled, .. } => oled,
        };
        Some((oled.revision()?, oled.shared_pixels()?))
    }

    pub(crate) fn force_oled(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
    ) -> Result<(), String> {
        match self {
            Self::Legacy { snapshot, oled } => {
                super::force_latest_oled(targets, snapshot, oled, cache)
            }
            Self::Native { hardware, oled, .. } => super::oled_output::force_oled_typed(
                &mut targets.oled,
                hardware.leds.off,
                oled,
                cache,
            ),
        }
    }

    pub(crate) fn render_leds(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
        now: Instant,
    ) -> Option<Instant> {
        match self {
            Self::Legacy { snapshot, .. } => super::render_leds_only(targets, snapshot, cache, now),
            Self::Native { hardware, .. } => {
                super::native_leds::render_native_leds_at(targets, hardware, cache, now)
            }
        }
    }

    pub(crate) fn render_oled_leds(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
        now: Instant,
    ) -> Option<Instant> {
        match self {
            Self::Legacy { snapshot, oled } => {
                super::render_oled_and_leds_cached(targets, snapshot, oled, cache)
            }
            Self::Native { hardware, oled, .. } => {
                let leds = super::native_leds::render_native_leds_at(targets, hardware, cache, now);
                let frame = super::oled_output::render_oled_typed_if_changed(
                    &mut targets.oled,
                    hardware.leds.off,
                    oled,
                    cache,
                    now,
                );
                super::next_deadline(leds, frame)
            }
        }
    }

    pub(crate) fn render_hdmi(
        &self,
        targets: &mut HardwareRenderTargets,
        cache: &mut HardwareRenderCache,
        now: Instant,
    ) -> Option<Instant> {
        match self {
            Self::Legacy { snapshot, .. } => {
                super::retry_hdmi_if_due(targets, snapshot, cache, now)
            }
            Self::Native { hardware, .. } => {
                let signature = super::hdmi::hdmi_signature_typed(&hardware.hdmi);
                if cache.hdmi_initialized
                    && signature == cache.hdmi_signature
                    && !targets.hdmi.has_pending_retry()
                {
                    return None;
                }
                let result = targets.hdmi.render_typed(&hardware.hdmi, now);
                if result.applied {
                    cache.hdmi_signature = signature;
                    cache.hdmi_initialized = true;
                }
                result.retry_at
            }
        }
    }

    pub(crate) fn complete_native(&mut self, result: Result<(), String>) {
        let revision = self.revision().unwrap_or(0);
        if let Self::Native {
            hardware,
            completion,
            ..
        } = self
        {
            if let Some(sender) = completion.take() {
                let frame_revision = result.as_ref().ok().map(|_| revision);
                let _ = sender.send(NativeSceneCompletion {
                    generation: hardware.generation,
                    result,
                    frame_revision,
                });
            }
        }
    }

    pub(crate) fn submit_recording_frame(
        &self,
        audio: &crate::audio::AudioService,
    ) -> Result<(), String> {
        match self {
            Self::Legacy { oled, .. } => {
                if let (Some(revision), Some(pixels)) = (oled.revision(), oled.shared_pixels()) {
                    audio.submit_accepted_oled_frame_shared(revision, pixels)?;
                } else {
                    audio.clear_latest_physical_oled_frame()?;
                }
            }
            Self::Native { oled, .. } => {
                if let (Some(revision), Some(pixels)) = (oled.revision(), oled.shared_pixels()) {
                    audio.submit_accepted_oled_frame_shared(revision, pixels)?;
                }
            }
        }
        Ok(())
    }
}
