import type { RuntimeSnapshot } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";

export type InputAction =
  | { type: "device_input"; input: import("@cellsymphony/device-contracts").DeviceInput }
  | { type: "emergency_brake" }
  | { type: "shift"; active: boolean }
  | { type: "fn"; active: boolean };

export type NeoKeyLeds = {
  back: "off" | "solid_red";
  space: "off" | "beat" | "measure";
  shift: "off" | "solid_yellow" | "solid_blue";
  fn: "off" | "solid_yellow" | "solid_blue";
};

export type SimulatorSnapshot = {
  frame: RuntimeSnapshot;
  neoKeyLeds: NeoKeyLeds;
  masterVolume: number;
  voiceStealingMode: "off" | "lenient" | "balanced" | "aggressive";
  audioLoad: { ratio: number; voiceSteal: boolean };
  audioError: string | null;
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  autoSaveFlash: "none" | "flash";
  autoSaveFlashSerial?: number;
  displayBrightness: number;
  buttonBrightness: number;
};

export type RuntimeListener = (snapshot: SimulatorSnapshot) => void;
export type EventsListener = (events: MusicalEvent[]) => void;
