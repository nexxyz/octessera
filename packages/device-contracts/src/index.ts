import { createGridDomain, type GridCell, type GridDomain } from "./gridDomain";
import { GRID_HEIGHT, GRID_WIDTH, OLED_HEIGHT, OLED_WIDTH } from "./platformCapabilities.generated";
export { createGridDomain } from "./gridDomain";
export { GRID_HEIGHT, GRID_WIDTH, OLED_HEIGHT, OLED_WIDTH, PAN_POSITION_COUNT, PLATFORM_CAPS } from "./platformCapabilities.generated";
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
  RuntimeMidiRealtimeMessage,
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

export type MusicalEvent =
  | { type: "note_on"; channel: number; note: number; velocity: number; durationMs?: number }
  | { type: "note_off"; channel: number; note: number }
  | { type: "cc"; channel: number; controller: number; value: number };

export type DeviceInput =
  | { type: "encoder_turn"; delta: -1 | 1; id?: "main" | "aux1" | "aux2" | "aux3" | "aux4" }
  | { type: "encoder_press"; id?: "main" | "aux1" | "aux2" | "aux3" | "aux4" }
  | { type: "button_a"; pressed?: boolean }
  | { type: "button_s"; pressed?: boolean }
  | { type: "button_shift"; pressed?: boolean }
  | { type: "button_fn"; pressed?: boolean }
  | { type: "button_combined_modifier"; pressed?: boolean }
  | { type: "midi_clock"; pulses: number }
  | { type: "midi_start" }
  | { type: "midi_continue" }
  | { type: "midi_stop" }
  | { type: "grid_press"; x: number; y: number }
  | { type: "grid_release"; x: number; y: number }
  | { type: "behavior_action"; actionType: string };

export type PageId = string;

export type DisplayFrame = {
  page: PageId;
  title: string;
  lines: string[];
  editing: boolean;
  colors?: number[]; // RGB565 colors per line (optional, for OLED rendering)
  barValues?: Array<{ frac: number; numChars: number; style?: "marker" | string } | null>;
};

export type OledFrame = {
  width: typeof OLED_WIDTH;
  height: typeof OLED_HEIGHT;
  format: "rgb565be";
  pixels: Uint8Array;
};

export type LedCell = { r: number; g: number; b: number };
export const GRID_DOMAIN = createGridDomain(GRID_WIDTH, GRID_HEIGHT);
export type { GridCell, GridDomain };
export type LedMatrixFrame = {
  width: number;
  height: number;
  cells: LedCell[];
};

export type GridInteraction = "paint" | "momentary";

export type TransportFrame = {
  playing: boolean;
  bpm: number;
  tick: number;
  ppqnPulse: number;
};

export type RuntimeSnapshotSettings = {
  displayBrightness: number;
  buttonBrightness: number;
  masterVolume: number;
  voiceStealingMode: "off" | "lenient" | "balanced" | "aggressive";
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  autoSaveFlash: "none" | "flash";
  autoSaveFlashSerial?: number;
  transportFlash: "none" | "beat" | "measure";
  stopLatched: boolean;
  shiftHeld: boolean;
  fnHeld: boolean;
  combinedModifierHeld: boolean;
  midi: {
    enabled: boolean;
    outId: string | null;
    inId: string | null;
    syncMode: "internal" | "external";
    clockOutEnabled: boolean;
    clockInEnabled: boolean;
  };
};

export type RuntimeSnapshot = {
  display: DisplayFrame;
  oled?: OledFrame;
  leds: LedMatrixFrame;
  transport: TransportFrame;
  activeBehavior: string;
  gridInteraction: GridInteraction;
  settings?: RuntimeSnapshotSettings;
};

const CUTOFF_MIN_HZ = 80;
const CUTOFF_MAX_HZ = 16000;

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function cutoffDisplayToHz(display: number): number {
  const t = clamp(display, 0, 255) / 255;
  return Math.round(CUTOFF_MIN_HZ * Math.exp(t * Math.log(CUTOFF_MAX_HZ / CUTOFF_MIN_HZ)));
}
