import { createGridDomain } from "./gridDomain";
import { GRID_HEIGHT, GRID_WIDTH, OLED_HEIGHT, OLED_WIDTH } from "./platformCapabilities.generated";

export type MusicalEvent =
  | { type: "note_on"; channel: number; note: number; velocity: number; durationMs?: number }
  | { type: "note_off"; channel: number; note: number }
  | { type: "cc"; channel: number; controller: number; value: number };

export type DeviceInput =
  | { type: "encoder_turn"; delta: -1 | 1; id?: "main" | `aux${number}` }
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
  voiceStealingMode: "fixed12" | "fixed16" | "auto-soft" | "auto-balanced" | "auto-hard" | "none";
  instruments?: unknown[];
  mixer?: unknown;
  panPositions?: number;
  audioConfigRevision?: number;
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
