import type {
  DeviceInput,
  MusicalEvent,
  NativeRuntimeSnapshot,
  OledFrameRevision,
} from "./coreTypes";
import type { RuntimeErrorMetadata } from "./runtimeErrors";
import type { RuntimeAudioCommand } from "./runtimeAudioProtocol";
import type {
  RuntimePlatformEffect,
  RuntimeStoreResult,
} from "./runtimePlatformProtocol";

export { RUNTIME_FX_PARAM_IDS } from "./runtimeAudioProtocol";
export type {
  RuntimeAudioCommand,
  RuntimeAudioCommandOutbound,
  RuntimeFxParamId,
  RuntimeMomentaryFxTarget,
} from "./runtimeAudioProtocol";

export type {
  RuntimePlatformEffect,
  RuntimeStoreResult,
  RuntimeSystemInfo,
  RuntimeSystemInfoError,
} from "./runtimePlatformProtocol";

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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

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
