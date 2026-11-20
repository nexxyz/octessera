#[cfg(feature = "hardware-orange-pi-zero-2w")]
mod orange_runtime;
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
mod device_apply;
mod device_update;
#[cfg(feature = "profiling")]
mod dsp_profile;
#[cfg(feature = "profiling")]
mod dsp_scenarios;
mod encoder_queue;
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
mod host_power_lifecycle;
#[cfg(feature = "native-audio")]
mod initial_audio_prep;
#[cfg(feature = "native-audio")]
mod input;
mod keyboard_capture;
mod main_paths;
#[cfg(feature = "external-midi")]
mod midi_host;
mod native_scene_pump;
mod normal_menu;
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
#[cfg(feature = "native-audio")]
mod pi_boot_config;
mod pi_host_core;
#[cfg(test)]
mod pi_store_test_support;
mod platform_service;
mod power_lifecycle;
mod profiling_modes;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod raspberry_power;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod raspberry_runtime;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod raspberry_startup;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
mod rpi_oled_handoff_runtime;
mod runtime_dispatch;
#[cfg(feature = "native-audio")]
mod runtime_init;
#[cfg(feature = "native-audio")]
mod runtime_loop;
#[cfg(feature = "native-audio")]
mod runtime_output;
mod runtime_thread;
mod sample_browser;
mod setup_portal;
mod setup_portal_files;
mod setup_portal_paths;
mod setup_portal_worker;
#[cfg(feature = "native-audio")]
mod show_hold;
#[cfg(feature = "native-audio")]
mod shutdown_signal;
#[cfg(feature = "native-audio")]
mod store_fault;
#[cfg(test)]
mod test_temp_dir;
mod timing_input;
#[cfg(feature = "profiling")]
mod timing_load;
#[cfg(all(not(feature = "hardware-orange-pi-zero-2w"), feature = "profiling"))]
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

#[cfg(feature = "hardware-orange-pi-zero-2w")]
use orange_runtime as board_startup;
#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use raspberry_startup as board_startup;

use octessera_pi::board_profile;

#[cfg(feature = "native-audio")]
fn pin_normal_startup_thread() {
    if let Err(error) = audio_priority::pin_main_thread_to_cpu0() {
        eprintln!("Normal startup CPU affinity failed: {error}");
        std::process::exit(2);
    }
}

fn main() -> std::process::ExitCode {
    let utility_mode = match utility_mode::from_process() {
        Ok(mode) => mode,
        Err(error) => {
            eprintln!("Utility mode error: {error}");
            std::process::exit(2);
        }
    };
    if let Some(exit) = board_startup::run_utility_mode(utility_mode) {
        return exit;
    }
    #[cfg(feature = "native-audio")]
    pin_normal_startup_thread();
    #[cfg(all(
        target_os = "linux",
        any(
            feature = "hardware-raspberry-pi-zero-2w",
            feature = "hardware-orange-pi-zero-2w"
        )
    ))]
    {
        audio_priority::lock_process_memory();
        audio_priority::tighten_timer_slack();
    }
    board_startup::run_normal_startup();
    power_lifecycle::process_exit_code()
}

#[cfg(any(not(feature = "hardware-orange-pi-zero-2w"), feature = "profiling"))]
fn exit_code(success: bool) -> i32 {
    if success {
        0
    } else {
        1
    }
}
