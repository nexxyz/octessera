import type {
  DeviceInput,
  MusicalEvent,
  NativeRuntimeSnapshot,
  OledFrameRevision,
} from "./coreTypes";
import type {
  RuntimeErrorCode,
  RuntimeErrorFacts,
  RuntimeErrorMetadata,
  RuntimeOperation,
} from "./runtimeErrors";
import type { RuntimeSetupPortalStatus } from "./runtimeSetupPortal";
import type { RuntimeUserDataTransferStatus } from "./runtimeUserDataTransfer";

export {
  RUNTIME_SETUP_PORTAL_DISPOSITIONS,
  RUNTIME_SETUP_PORTAL_ERROR_CODES,
  RUNTIME_SETUP_PORTAL_PHASES,
  SETUP_PORTAL_SUFFIX_MAX_CHARS,
  isRuntimeSetupPortalStatus,
  isRuntimeSetupPortalSuffix,
} from "./runtimeSetupPortal";
export type {
  RuntimeSetupPortalDisposition,
  RuntimeSetupPortalErrorCode,
  RuntimeSetupPortalFailureErrorCode,
  RuntimeSetupPortalPhase,
  RuntimeSetupPortalStatus,
  RuntimeSetupPortalSuffix,
} from "./runtimeSetupPortal";
export {
  RUNTIME_USER_DATA_TRANSFER_PHASES,
  USER_DATA_TRANSFER_CODE_LENGTH,
  USER_DATA_TRANSFER_CODE_PATTERN,
  isRuntimeUserDataTransferStatus,
} from "./runtimeUserDataTransfer";
export type {
  RuntimeUserDataTransferPhase,
  RuntimeUserDataTransferStatus,
} from "./runtimeUserDataTransfer";

export const RUNTIME_STATUS_STATES = [
  "idle",
  "running",
  "paused",
  "error",
] as const;
export type RuntimeStatusState = (typeof RUNTIME_STATUS_STATES)[number];

export const RUNTIME_TRANSPORT_STATES = [
  "stopped",
  "playing",
  "paused",
] as const;
export type RuntimeTransportState = (typeof RUNTIME_TRANSPORT_STATES)[number];

export const MIDI_REALTIME_MESSAGE_TYPES = [
  "clock",
  "start",
  "continue",
  "stop",
] as const;
export type MidiRealtimeMessageType =
  (typeof MIDI_REALTIME_MESSAGE_TYPES)[number];

export const RUNTIME_MOMENTARY_FX_TYPES = [
  "none",
  "stutter",
  "freeze",
  "filter_sweep",
  "pitch_shift",
] as const;
export type RuntimeMomentaryFxType =
  (typeof RUNTIME_MOMENTARY_FX_TYPES)[number];

export type RuntimeMomentaryFxTarget =
  | { type: "global" }
  | { type: "fx_bus"; index: number }
  | { type: "instrument"; index: number };

export type RuntimeAudioCommand =
  | {
      type: "set_audio_config";
      revision: number;
      requestId?: string;
      config: Record<string, unknown>;
    }
  | { type: "set_master_volume"; volumePct: number }
  | {
      type: "set_instrument_mixer";
      instrumentSlot: number;
      volumePct?: number;
      panPos?: number;
    }
  | {
      type: "set_fx_bus_mixer";
      busIndex: number;
      panPos?: number;
      volumePct?: number;
    }
  | {
      type: "set_synth_param";
      instrumentSlot: number;
      path: string;
      value: number;
    }
  | {
      type: "set_sample_bank_param";
      instrumentSlot: number;
      path: string;
      value: number;
    }
  | {
      type: "set_fx_bus_slot";
      busIndex: number;
      slotIndex: number;
      fxType: string;
      params: Record<string, unknown>;
    }
  | {
      type: "set_global_fx_slot";
      slotIndex: number;
      fxType: string;
      params: Record<string, unknown>;
    }
  | {
      type: "momentary_fx_start";
      id: string;
      fxType: RuntimeMomentaryFxType;
      params: Record<string, unknown>;
      target: RuntimeMomentaryFxTarget;
    }
  | { type: "momentary_fx_update"; id: string; params: Record<string, unknown> }
  | { type: "momentary_fx_stop"; id: string }
  | {
      type: "sample_preview";
      instrumentSlot: number;
      sampleSlot: number;
      path: string;
      velocity: number;
    };

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
  | { type: "reboot" }
  | { type: "shutdown" }
  | { type: "hardware_test" }
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

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

