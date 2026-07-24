export const RUNTIME_ERROR_DOMAINS = ["runtime", "storage", "midi", "sample", "audio", "serialization"] as const;
export type RuntimeErrorDomain = (typeof RUNTIME_ERROR_DOMAINS)[number];

export const RUNTIME_ERROR_CODES = [
  "operation_failed",
  "unavailable",
  "invalid_payload",
  "not_found",
  "unsupported",
  "serialization_failed",
  "audio_thread_failed"
] as const;
export type RuntimeErrorCode = (typeof RUNTIME_ERROR_CODES)[number];

export const RUNTIME_OPERATIONS = [
  "runtime_dispatch",
  "device_input",
  "transport",
  "musical_event",
  "midi_event",
  "midi_message",
  "audio_command",
  "audio_thread",
  "snapshot",
  "transport_stop",
  "store",
  "store_list_presets",
  "store_load_preset",
  "store_save_preset",
  "store_delete_preset",
  "store_load_default",
  "store_save_default",
  "store_save_backup",
  "store_save_recovery",
  "runtime_emission",
  "persistence",
  "midi_list_outputs",
  "midi_list_inputs",
  "midi_status",
  "sample_list",
  "sample_preview",
  "device_update",
  "system_info"
] as const;
export type RuntimeOperation = (typeof RUNTIME_OPERATIONS)[number];

export const RUNTIME_RECOVERIES = ["retry", "retain_last_good", "stop_and_silence"] as const;
export type RuntimeRecovery = (typeof RUNTIME_RECOVERIES)[number];

export type RuntimeErrorMetadata = {
  domain: RuntimeErrorDomain;
  code: RuntimeErrorCode;
  operation: RuntimeOperation;
  recovery: RuntimeRecovery;
  requestId?: string;
  revision?: number;
  message?: string;
};

export type RuntimeErrorFacts = Omit<RuntimeErrorMetadata, "recovery">;
