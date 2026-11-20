use super::*;

pub(crate) fn run(
    mut audio: AudioManager,
    usb_config: crate::usb_config::UsbRuntimeConfig,
    audio_optimization: playback_runtime::AudioOptimization,
    midi_rx: Receiver<MidiMessage>,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    mut candidate_readiness: CandidateReadiness,
    hdmi: crate::render::hdmi::HdmiFramebuffer,
) -> Result<(), String> {
    let devices = ORANGE_PI_ZERO_2W_DEVICES;
    let mut fault = HardwareFault::new();
    let trellis = NeoTrellis::new_at(devices.i2c.path, devices.trellis_addrs)
        .map_err(|error| fault.push("TRELLIS", error))
        .ok();
    let neokey = NeoKey::new_at(devices.i2c.path, devices.neokey_addr)
        .map_err(|error| fault.push("NEOKEY", error))
        .ok();
    let encoders = init_encoders()
        .map_err(|error| fault.push("ENCODERS", error))
        .ok();
    let interrupt = SeesawInterrupt::new()
        .map_err(|error| fault.push("SEESAW_INT", error))
        .ok();
    let (trellis, neokey, (encoder_rx, _encoders), interrupt) =
        match (trellis, neokey, encoders, interrupt) {
            (Some(trellis), Some(neokey), Some(encoders), Some(interrupt)) => {
                (trellis, neokey, encoders, interrupt)
            }
            (trellis, neokey, encoders, _) => {
                fault.attach_outputs(None, trellis, neokey);
                if let Some((events, encoders)) = encoders {
                    fault.attach_wake_encoders(events, encoders);
                }
                enter_hardware_fault_mode(fault);
            }
        };
    let seesaw = match seesaw_io::spawn_interrupt(trellis, neokey, interrupt) {
        Ok(seesaw) => seesaw,
        Err(error) => {
            fault.push("SEESAW", error);
            fault.attach_wake_encoders(encoder_rx, _encoders);
            enter_hardware_fault_mode(fault);
        }
    };
    let keyboard = crate::keyboard_capture::KeyboardCapture::spawn(seesaw.input_tx.clone(), true);
    let prepared =
        match PiHostAdapter::new(audio.service(), midi_handler, usb_config.midi_out_enabled)
            .and_then(|host| {
                prepare_runtime(host, audio_optimization, true, Some(keyboard.control()))
            }) {
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
            let handoff_result =
                handoff.mark_unavailable_and_failed(StartupFatalCode::OledUnavailable);
            let _ = seesaw.shutdown();
            let error = match handoff_result {
                Ok(()) => format!("Orange OLED adoption failed: {error}"),
                Err(handoff_error) => {
                    format!(
                        "Orange OLED adoption failed: {error}; handoff failure: {handoff_error}"
                    )
                }
            };
            return Err(error);
        }
    };
    let render = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx: seesaw.command_tx.clone(),
        oled_handoff: Some(handoff),
        hdmi,
    });
    let mut prepared = prepared;
    if let Err(error) = publish_prepared_initial_scene(&mut prepared, &render) {
        let handoff_error = render.mark_oled_failed().err();
        let _ = render.abort();
        let _ = seesaw.shutdown();
        return Err(match handoff_error {
            Some(handoff_error) => format!(
                "Orange initial OLED render failed: {error}; handoff failure: {handoff_error}"
            ),
            None => format!("Orange initial OLED render failed: {error}"),
        });
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
        });
    }
    let suspend =
        match crate::orange_oled_suspend::OrangeOledSuspendCoordinator::spawn(render.clone()) {
            Ok(suspend) => suspend,
            Err(error) => {
                let _ = render.abort();
                let _ = seesaw.shutdown();
                return Err(format!("Orange OLED suspend coordinator failed: {error}"));
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
    let keyboard_result = keyboard.shutdown();
    let suspend_result = suspend.shutdown();
    let render_result = lifecycle::teardown_render(&result, &render);
    let seesaw_result = seesaw.shutdown();
    let result = result
        .and(suspend_result)
        .and(render_result)
        .and(keyboard_result)
        .and(seesaw_result);
    drop(audio);
    result
}
