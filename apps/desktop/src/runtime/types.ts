import type {
  RuntimeSnapshot,
  RuntimeStatus,
} from '@octessera/device-contracts';
import type { OledFrameCacheFault } from './oledFrameCache';

export type SimulatorSnapshot = {
  frame: RuntimeSnapshot;
  runtimeStatus: RuntimeStatus | null;
  oledFrameFault: OledFrameCacheFault | null;
  oledFrameAvailable: boolean;
  neoKeyLeds: Record<
    'back' | 'space' | 'shift' | 'fn',
    [number, number, number]
  >;
  masterVolume: number;
  audioLoad: import('../audio/audioLoadEvents').AudioLoadStatus;
  instruments: unknown[];
  mixer: unknown;
  panPositions: number;
  autoSaveFlash: 'none' | 'flash';
  autoSaveFlashSerial?: number;
  displayBrightness: number;
  buttonBrightness: number;
};

export type RuntimeListener = (snapshot: SimulatorSnapshot) => void;
