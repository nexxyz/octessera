#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod input;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_candidate;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_device_apply;
mod render;
mod render_loop;
mod render_loop_queue;
mod seesaw_io;

#[cfg(feature = "native-audio")]
mod audio;
#[cfg(feature = "native-audio")]
mod audio_config_parse;
#[cfg(feature = "native-audio")]
mod audio_engine_owner;
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
mod audio_stream_health;
#[cfg(feature = "native-audio")]
mod autoaux_menu;
#[cfg(feature = "native-audio")]
mod autoaux_sequence;
mod bluetooth;
mod boot_oled_handoff;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod boot_startup_delayed;
mod candidate_readiness;
mod device_update;
mod dsp_profile;
mod dsp_scenarios;
mod encoder_queue;
mod fat_diagnostic;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_fault;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_init;
mod hardware_runtime_scheduler;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_test;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_test_noise;
#[cfg(feature = "native-audio")]
mod hdmi_connector;
mod host_adapter;
#[cfg(feature = "native-audio")]
mod host_audio_command;
#[cfg(feature = "native-audio")]
mod host_audio_prep;
#[cfg(feature = "native-audio")]
mod initial_audio_prep;
#[cfg(all(feature = "native-audio", not(feature = "hardware-orange-pi-zero-2w")))]
mod input;
mod keyboard_capture;
#[cfg(any(
    feature = "hardware-orange-pi-zero-2w",
    all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    )
))]
mod live_audio_benchmark;
mod main_paths;
#[cfg(feature = "native-audio")]
mod main_runtime_loop;
#[cfg(feature = "external-midi")]
mod midi_host;
mod native_scene_pump;
mod normal_menu;
mod oled_frame_cache;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod oled_utility;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_oled_suspend;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_oled_suspend_policy;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_reboot;
mod persistence;
#[cfg(feature = "native-audio")]
mod pi_boot_config;
mod pi_host_core;
#[cfg(test)]
mod pi_store_test_support;
mod platform_service;
mod power_lifecycle;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod raspberry_power;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod raspberry_runtime;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod rpi_device_apply;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod rpi_oled_handoff_runtime;
mod runtime_loop;
#[cfg(feature = "native-audio")]
mod runtime_output;
mod runtime_thread;
mod sample_browser;
mod setup_portal;
mod setup_portal_files;
mod setup_portal_paths;
mod setup_portal_worker;
#[cfg(test)]
mod test_temp_dir;
#[cfg(feature = "native-audio")]
mod timing_input;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod timing_probe;
mod ui_profile;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[cfg(test)]
mod update_menu_fixture_tests;
#[cfg(feature = "native-audio")]
mod usb_config;
#[cfg(feature = "native-audio")]
mod usb_config_validation;
mod user_data_archive;
mod user_data_restore;
mod user_data_transfer;
mod utility_mode;
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

#[cfg(not(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
)))]
fn raspberry_benchmark_requested() -> bool {
    raspberry_benchmark_requested_from(std::env::args().skip(1))
}

#[cfg(all(
    feature = "hardware-raspberry-pi-zero-2w",
    feature = "routing-tree-benchmark",
    feature = "benchmark-voice-pools-128",
))]
fn orange_benchmark_requested_from(mut args: impl Iterator<Item = String>) -> bool {
    args.any(|arg| arg == "--benchmark-orange-audio")
}

#[cfg(any(
    test,
    not(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    ))
))]
fn raspberry_benchmark_requested_from(mut args: impl Iterator<Item = String>) -> bool {
    args.any(|arg| arg == "--benchmark-raspberry-audio")
}

