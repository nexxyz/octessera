use crate::audio::AudioManager;
use crate::hardware_init::{init_encoders, init_hardware, HardwareDevices};
use crate::main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};
use crate::render::HardwareRenderTargets;
use crate::render_loop::RenderWorker;
use crate::utility_mode::UtilityMode;
use crate::{
    boot_oled_handoff, exit_code, hardware_fault, hardware_test, oled_utility, profiling_modes,
    raspberry_runtime, render, rpi_oled_handoff_runtime, seesaw_io, store_fault,
};
use octessera_pi::board_profile;
use std::process::ExitCode;

pub(crate) fn run_utility_mode(utility_mode: UtilityMode) -> Option<ExitCode> {
    if utility_mode == UtilityMode::Normal && board_profile::metadata_requested() {
        board_profile::print_build_metadata();
        return Some(ExitCode::SUCCESS);
    }
    match utility_mode {
        UtilityMode::InteractiveHardware => {
            std::process::exit(exit_code(hardware_test::run_interactive()))
        }
        UtilityMode::InteractiveNoise => {
            std::process::exit(exit_code(hardware_test::run_noise_only()))
        }
        UtilityMode::Normal => {}
    }
    profiling_modes::run_requested();
    if oled_utility::requested() {
        std::process::exit(exit_code(oled_utility::run()));
    }
    None
}

pub(crate) fn run_normal_startup() {
    let _ = simple_logger::init();

    let handoff_mode = match boot_oled_handoff::mode_from_env() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = board_profile::validate_runtime_profile() {
        eprintln!("{error}");
        std::process::exit(2);
    }

    println!("octessera - Pi native runtime");

    let store_dir = default_store_dir();
    crate::user_data_restore::finish_interrupted_restore_at_boot();
    let boot_handoff = handoff_mode == boot_oled_handoff::HandoffMode::V1;
    let (usb_config, audio_optimization) =
        store_fault::read_boot_runtime_config_or_restore(&store_dir, boot_handoff);
    let hardware = match init_hardware(!boot_handoff) {
        Ok(devices) => devices,
        Err(fault) => hardware_fault::run_hardware_fault_mode(fault, boot_handoff),
    };
    let (event_rx, _encoders) = match init_encoders() {
        Ok(encoders) => encoders,
        Err(mut fault) => {
            fault.attach_outputs(hardware.oled, Some(hardware.trellis), Some(hardware.neokey));
            hardware_fault::run_hardware_fault_mode(fault, boot_handoff);
        }
    };
    let HardwareDevices {
        _i2c_bus,
        oled,
        trellis,
        neokey,
        input_interrupt,
        _dac,
    } = hardware;
    let seesaw_io = seesaw_io::spawn_interrupt(trellis, neokey, input_interrupt);
    let audio = match init_audio(audio_optimization, usb_config.audio_outputs) {
        Ok(audio) => audio,
        Err(error) => {
            let mut fault = hardware_fault::HardwareFault::new();
            fault.push("AUDIO", format!("Audio init failed: {error}"));
            fault.attach_outputs(oled, None, None);
            fault.attach_wake_encoders(event_rx, _encoders);
            hardware_fault::run_hardware_fault_mode(Box::new(fault), boot_handoff);
        }
    };
    #[cfg(board_raspberry)]
    let mut audio = audio;
    #[cfg(board_raspberry)]
    let audio_load_rx = audio
        .as_mut()
        .and_then(AudioManager::take_load_status_receiver);

    let (midi_handler, midi_rx) = crate::input::midi_input_channel();

    let samples_dir = default_samples_dir();
    ensure_runtime_dirs(&store_dir, &samples_dir);
    let runtime_config = raspberry_runtime::RaspberryRuntimeConfig {
        audio: audio.as_ref().map(AudioManager::service),
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled: usb_config.midi_out_enabled,
        audio_outputs: usb_config.audio_outputs,
        usb_data_role: usb_config.data_role,
        audio_optimization,
        #[cfg(board_raspberry)]
        audio_load_rx,
        midi_rx,
        keyboard: seesaw_io.spawn_keyboard(usb_config.data_role),
        input_rx: seesaw_io.input_rx,
        encoder_rx: event_rx,
        early_boot_splash: handoff_mode == boot_oled_handoff::HandoffMode::V1,
    };
    let hdmi = render::hdmi::HdmiFramebuffer::new();
    if handoff_mode == boot_oled_handoff::HandoffMode::V1 {
        rpi_oled_handoff_runtime::run(runtime_config, seesaw_io.command_tx.clone(), hdmi);
    } else {
        let oled = oled.expect("direct startup must initialize OLED");
        #[cfg(test)]
        let oled = render::test_oled_output::real_oled_output(oled);
        let render_worker = RenderWorker::spawn(HardwareRenderTargets {
            oled,
            seesaw_tx: seesaw_io.command_tx.clone(),
            oled_handoff: None,
            hdmi,
        });
        raspberry_runtime::run(runtime_config, render_worker);
    }
}

fn init_audio(
    audio_optimization: playback_runtime::AudioOptimization,
    audio_outputs: playback_runtime::AudioOutputSet,
) -> Result<Option<AudioManager>, String> {
    match AudioManager::new(audio_optimization, audio_outputs) {
        Ok(audio) => {
            audio.service().ensure_route_readiness()?;
            println!("Audio ready");
            Ok(Some(audio))
        }
        Err(error) => Err(error),
    }
}
