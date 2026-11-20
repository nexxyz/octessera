//! Native Pi app: adapts the shared runtime to the Raspberry Pi and Orange Pi hardware, audio, and OLED.
#[cfg(board_orange)]
mod orange_runtime;
mod render;
mod render_loop;
mod render_loop_queue;
mod seesaw_io;

mod audio;
mod audio_config_parse;
mod audio_engine_owner;
mod audio_event;
mod audio_priority;
mod audio_recording;
mod audio_replay;
mod audio_route;
mod audio_stream_health;
mod autoaux_menu;
mod autoaux_sequence;
mod bluetooth;
mod boot_oled_handoff;
#[cfg(any(board_raspberry, board_host))]
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
#[cfg(any(board_raspberry, board_host))]
mod hardware_init;
mod hardware_runtime_scheduler;
#[cfg(any(board_raspberry, board_host))]
mod hardware_test;
#[cfg(any(board_raspberry, board_host))]
mod hardware_test_noise;
mod hdmi_connector;
mod host_adapter;
mod host_audio_command;
mod host_audio_prep;
mod host_power_lifecycle;
mod initial_audio_prep;
mod input;
mod keyboard_capture;
mod main_paths;
mod midi_host;
mod native_scene_pump;
mod normal_menu;
#[cfg(any(board_raspberry, board_host))]
mod oled_utility;
#[cfg(board_orange)]
mod orange_audio;
#[cfg(board_orange)]
mod orange_oled_suspend;
#[cfg(board_orange)]
mod orange_oled_suspend_policy;
#[cfg(board_orange)]
mod orange_reboot;
mod pi_boot_config;
mod pi_host_core;
#[cfg(test)]
mod pi_store_test_support;
mod platform_service;
mod power_lifecycle;
mod profiling_modes;
#[cfg(any(board_raspberry, board_host))]
mod raspberry_power;
#[cfg(any(board_raspberry, board_host))]
mod raspberry_runtime;
#[cfg(any(board_raspberry, board_host))]
mod raspberry_startup;
#[cfg(any(board_raspberry, board_host))]
mod rpi_oled_handoff_runtime;
mod runtime_dispatch;
mod runtime_init;
mod runtime_loop;
mod runtime_output;
mod runtime_thread;
mod sample_browser;
mod setup_portal;
mod setup_portal_files;
mod setup_portal_paths;
mod setup_portal_worker;
mod show_hold;
mod shutdown_signal;
mod store_fault;
#[cfg(test)]
mod test_temp_dir;
mod timing_input;
#[cfg(feature = "profiling")]
mod timing_load;
#[cfg(all(any(board_raspberry, board_host), feature = "profiling"))]
mod timing_probe;
mod ui_profile;
#[cfg(any(board_raspberry, board_host))]
#[cfg(test)]
mod update_menu_fixture_tests;
mod usb_config;
mod usb_config_validation;
mod user_data_archive;
mod user_data_restore;
mod user_data_transfer;
mod utility_mode;

#[cfg(board_orange)]
use orange_runtime as board_startup;
#[cfg(any(board_raspberry, board_host))]
use raspberry_startup as board_startup;

use octessera_pi::board_profile;

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
    pin_normal_startup_thread();
    #[cfg(all(target_os = "linux", not(board_host)))]
    {
        audio_priority::lock_process_memory();
        audio_priority::tighten_timer_slack();
    }
    board_startup::run_normal_startup();
    power_lifecycle::process_exit_code()
}

#[cfg(any(board_raspberry, board_host, feature = "profiling"))]
fn exit_code(success: bool) -> i32 {
    if success {
        0
    } else {
        1
    }
}
