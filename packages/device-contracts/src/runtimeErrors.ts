export type RuntimeErrorMetadata = {
  domain:
    | "runtime"
    | "storage"
    | "midi"
    | "sample"
    | "audio"
    | "recording"
    | "serialization";
  code:
    | "operation_failed"
    | "unavailable"
    | "invalid_payload"
    | "not_found"
    | "unsupported"
    | "serialization_failed"
    | "audio_thread_failed";
  operation:
    | "runtime_dispatch"
    | "device_input"
    | "transport"
    | "musical_event"
    | "midi_event"
    | "midi_message"
    | "audio_command"
    | "audio_thread"
    | "snapshot"
    | "transport_stop"
    | "store"
    | "store_list_presets"
    | "store_load_preset"
    | "store_save_preset"
    | "store_delete_preset"
    | "store_load_default"
    | "store_load_system"
    | "store_save_system"
    | "store_save_default"
    | "store_save_backup"
    | "store_save_recovery"
    | "runtime_emission"
    | "persistence"
    | "midi_list_outputs"
    | "midi_list_inputs"
    | "midi_status"
    | "sample_list"
    | "sample_preview"
    | "device_update"
    | "recording"
    | "system_info"
    | "setup_portal"
    | "user_data_transfer";
  recovery: "retry" | "retain_last_good" | "stop_and_silence";
  requestId?: string;
  revision?: number;
  message?: string;
};