export type RuntimeStatus = {
  state: RuntimeStatusState;
  transport: RuntimeTransportState;
  currentPpqnPulse: number;
  pendingResync: boolean;
  syncSource: "internal" | "external";
  message?: string;
  error?: RuntimeErrorMetadata;
};

export type RuntimeDeviceInputMessage = {
  type: "device_input";
  input: DeviceInput;
  requestSnapshot?: boolean;
};

export type RuntimeTransportPulseStepMessage = {
  type: "transport_pulse_step";
  pulses: number;
  source: "internal" | "external";
  atPpqnPulse?: number;
  requestSnapshot?: boolean;
};

export type RuntimeMidiRealtimeLogicalMessage =
  | { type: "midi_realtime"; message: "clock"; pulses: number }
  | {
      type: "midi_realtime";
      message: Exclude<MidiRealtimeMessageType, "clock">;
    };

export type RuntimeMidiRealtimeWireMessage =
  | { type: "midi_realtime_clock"; pulses: number }
  | { type: "midi_realtime_start" }
  | { type: "midi_realtime_continue" }
  | { type: "midi_realtime_stop" };

export type RuntimeTransportStopMessage = { type: "transport_stop" };

export type RuntimeResultMessage = {
  type: "runtime_result";
  result: RuntimeStoreResult;
};

export type RuntimeHostMessage =
  | RuntimeDeviceInputMessage
  | RuntimeTransportPulseStepMessage
  | RuntimeMidiRealtimeWireMessage
  | RuntimeTransportStopMessage
  | RuntimeResultMessage;

export type RuntimeSnapshotMessage = {
  type: "snapshot";
  snapshot: NativeRuntimeSnapshot;
};
export type RuntimeOledFrameMessage = {
  type: "oled_frame";
  revision: OledFrameRevision;
  width: 128;
  height: 128;
  format: "rgb565be";
  pixelsBase64: string;
};
export type RuntimePlatformEffectsMessage = {
  type: "platform_effects";
  effects: RuntimePlatformEffect[];
};
export type RuntimeMusicalEventsMessage = {
  type: "musical_events";
  events: MusicalEvent[];
};
export type RuntimeMidiEventsMessage = {
  type: "midi_events";
  events: MusicalEvent[];
};
export type RuntimeAudioCommandsMessage = {
  type: "audio_commands";
  commands: RuntimeAudioCommand[];
};
export type RuntimeStatusMessage = {
  type: "runtime_status";
  status: RuntimeStatus;
};

export type RuntimeRunnerMessage =
  | RuntimeSnapshotMessage
  | RuntimeOledFrameMessage
  | RuntimePlatformEffectsMessage
  | RuntimeMusicalEventsMessage
  | RuntimeMidiEventsMessage
  | RuntimeAudioCommandsMessage
  | RuntimeStatusMessage;

export function isPositiveOledFrameRevision(
  value: unknown,
): value is OledFrameRevision {
  return typeof value === "number" && Number.isSafeInteger(value) && value > 0;
}

export function createOledFrameRevision(value: number): OledFrameRevision {
  if (!isPositiveOledFrameRevision(value))
    throw new RangeError("OLED frame revision must be a positive safe integer");
  return value;
}

export function isRuntimeSnapshotMessage(
  value: unknown,
): value is RuntimeSnapshotMessage {
  if (
    !isRecord(value) ||
    value.type !== "snapshot" ||
    !isRecord(value.snapshot)
  )
    return false;
  return isPositiveOledFrameRevision(value.snapshot.oledFrameRevision);
}

export type RuntimeContractFixture = {
  id: string;
  description: string;
  hostMessages: RuntimeHostMessage[];
  runnerMessages: RuntimeRunnerMessage[];
};
