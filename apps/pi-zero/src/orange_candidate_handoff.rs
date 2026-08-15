use super::*;

pub(crate) fn run(
    mut audio: AudioManager,
    usb_config: crate::usb_config::UsbRuntimeConfig,
    midi_rx: Receiver<MidiMessage>,
    midi_handler: Arc<dyn Fn(Vec<u8>) + Send + Sync>,
    mut candidate_readiness: CandidateReadiness,
) -> Result<(), OrangeRunError> {
    let devices = ORANGE_PI_ZERO_2W_DEVICES;
    let trellis = NeoTrellis::new_with_mode(
        devices.i2c.path,
        devices.trellis_addrs,
        SeesawInputMode::Polling,
    )?;
    let neokey = NeoKey::new_with_mode(
        devices.i2c.path,
        devices.neokey_addr,
        SeesawInputMode::Polling,
    )?;
    let (encoder_rx, _encoders) = init_encoders()?;
    let seesaw = seesaw_io::spawn_polling(trellis, neokey)?;
    let prepared = match prepare_runtime(
        audio.service(),
        midi_handler,
        usb_config.midi_out_enabled,
        true,
    ) {
        Ok(prepared) => prepared,
        Err(error) => {
            let _ = seesaw.shutdown();
            return Err(format!("Orange runtime preparation failed: {error}").into());
        }
    };
    let handoff = match crate::boot_oled_handoff::native_attach() {
        Ok(handoff) => handoff,
        Err(error) => {
            let _ = seesaw.shutdown();
            return Err(format!("Orange OLED boot handoff attach failed: {error}").into());
        }
    };
    let oled = match OledSsd1351::adopt_existing() {
        Ok(oled) => oled,
        Err(error) => {
            handoff.mark_failed();
            let _ = seesaw.shutdown();
            return Err(format!("Orange OLED adoption failed: {error}").into());
        }
    };
    let render = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx: seesaw.command_tx.clone(),
        oled_handoff: Some(handoff),
    });
    let mut prepared = prepared;
    if let Err(error) = publish_prepared_acknowledged_snapshot(&mut prepared, &render) {
        let _ = render.mark_oled_failed();
        let _ = render.abort();
        let _ = seesaw.shutdown();
        return Err(format!("Orange initial OLED render failed: {error}").into());
    }
    if let Err(error) = render.mark_first_menu_rendered() {
        let _ = render.mark_oled_failed();
        let _ = render.abort();
        let _ = seesaw.shutdown();
        return Err(format!("Orange OLED handoff status failed: {error}").into());
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
