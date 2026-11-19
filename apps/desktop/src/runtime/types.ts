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
  audioLoad: import('../audio/audioLoadEvents').AudioLoadStatus;
  displayBrightness: number;
};

export type RuntimeListener = (snapshot: SimulatorSnapshot) => void;
