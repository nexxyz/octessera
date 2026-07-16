mod audio;
mod audio_config_parse;
mod audio_hotplug;
mod audio_priority;
mod audio_stream_health;
mod diagnostics;
mod dsp_profile;
mod encoder_queue;
mod hardware_fault;
mod hardware_init;
mod hardware_test;
mod hardware_test_noise;
mod host_adapter;
mod host_audio_command;
mod host_audio_prep;
mod input;
mod main_paths;
mod main_runtime_loop;
mod oled_test;
mod persistence;
mod platform_service;
mod recording;
mod render;
mod render_loop;
mod runtime_loop;
mod runtime_thread;
mod sample_browser;
mod seesaw_io;
mod timing_probe;
mod ui_profile;
mod usb_config;
mod wake_trace;

use audio::AudioManager;
use hardware_init::{init_encoders, init_hardware, HardwareDevices};
use input::{midi_realtime_message, MidiMessage};
use render::HardwareRenderTargets;
use render_loop::RenderWorker;
use std::sync::mpsc;
use std::sync::Arc;

use main_paths::{default_samples_dir, default_store_dir, ensure_runtime_dirs};

fn main() {
    let _ = simple_logger::init();

    run_requested_utility();

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
    let seesaw_io = seesaw_io::spawn(trellis, neokey, input_interrupt);
    let store_dir = default_store_dir();
    let usb_config = usb_config::read_usb_runtime_config(&store_dir);
    let audio = init_audio(
        audio_output_buffer_frames_from_default_config(&store_dir),
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

fn exit_code(success: bool) -> i32 {
    if success {
        0
    } else {
        1
    }
}

fn early_boot_splash_enabled() -> bool {
    std::env::var("OCTESSERA_EARLY_BOOT_SPLASH").as_deref() == Ok("1")
}

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

fn audio_output_buffer_frames_from_default_config(store_dir: &std::path::Path) -> Option<u32> {
    let payload = std::fs::read_to_string(store_dir.join("default.json")).ok()?;
    let payload: serde_json::Value = serde_json::from_str(&payload).ok()?;
    payload
        .get("runtimeConfig")
        .unwrap_or(&payload)
        .get("sound")
        .and_then(|sound| sound.get("audioOutputBufferFrames"))
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as u32)
}
