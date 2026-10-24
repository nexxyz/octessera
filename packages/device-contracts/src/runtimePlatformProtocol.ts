import type {
  RuntimeErrorCode,
  RuntimeErrorFacts,
  RuntimeOperation,
} from "./runtimeErrors";
import type { RuntimeSetupPortalStatus } from "./runtimeSetupPortal";
import type { RuntimeUserDataTransferStatus } from "./runtimeUserDataTransfer";
import type { RuntimeAudioCommand } from "./runtimeAudioProtocol";

export type RuntimePlatformEffect =
  | { type: "store_list_presets" }
  | { type: "store_load_preset"; name: string }
  | {
      type: "store_save_preset";
      name: string;
      payload: Record<string, unknown>;
      mode?: "immediate" | "deferred";
    }
  | { type: "store_delete_preset"; name: string }
  | { type: "store_load_default" }
  | { type: "store_load_system" }
  | { type: "store_save_system"; payload: Record<string, unknown> }
  | {
      type: "store_save_default";
      payload: Record<string, unknown>;
      mode?: "immediate" | "deferred";
    }
  | { type: "store_save_backup"; payload: Record<string, unknown> }
  | { type: "store_save_recovery"; payload: Record<string, unknown> }
  | { type: "midi_list_outputs_request" }
  | { type: "midi_list_inputs_request" }
  | { type: "midi_select_output"; id: string | null }
  | { type: "midi_select_input"; id: string | null }
  | { type: "midi_panic" }
  | { type: "recording_start_audio"; maxMinutes: number }
  | { type: "recording_start_audio_oled"; maxMinutes: number }
  | { type: "recording_stop" }
  | { type: "reboot" }
  | { type: "shutdown" }
  | { type: "update_check" }
  | { type: "update_apply" }
  | { type: "rollback" }
  | { type: "system_info_request" }
  | { type: "setup_portal_open" }
  | { type: "user_data_transfer_open" }
  | { type: "user_data_transfer_close" }
  | {
      type: "sample_list_request";
      instrumentSlot: number;
      sampleSlot: number;
      dir: string;
    }
  | { type: "audio_command"; command: RuntimeAudioCommand };

export type RuntimeStoreResult =
  | { type: "list_presets_result"; names: string[] }
  | {
      type: "load_preset_result";
      name: string;
      payload: Record<string, unknown> | null;
    }
  | {
      type: "save_preset_result";
      name: string;
      outcome: "created" | "overwritten";
    }
  | { type: "delete_preset_result"; name: string; ok: boolean }
  | { type: "load_default_result"; payload: Record<string, unknown> | null }
  | { type: "load_system_result"; payload: Record<string, unknown> | null }
  | { type: "save_system_result"; ok: boolean }
  | { type: "save_default_result"; ok: boolean; isAuto?: boolean }
  | { type: "save_backup_result"; ok: boolean }
  | { type: "save_recovery_result"; ok: boolean }
  | { type: "store_error"; message: string }
  | { type: "runtime_failure"; error: RuntimeErrorFacts }
  | {
      type: "identified";
      result: RuntimeStoreResult;
      requestId: string;
      revision?: number;
    }
  | {
      type: "operation_succeeded";
      operation: RuntimeOperation;
      requestId?: string;
      revision?: number;
    }
  | {
      type: "midi_list_outputs_result";
      outputs: Array<{ id: string; name: string }>;
    }
  | {
      type: "midi_list_inputs_result";
      inputs: Array<{ id: string; name: string }>;
    }
  | {
      type: "midi_status";
      ok: boolean;
      message?: string;
      selectedOutId?: string | null;
      selectedInId?: string | null;
    }
  | {
      type: "sample_list_result";
      instrumentSlot: number;
      sampleSlot: number;
      dir: string;
      entries: Array<{ name: string; path: string; isDir: boolean }>;
    }
  | {
      type: "sample_list_error";
      instrumentSlot: number;
      sampleSlot: number;
      dir: string;
      message: string;
    }
  | { type: "sample_preview_error"; message: string }
  | { type: "device_update_status"; ok: boolean; message: string }
  | {
      type: "recording_status";
      ok: boolean;
      message: string;
    }
  | { type: "system_info_result"; info: RuntimeSystemInfo }
  | { type: "system_info_error"; error: RuntimeSystemInfoError }
  | RuntimeSetupPortalStatus
  | RuntimeUserDataTransferStatus;

export type RuntimeSystemInfo = {
  os: string;
  osVersion: string;
  octesseraVersion: string;
  primaryIp: string | null;
  primaryMac: string | null;
  hostname: string;
  boardProfile: string;
};

export type RuntimeSystemInfoError = {
  code: RuntimeErrorCode;
  message: string;
};
