#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod input;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_candidate;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod render;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod render_loop;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod seesaw_io;

#[cfg(feature = "native-audio")]
mod audio;
#[cfg(feature = "native-audio")]
mod audio_config_parse;
#[cfg(feature = "native-audio")]
mod audio_event;
#[cfg(feature = "native-audio")]
mod audio_hotplug;
#[cfg(feature = "native-audio")]
mod audio_priority;
#[cfg(feature = "native-audio")]
mod audio_stream_health;
mod candidate_readiness;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod device_update;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod diagnostics;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod dsp_profile;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod encoder_queue;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_fault;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_init;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_test;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod hardware_test_noise;
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
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod oled_test;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_audio;
#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_host_adapter;
mod persistence;
mod platform_service;
#[cfg(all(feature = "native-audio", not(feature = "hardware-orange-pi-zero-2w")))]
mod recording;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod render;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod render_loop;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod runtime_loop;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod runtime_thread;
mod sample_browser;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod seesaw_io;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod timing_probe;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod ui_profile;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[cfg(test)]
mod update_menu_fixture_tests;
#[cfg(feature = "native-audio")]
mod usb_config;
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
    if octessera_pi::board_profile::metadata_requested() {
        octessera_pi::board_profile::print_build_metadata();
        return;
    }
    if let Err(error) = orange_candidate::run() {
        eprintln!("Orange foreground candidate failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn main() {
    if board_profile::metadata_requested() {
        board_profile::print_build_metadata();
        return;
    }

    let _ = simple_logger::init();

    run_requested_utility();

    if let Err(error) = board_profile::validate_runtime_profile() {
        eprintln!("{error}");
        std::process::exit(2);
    }

    println!("octessera - Pi native runtime");

    let hardware = match init_hardware() {
        Ok(devices) => devices,
        Err(fault) => hardware_fault::run_hardware_fault_mode(fault),
    };
    let (event_rx, _encoders) = match init_encoders() {
        Ok(encoders) => encoders,
        Err(mut fault) => {
            fault.attach_outputs(
                Some(hardware.oled),
                Some(hardware.trellis),
                Some(hardware.neokey),
            );
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
    let audio = init_audio(
        usb_config::audio_output_buffer_frames_from_default_config(&store_dir),
        usb_config.audio_out,
    );

    let (midi_tx, midi_rx) = mpsc::channel::<MidiMessage>();
    let midi_handler = Arc::new(move |bytes: Vec<u8>| {
        if let Some(message) = midi_realtime_message(&bytes) {
            let _ = midi_tx.send(message);
        }
    });

    let samples_dir = default_samples_dir();
    ensure_runtime_dirs(&store_dir, &samples_dir);
    let render_worker = RenderWorker::spawn(HardwareRenderTargets {
        oled,
        seesaw_tx: seesaw_io.command_tx.clone(),
        hdmi: match render::hdmi::HdmiFramebuffer::open_from_env() {
            Ok(hdmi) => hdmi,
            Err(error) => {
                eprintln!("pi HDMI framebuffer disabled: {error}");
                None
            }
        },
    });

    let runtime = runtime_thread::spawn(runtime_thread::RuntimeThreadConfig {
        audio: audio.as_ref().map(AudioManager::service),
        store_dir,
        samples_dir,
        midi_handler,
        usb_midi_out_enabled: usb_config.midi_out_enabled,
        usb_audio_out: usb_config.audio_out,
        midi_rx,
        input_rx: seesaw_io.input_rx,
        encoder_rx: event_rx,
        render_worker,
        early_boot_splash: early_boot_splash_enabled(),
    });
    if runtime.join().is_err() {
        eprintln!("pi runtime thread panicked");
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
    if diagnostics::diagnostic_requested() {
        std::process::exit(exit_code(diagnostics::run_pre_hardware_diagnostics()));
    }
    if oled_test::requested() {
        std::process::exit(exit_code(oled_test::run()));
    }
    if hardware_test::noise_requested() {
        std::process::exit(exit_code(hardware_test::run_noise_only()));
    }
    if hardware_test::requested() {
        std::process::exit(exit_code(hardware_test::run()));
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn exit_code(success: bool) -> i32 {
    if success {
        0
    } else {
        1
    }
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn early_boot_splash_enabled() -> bool {
    std::env::var("OCTESSERA_EARLY_BOOT_SPLASH").as_deref() == Ok("1")
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
fn init_audio(
    output_buffer_frames: Option<u32>,
    audio_out: usb_config::UsbAudioOut,
) -> Option<AudioManager> {
    match AudioManager::new(output_buffer_frames, audio_out) {
        Ok(audio) => {
            println!("Audio ready");
            Some(audio)
        }
        Err(error) => {
            println!("Audio init failed: {error} (continuing without audio)");
            None
        }
    }
}
