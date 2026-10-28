import type {
  DeviceInput,
  NativeRuntimeSnapshot,
  OledFrameRevision,
} from "./coreTypes";
import type { RuntimeErrorMetadata } from "./runtimeErrors";

export type RuntimeStatus = {
  state: "idle" | "running" | "paused" | "error";
  transport: "stopped" | "playing" | "paused";
  currentPpqnPulse: number;
  pendingResync: boolean;
  syncSource: "internal" | "external";
  message?: string;
  error?: RuntimeErrorMetadata;
};

type RuntimeDeviceInputMessage = {
  type: "device_input";
  input: DeviceInput;
  requestSnapshot?: boolean;
};

type RuntimeTransportPulseStepMessage = {
  type: "transport_pulse_step";
  pulses: number;
  source: "internal" | "external";
  atPpqnPulse?: number;
  requestSnapshot?: boolean;
};

export type RuntimeHostMessage =
  | RuntimeDeviceInputMessage
  | RuntimeTransportPulseStepMessage;

type RuntimeSnapshotMessage = {
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
type RuntimeStatusMessage = {
  type: "runtime_status";
  status: RuntimeStatus;
};

export type RuntimeRunnerMessage =
  | RuntimeSnapshotMessage
  | RuntimeOledFrameMessage
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
