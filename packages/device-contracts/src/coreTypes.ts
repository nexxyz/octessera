import { createGridDomain } from "./gridDomain";
import {
  GRID_HEIGHT,
  GRID_WIDTH,
  OLED_HEIGHT,
  OLED_WIDTH,
} from "./platformCapabilities.generated";
import type { RuntimeErrorMetadata } from "./runtimeErrors";

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

export type OledDisplayLayout = "rows" | "card" | "list";

export type DisplayFrame = {
  page: PageId;
  bodyLayout: OledDisplayLayout;
  title: string;
  titleColor?: number;
  lines: string[];
  editing: boolean;
  off?: boolean;
  splash?: "startup" | "sleep" | "wakeup" | "shutdown" | string;
  toast?: string;
  colors?: number[];
  barValues?: Array<{
    frac: number;
    style?: "marker" | string;
  } | null>;
  rows?: Array<DisplayMenuRow | null>;
  scrollOffset?: number | null;
  totalRows?: number | null;
  visibleRows?: number | null;
};

export type AuxCap = {
  encoder: number;
  shift: boolean;
};

export type AuxHintLabel = {
  text: string;
  color: number;
  source: "auto" | "manual";
};

export type AuxHint = {
  turn: AuxHintLabel | null;
  click: AuxHintLabel | null;
};

export type AuxHints = {
  normal: [AuxHint, AuxHint, AuxHint];
  shift: [AuxHint, AuxHint, AuxHint];
};

export type DisplayMenuRow = {
  kind: "value" | "group" | "action" | "entry" | "picker";
  label: string;
  value: string;
  textCursor: number | null;
  auxCap: AuxCap | null;
  preview?: boolean;
  active?: boolean;
};

export type OledFrame = {
  width: typeof OLED_WIDTH;
  height: typeof OLED_HEIGHT;
  format: "rgb565be";
  pixels: Uint8Array;
};

export type OledFrameRevision = number & {
  readonly __oledFrameRevision: unique symbol;
};

export type RuntimeSnapshotFields = {
  display: DisplayFrame;
  leds: LedMatrixFrame;
  hdmi?: HdmiSnapshot;
  transport: TransportFrame;
  activeBehavior: string;
  gridInteraction: GridInteraction;
  neoKeyLeds: NeoKeyLeds;
  selectedRow?: number | null;
  eventDotLevel: number;
  workerUtilization?: number;
  missedQuantumFlash?: boolean;
  transportIcon: "play" | "pause" | "stop";
  transportFlash: "none" | "beat" | "measure";
  auxHints?: AuxHints;
  auxShiftLayer?: boolean;
  recordingActive?: boolean;
  recordingPulseDim?: boolean;
  cpuLoadRatio?: number;
  settings?: RuntimeSnapshotSettings;
  runtimeError?: RuntimeErrorMetadata;
};

export const GRID_DOMAIN = createGridDomain(GRID_WIDTH, GRID_HEIGHT);

export type LedMatrixFrame = {
  width: number;
  height: number;
  rgb: number[];
  active: boolean[];
};

export type NeoKeyLeds = {
  back: [number, number, number];
  space: [number, number, number];
  shift: [number, number, number];
  fn: [number, number, number];
};

type HdmiMode =
  "terminal" | "live-grid" | "plain-grid" | "active-behavior" | "cycle-behaviors";

type HdmiSnapshot = {
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
  instruments?: unknown[];
  mixer?: unknown;
  panPositions?: number;
  audioConfigRevision?: number;
  autoSaveFlash: "none" | "flash";
  autoSaveFlashSerial?: number;
  dimTimerSeconds?: number;
  screenSleepSeconds?: number;
  ledsDimmed?: boolean;
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

export type NativeRuntimeSnapshot = RuntimeSnapshotFields & {
  oledFrameRevision: OledFrameRevision;
};

export type RuntimeSnapshot = RuntimeSnapshotFields & {
  oled: OledFrame;
};

export type LocalBootstrapSnapshot = RuntimeSnapshotFields & {
  oled: OledFrame;
  oledFrameRevision: 0;
};
