import { createGridDomain } from "./gridDomain";
import { GRID_HEIGHT, GRID_WIDTH, OLED_HEIGHT, OLED_WIDTH } from "./platformCapabilities.generated";
import type { RuntimeErrorMetadata } from "./runtimeErrors";

export type MusicalEvent =
  | { type: "note_on"; channel: number; note: number; velocity: number; durationMs?: number }
  | { type: "note_off"; channel: number; note: number }
  | { type: "cc"; channel: number; controller: number; value: number };

export type DeviceInput =
  | { type: "encoder_turn"; delta: number; id?: "main" | `aux${number}` }
  | { type: "encoder_press"; id?: "main" | `aux${number}` }
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
  off?: boolean;
  splash?: "startup" | "sleep" | "wakeup" | "shutdown" | string;
  toast?: string;
  colors?: number[];
  barValues?: Array<{ frac: number; numChars: number; style?: "marker" | string } | null>;
  scrollOffset?: number | null;
  totalRows?: number | null;
  visibleRows?: number | null;
};

export type OledFrame = {
  width: typeof OLED_WIDTH;
  height: typeof OLED_HEIGHT;
  format: "rgb565be";
  pixels: Uint8Array;
};

export type LedCell = { r: number; g: number; b: number };
export const GRID_DOMAIN = createGridDomain(GRID_WIDTH, GRID_HEIGHT);

export type LedMatrixFrame = {
  width: number;
  height: number;
  rgb: number[];
  active: boolean[];
};

export type HdmiMode = "none" | "live-grid" | "plain-grid" | "active-behavior" | "cycle-behaviors";

export type HdmiSnapshot = {
  mode: HdmiMode;
  showGridlines: boolean;
  cycleMeasures: number;
  sourceLayerIndex: number;
  sourceBehaviorId: string;
  grid: LedMatrixFrame;
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
  gridBrightness?: number;
  buttonBrightness: number;
  masterVolume: number;
  voiceStealingMode: "fixed12" | "fixed16" | "auto-soft" | "auto-balanced" | "auto-hard" | "none";
  instruments?: unknown[];
  mixer?: unknown;
  panPositions?: number;
  audioConfigRevision?: number;
  autoSaveFlash: "none" | "flash";
  autoSaveFlashSerial?: number;
  dimTimerSeconds?: number;
  screenSleepSeconds?: number;
  ledsDimmed?: boolean;
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
  hdmi?: HdmiSnapshot;
  transport: TransportFrame;
  activeBehavior: string;
  gridInteraction: GridInteraction;
  selectedRow?: number | null;
  eventDotOn?: boolean;
  voiceSteal?: boolean;
  transportIcon?: "play" | "pause" | "stop";
  transportFlash?: "none" | "beat" | "measure";
  cpuLoadRatio?: number;
  settings?: RuntimeSnapshotSettings;
  runtimeError?: RuntimeErrorMetadata;
};
