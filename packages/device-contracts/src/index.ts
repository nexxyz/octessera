export { GRID_DOMAIN } from "./coreTypes";
export type {
  DeviceInput,
  DisplayFrame,
  GridInteraction,
  LedCell,
  LedMatrixFrame,
  MusicalEvent,
  OledFrame,
  PageId,
  RuntimeSnapshot,
  RuntimeSnapshotSettings,
  TransportFrame
} from "./coreTypes";
export { createGridDomain } from "./gridDomain";
export type { GridCell, GridDomain } from "./gridDomain";
export { AUX_ENCODER_COUNT, GRID_HEIGHT, GRID_WIDTH, OLED_HEIGHT, OLED_WIDTH, PAN_POSITION_COUNT, PLATFORM_CAPS } from "./platformCapabilities.generated";
export {
  MIDI_REALTIME_MESSAGE_TYPES,
  RUNTIME_MOMENTARY_FX_TYPES,
  RUNTIME_STATUS_STATES,
  RUNTIME_TRANSPORT_STATES,
  SHARED_RUNTIME_CONTRACT_FIXTURES
} from "./runtimeProtocol";
export type {
  MidiRealtimeMessageType,
  RuntimeAudioCommand,
  RuntimeAudioCommandsMessage,
  RuntimeContractFixture,
  RuntimeDeviceInputMessage,
  RuntimeHostMessage,
  RuntimeMidiRealtimeLogicalMessage,
  RuntimeMidiRealtimeWireMessage,
  RuntimeMomentaryFxTarget,
  RuntimeMomentaryFxType,
  RuntimeMusicalEventsMessage,
  RuntimePlatformEffect,
  RuntimePlatformEffectsMessage,
  RuntimeResultMessage,
  RuntimeRunnerMessage,
  RuntimeSnapshotMessage,
  RuntimeStoreResult,
  RuntimeStatus,
  RuntimeStatusMessage,
  RuntimeStatusState,
  RuntimeTransportPulseStepMessage,
  RuntimeTransportState
} from "./runtimeProtocol";

const CUTOFF_MIN_HZ = 80;
const CUTOFF_MAX_HZ = 16000;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function cutoffDisplayToHz(display: number): number {
  const t = clamp(display, 0, 255) / 255;
  return Math.round(CUTOFF_MIN_HZ * Math.exp(t * Math.log(CUTOFF_MAX_HZ / CUTOFF_MIN_HZ)));
}
