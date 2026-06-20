import type { RuntimeSnapshot } from "@cellsymphony/device-contracts";

export type InputAction =
  | { type: "device_input"; input: import("@cellsymphony/device-contracts").DeviceInput }
  | { type: "emergency_brake" }
  | { type: "shift"; active: boolean }
  | { type: "fn"; active: boolean };

export type SimulatorSnapshot = {
  frame: RuntimeSnapshot;
  neoKeyLeds: {
    back: "off" | "solid_red";
    space: "off" | "beat" | "measure";
    shift: "off" | "solid_yellow" | "solid_blue";
    fn: "off" | "solid_yellow" | "solid_blue";
  };
  masterVolume: number;
  voiceStealingMode: "off" | "lenient" | "balanced" | "aggressive";
  audioLoad: { ratio: number; voiceSteal: boolean };
  audioError: string | null;
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  audioConfigRevision?: number;
  autoSaveFlash: "none" | "flash";
  autoSaveFlashSerial?: number;
  displayBrightness: number;
  buttonBrightness: number;
};

export type RuntimeListener = (snapshot: SimulatorSnapshot) => void;
