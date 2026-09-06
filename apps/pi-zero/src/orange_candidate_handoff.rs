use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrangeStartupOperation {
    Trellis,
    Neokey,
    Controls,
    Audio,
    Oled,
}

pub(crate) fn startup_fatal_code(operation: OrangeStartupOperation) -> StartupFatalCode {
    match operation {
        OrangeStartupOperation::Trellis => StartupFatalCode::TrellisUnavailable,
        OrangeStartupOperation::Neokey => StartupFatalCode::NeokeyUnavailable,
        OrangeStartupOperation::Controls => StartupFatalCode::ControlsUnavailable,
        OrangeStartupOperation::Audio => StartupFatalCode::AudioUnavailable,
        OrangeStartupOperation::Oled => StartupFatalCode::OledUnavailable,
    }
}

pub(crate) fn run(
    mut audio: AudioManager,
    usb_config: crate::usb_config::UsbRuntimeConfig,
    audio_optimization: playback_runtime::AudioOptimization,
    midi_rx: Receiver<MidiMessage>,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    mut candidate_readiness: CandidateReadiness,
    hdmi: crate::render::hdmi::HdmiFramebuffer,
) -> Result<(), OrangeRunError> {
    let devices = ORANGE_PI_ZERO_2W_DEVICES;
    let trellis = match NeoTrellis::new_with_mode(
        devices.i2c.path,
        devices.trellis_addrs,
        SeesawInputMode::Polling,
    ) {
        Ok(trellis) => trellis,
        Err(error) => {
            return Err(startup_failure(
                crate::boot_oled_handoff::HandoffMode::V1,
                startup_fatal_code(OrangeStartupOperation::Trellis),
                format!("Orange NeoTrellis startup failed: {error}"),
            ))
        }
    };
    let neokey = match NeoKey::new_with_mode(
        devices.i2c.path,
        devices.neokey_addr,
        SeesawInputMode::Polling,
    ) {
        Ok(neokey) => neokey,
        Err(error) => {
            return Err(startup_failure(
                crate::boot_oled_handoff::HandoffMode::V1,
                startup_fatal_code(OrangeStartupOperation::Neokey),
                format!("Orange NeoKey startup failed: {error}"),
            ))
        }
    };
    let (encoder_rx, _encoders) = init_encoders().map_err(|error| {
        startup_failure(
            crate::boot_oled_handoff::HandoffMode::V1,
            startup_fatal_code(OrangeStartupOperation::Controls),
            format!("Orange encoder startup failed: {error}"),
        )
    })?;
    let seesaw = seesaw_io::spawn_polling(trellis, neokey).map_err(|error| {
        startup_failure(
            crate::boot_oled_handoff::HandoffMode::V1,
            startup_fatal_code(OrangeStartupOperation::Controls),
            format!("Orange control startup failed: {error}"),
        )
    })?;
    let prepared = match prepare_runtime(
        audio.service(),
        midi_handler,
        usb_config.midi_out_enabled,
        audio_optimization,
        true,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = seesaw.shutdown();
            return Err(startup_failure(
                crate::boot_oled_handoff::HandoffMode::V1,
                crate::boot_oled_handoff::StartupFatalCode::StartupFailed,
                format!("Orange runtime preparation failed: {error}"),
            ));
        }
    };
    let handoff = match crate::boot_oled_handoff::native_attach_after_startup_clear() {
        Ok(handoff) => handoff,
        Err(error) => {
            let _ = seesaw.shutdown();
            return Err(startup_failure(
                crate::boot_oled_handoff::HandoffMode::V1,
                crate::boot_oled_handoff::StartupFatalCode::StartupFailed,
                format!("Orange OLED boot handoff attach failed: {error}"),
            ));
        }
    };
    let oled = match OledSsd1351::adopt_existing() {
        Ok(oled) => oled,
        Err(error) => {
            let handoff_result = handoff
                .mark_unavailable_and_failed(startup_fatal_code(OrangeStartupOperation::Oled));
            let _ = seesaw.shutdown();
            let error = match handoff_result {
                Ok(()) => format!("Orange OLED adoption failed: {error}"),
                Err(handoff_error) => {
                    format!(
                        "Orange OLED adoption failed: {error}; handoff failure: {handoff_error}"
                    )
                }
            };
            return Err(error.into());
        }
    };
    let render = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx: seesaw.command_tx.clone(),
        oled_handoff: Some(handoff),
        hdmi,
    });
    let mut prepared = prepared;
    if let Err(error) = publish_prepared_acknowledged_snapshot(&mut prepared, &render) {
        let handoff_error = render.mark_oled_failed().err();
        let _ = render.abort();
        let _ = seesaw.shutdown();
        return Err(match handoff_error {
            Some(handoff_error) => format!(
                "Orange initial OLED render failed: {error}; handoff failure: {handoff_error}"
            ),
            None => format!("Orange initial OLED render failed: {error}"),
        }
        .into());
    }
    if let Err(error) = render.mark_first_menu_rendered() {
        let handoff_error = render.mark_oled_failed().err();
        let _ = render.abort();
        let _ = seesaw.shutdown();
        return Err(match handoff_error {
            Some(handoff_error) => format!(
                "Orange OLED handoff status failed: {error}; handoff failure: {handoff_error}"
            ),
            None => format!("Orange OLED handoff status failed: {error}"),
        }
        .into());
    }
    let suspend =
        match crate::orange_oled_suspend::OrangeOledSuspendCoordinator::spawn(render.clone()) {
            Ok(suspend) => suspend,
            Err(error) => {
                let _ = render.abort();
                let _ = seesaw.shutdown();
                return Err(format!("Orange OLED suspend coordinator failed: {error}").into());
            }
        };
    let result = run_prepared_runtime(
        prepared,
        &seesaw,
        &encoder_rx,
        &render,
        &mut audio,
        &mut candidate_readiness,
        midi_rx,
        true,
    );
    let suspend_result = suspend.shutdown();
    let render_result = lifecycle::teardown_render(&result, &render);
    let seesaw_result = seesaw.shutdown();
    let result = result
        .and_then(|resolution| {
            suspend_result
                .map(|_| resolution)
                .map_err(OrangeRunError::from)
        })
        .and_then(|resolution| {
            render_result
                .map(|_| resolution)
                .map_err(OrangeRunError::from)
        })
        .and_then(|resolution| {
            seesaw_result
                .map(|_| resolution)
                .map_err(OrangeRunError::from)
        });
    drop(audio);
    result.and_then(crate::orange_device_apply::finish_shutdown_resolution)
}
