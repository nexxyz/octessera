#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod input;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_candidate;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_device_apply;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod render;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod render_loop;
mod render_loop_queue;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod seesaw_io;

#[cfg(feature = "native-audio")]
mod audio;
#[cfg(feature = "native-audio")]
mod audio_config_parse;
#[cfg(feature = "native-audio")]
mod audio_event;
#[cfg(feature = "native-audio")]
mod audio_priority;
#[cfg(feature = "native-audio")]
mod audio_recording;
#[cfg(feature = "native-audio")]
mod audio_replay;
#[cfg(feature = "native-audio")]
mod audio_route;
#[cfg(feature = "native-audio")]
mod audio_sink_registry;
#[cfg(feature = "native-audio")]
mod audio_stream_health;
mod boot_oled_handoff;
mod candidate_readiness;
mod device_update;
mod diagnostics;
mod dsp_profile;
mod dsp_scenarios;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod encoder_queue;
mod fat_diagnostic;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_fault;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_init;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_test;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_test_noise;
#[cfg(feature = "native-audio")]
mod hdmi_connector;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod host_adapter;
#[cfg(feature = "native-audio")]
mod host_audio_command;
#[cfg(feature = "native-audio")]
mod host_audio_prep;
#[cfg(all(feature = "native-audio", not(feature = "hardware-orange-pi-zero-2w")))]
mod input;
#[cfg(feature = "native-audio")]
mod main_paths;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod main_runtime_loop;
#[cfg(feature = "external-midi")]
mod midi_host;
mod normal_menu;
mod oled_frame_cache;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod oled_test;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio_benchmark;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_host_adapter;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_oled_suspend;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_oled_suspend_policy;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_reboot;
mod persistence;
mod platform_service;
#[cfg(all(feature = "native-audio", not(feature = "hardware-orange-pi-zero-2w")))]
mod recording;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod render;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod render_loop;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod rpi_oled_handoff_runtime;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod runtime_loop;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod runtime_thread;
mod sample_browser;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod seesaw_io;
mod setup_portal;
mod setup_portal_files;
mod setup_portal_paths;
mod setup_portal_worker;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod snapshot_cadence;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod timing_probe;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod ui_profile;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[cfg(test)]
mod update_menu_fixture_tests;
#[cfg(feature = "native-audio")]
mod usb_config;
mod user_data_archive;
mod user_data_media_paths;
mod user_data_restore;
mod user_data_transfer;
mod utility_mode;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod wake_trace;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use audio::AudioManager;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use hardware_init::{init_encoders, init_hardware, HardwareDevices};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use input::{midi_realtime_message, MidiMessage};
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use render::HardwareRenderTargets;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use render_loop::RenderWorker;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::sync::mpsc;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use std::sync::Arc;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};
use octessera_pi::board_profile;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn main() {
    let utility_mode = match utility_mode::from_process() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("Utility mode error: {error}");
            std::process::exit(2);
        }
    };
    if utility_mode == utility_mode::UtilityMode::Normal
        && octessera_pi::board_profile::metadata_requested()
    {
        octessera_pi::board_profile::print_build_metadata();
        return;
    }
    match utility_mode {
        utility_mode::UtilityMode::LegacyDiagnostic => {
            std::process::exit(compatibility_diagnostic_exit_code())
        }
        utility_mode::UtilityMode::FatDiagnostic => std::process::exit(fat_diagnostic_exit_code()),
        utility_mode::UtilityMode::InteractiveHardware
        | utility_mode::UtilityMode::InteractiveNoise => {
            eprintln!(
                "interactive hardware test is only available on the canonical Raspberry build"
            );
            std::process::exit(2);
        }
        utility_mode::UtilityMode::Normal => {}
    }
    if dsp_profile::profile_requested() && orange_audio_benchmark::requested() {
        eprintln!("--profile-dsp and --benchmark-orange-audio cannot be combined");
        std::process::exit(2);
    }
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
    if orange_audio_benchmark::requested() {
        let result = orange_audio_benchmark::run();
        if let Err(error) = &result {
            eprintln!("Orange audio benchmark failed: {error}");
        }
        std::process::exit(exit_code(result.is_ok()));
    }
    if let Err(error) = orange_candidate::run() {
        eprintln!("Orange foreground candidate failed: {error}");
        std::process::exit(error.exit_code());
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn main() {
    let utility_mode = match utility_mode::from_process() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("Utility mode error: {error}");
            std::process::exit(2);
        }
    };
    if utility_mode == utility_mode::UtilityMode::Normal && board_profile::metadata_requested() {
        board_profile::print_build_metadata();
        return;
    }
    match utility_mode {
        utility_mode::UtilityMode::LegacyDiagnostic => {
            std::process::exit(compatibility_diagnostic_exit_code())
        }
        utility_mode::UtilityMode::FatDiagnostic => std::process::exit(fat_diagnostic_exit_code()),
        utility_mode::UtilityMode::InteractiveHardware => {
            std::process::exit(exit_code(hardware_test::run_interactive()))
        }
        utility_mode::UtilityMode::InteractiveNoise => {
            std::process::exit(exit_code(hardware_test::run_noise_only()))
        }
        utility_mode::UtilityMode::Normal => {}
    }

    let _ = simple_logger::init();

    let handoff_mode = match boot_oled_handoff::mode_from_env() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    run_requested_utility();

    if let Err(error) = board_profile::validate_runtime_profile() {
        eprintln!("{error}");
        std::process::exit(2);
    }

    println!("octessera - Pi native runtime");

    let hardware = match init_hardware(handoff_mode == boot_oled_handoff::HandoffMode::Legacy) {
        Ok(devices) => devices,
        Err(fault) => hardware_fault::run_hardware_fault_mode(fault),
    };
    let (event_rx, _encoders) = match init_encoders() {
        Ok(encoders) => encoders,
        Err(mut fault) => {
            fault.attach_outputs(hardware.oled, Some(hardware.trellis), Some(hardware.neokey));
            hardware_fault::run_hardware_fault_mode(fault);
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
    let store_dir = default_store_dir();
    let usb_config = match usb_config::read_usb_runtime_config(&store_dir) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("USB runtime configuration is unavailable: {error}");
            std::process::exit(2);
        }
    };
    let audio = match init_audio(
        usb_config::audio_output_buffer_frames_from_default_config(&store_dir),
        usb_config.audio_outputs,
    ) {
        Ok(audio) => audio,
        Err(error) => {
            eprintln!("Audio init failed: {error}");
            std::process::exit(2);
        }
    };

    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });

    let samples_dir = default_samples_dir();
    ensure_runtime_dirs(&store_dir, &samples_dir);
    let runtime_config = runtime_thread::RuntimeThreadConfig {
        audio: audio.as_ref().map(AudioManager::service),
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled: usb_config.midi_out_enabled,
        audio_outputs: usb_config.audio_outputs,
        midi_rx,
        input_rx: seesaw_io.input_rx,
        encoder_rx: event_rx,
        early_boot_splash: handoff_mode == boot_oled_handoff::HandoffMode::V1,
    };
    let hdmi = match render::hdmi::HdmiFramebuffer::open_from_env() {
        Ok(hdmi) => hdmi,
        Err(error) => {
            eprintln!("pi HDMI framebuffer disabled: {error}");
            None
        }
    };
    if handoff_mode == boot_oled_handoff::HandoffMode::V1 {
        rpi_oled_handoff_runtime::run(runtime_config, seesaw_io.command_tx.clone(), hdmi);
    } else {
        let oled = oled.expect("legacy startup must initialize OLED");
        let render_worker = RenderWorker::spawn(HardwareRenderTargets {
            oled,
            seesaw_tx: seesaw_io.command_tx.clone(),
            oled_handoff: None,
            hdmi,
        });
        let runtime = runtime_thread::spawn(runtime_config, render_worker);
        if runtime.join().is_err() {
            eprintln!("pi runtime thread panicked");
        }
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn run_requested_utility() {
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
    if timing_probe::requested() {
        std::process::exit(exit_code(timing_probe::run()));
    }
    if oled_test::requested() {
        std::process::exit(exit_code(oled_test::run()));
    }
}

fn fat_diagnostic_exit_code() -> i32 {
    diagnostic_result_exit_code(fat_diagnostic::run())
}

fn compatibility_diagnostic_exit_code() -> i32 {
    diagnostic_result_exit_code(diagnostics::run_pre_hardware_diagnostics())
}

fn diagnostic_result_exit_code(result: Result<bool, String>) -> i32 {
    match result {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(error) if error == "help requested" => 0,
        Err(error) => {
            eprintln!("FAT diagnostic argument/error: {error}");
            2
        }
    }
}

fn exit_code(success: bool) -> i32 {
    if success {
        0
    } else {
        1
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn init_audio(
    output_buffer_frames: Option<u32>,
    audio_outputs: playback_runtime::AudioOutputSet,
) -> Result<Option<AudioManager>, String> {
    match AudioManager::new(output_buffer_frames, audio_outputs) {
        Ok(audio) => {
            audio.service().ensure_route_readiness()?;
            println!("Audio ready");
            Ok(Some(audio))
        }
        Err(error) => Err(error),
    }
}