#[cfg(feature = "native-audio")]
fn pin_normal_startup_thread() {
    if let Err(error) = audio_priority::pin_main_thread_to_cpu0() {
        eprintln!("Normal startup CPU affinity failed: {error}");
        std::process::exit(2);
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
fn main() {
    if raspberry_benchmark_requested() {
        eprintln!("--benchmark-raspberry-audio requires the Raspberry diagnostic benchmark build");
        std::process::exit(2);
    }
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
    if dsp_profile::profile_requested() && live_audio_benchmark::requested() {
        eprintln!("--profile-dsp and live audio benchmark cannot be combined");
        std::process::exit(2);
    }
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
    if live_audio_benchmark::requested() {
        let result = live_audio_benchmark::run();
        if let Err(error) = &result {
            eprintln!("Live audio benchmark failed: {error}");
        }
        std::process::exit(exit_code(result.is_ok()));
    }
    #[cfg(feature = "native-audio")]
    pin_normal_startup_thread();
    if let Err(error) = runtime_thread::run_on_runtime_thread(orange_candidate::run) {
        eprintln!("Orange foreground candidate failed: {error}");
        std::process::exit(error.exit_code());
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn main() {
    #[cfg(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    ))]
    if orange_benchmark_requested_from(std::env::args().skip(1)) {
        eprintln!("--benchmark-orange-audio requires the Orange diagnostic benchmark build");
        std::process::exit(2);
    }
    #[cfg(not(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    )))]
    if raspberry_benchmark_requested()
        || std::env::args()
            .skip(1)
            .any(|arg| arg == "--benchmark-orange-audio")
    {
        eprintln!("live audio benchmark arguments require their matching diagnostic build");
        std::process::exit(2);
    }
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
        utility_mode::UtilityMode::FatDiagnostic => std::process::exit(fat_diagnostic_exit_code()),
        utility_mode::UtilityMode::InteractiveHardware => {
            std::process::exit(exit_code(hardware_test::run_interactive()))
        }
        utility_mode::UtilityMode::InteractiveNoise => {
            std::process::exit(exit_code(hardware_test::run_noise_only()))
        }
        utility_mode::UtilityMode::Normal => {}
    }

    #[cfg(all(
        feature = "hardware-raspberry-pi-zero-2w",
        feature = "routing-tree-benchmark",
        feature = "benchmark-voice-pools-128",
    ))]
    {
        if dsp_profile::profile_requested() && live_audio_benchmark::requested() {
            eprintln!("--profile-dsp and live audio benchmark cannot be combined");
            std::process::exit(2);
        }
        if dsp_profile::profile_requested() {
            std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
        }
        if live_audio_benchmark::requested() {
            #[cfg(feature = "native-audio")]
            pin_normal_startup_thread();
            let result = live_audio_benchmark::run();
            if let Err(error) = &result {
                eprintln!("Live audio benchmark failed: {error}");
            }
            std::process::exit(exit_code(result.is_ok()));
        }
    }

    run_requested_utility();

    #[cfg(feature = "native-audio")]
    pin_normal_startup_thread();
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
    let (usb_config, audio_optimization) = match usb_config::read_boot_runtime_config(&store_dir) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Pi System/Patch boot configuration is unavailable: {error}");
            std::process::exit(2);
        }
    };
    let hardware = match init_hardware(handoff_mode == boot_oled_handoff::HandoffMode::Direct) {
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
    let audio = match init_audio(audio_optimization, usb_config.audio_outputs) {
        Ok(audio) => audio,
        Err(error) => {
            eprintln!("Audio init failed: {error}");
            std::process::exit(2);
        }
    };
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    let mut audio = audio;
    #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
    let audio_load_rx = audio
        .as_mut()
        .and_then(AudioManager::take_load_status_receiver);

    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });

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
        #[cfg(feature = "hardware-raspberry-pi-zero-2w")]
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
        #[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
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

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn run_requested_utility() {
    if dsp_profile::profile_requested() {
        std::process::exit(exit_code(dsp_profile::run_dsp_profile().is_ok()));
    }
    if timing_probe::requested() {
        std::process::exit(exit_code(timing_probe::run()));
    }
    if oled_utility::requested() {
        std::process::exit(exit_code(oled_utility::run()));
    }
}

fn fat_diagnostic_exit_code() -> i32 {
    diagnostic_result_exit_code(fat_diagnostic::run())
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
#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
