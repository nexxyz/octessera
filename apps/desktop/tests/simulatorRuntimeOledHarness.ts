import {
  createOledFrameRevision,
  GRID_HEIGHT,
  GRID_WIDTH,
  OLED_HEIGHT,
  OLED_WIDTH,
  type NativeRuntimeSnapshot,
  type RuntimeOledFrameMessage,
  type RuntimeRunnerMessage,
} from '@octessera/device-contracts';
import { createSimulatorRuntime } from '../src/runtime/simulatorRuntime';
import type { RuntimeScheduler } from '../src/runtime/runtimeScheduler';

const FRAME_BYTES = OLED_WIDTH * OLED_HEIGHT * 2;

class FakeScheduler implements RuntimeScheduler {
  start(): void {}
  stop(): void {}
}

export type Harness = {
  runtime: ReturnType<typeof createSimulatorRuntime>;
  setResponse: (messages: RuntimeRunnerMessage[]) => void;
  emitAsync: (seq: number, messages: RuntimeRunnerMessage[]) => void;
  emitAudioLoad: (status: { ratio: number; voiceSteal: boolean }) => void;
};

function base64(bytes: Uint8Array): string {
  let binary = '';
  for (let offset = 0; offset < bytes.length; offset += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
  }
  return btoa(binary);
}

export function oledFrame(
  revision: number,
  fill: number,
): RuntimeOledFrameMessage {
  return {
    type: 'oled_frame',
    revision: createOledFrameRevision(revision),
    width: OLED_WIDTH,
    height: OLED_HEIGHT,
    format: 'rgb565be',
    pixelsBase64: base64(new Uint8Array(FRAME_BYTES).fill(fill)),
  };
}

export function snapshot(
  revision: number | null,
  title = 'Boot',
  transportPlaying = false,
  masterVolume = 73,
): RuntimeRunnerMessage {
  const snapshotFields = {
    display: {
      page: 'boot',
      bodyLayout: 'rows',
      title,
      lines: [],
      editing: false,
    },
    leds: {
      width: GRID_WIDTH,
      height: GRID_HEIGHT,
      rgb: new Array<number>(GRID_WIDTH * GRID_HEIGHT * 3).fill(0),
      active: new Array<boolean>(GRID_WIDTH * GRID_HEIGHT).fill(false),
    },
    transport: { playing: transportPlaying, bpm: 120, tick: 0, ppqnPulse: 0 },
    activeBehavior: 'life',
    gridInteraction: 'paint' as const,
    neoKeyLeds: {
      back: [221, 130, 205] as [number, number, number],
      space: [221, 130, 205] as [number, number, number],
      shift: [67, 68, 71] as [number, number, number],
      fn: [67, 68, 71] as [number, number, number],
    },
    eventDotOn: false,
    transportIcon: 'stop' as const,
    transportFlash: 'none' as const,
    settings: {
      displayBrightness: 75,
      buttonBrightness: 75,
      masterVolume,
      voiceStealingMode: 'auto-balanced' as const,
      autoSaveFlash: 'none' as const,
      stopLatched: false,
      shiftHeld: false,
      fnHeld: false,
      combinedModifierHeld: false,
      midi: {
        enabled: false,
        outId: null,
        inId: null,
        syncMode: 'internal' as const,
        clockOutEnabled: false,
        clockInEnabled: false,
      },
    },
  };
  const withRevision =
    revision === null
      ? snapshotFields
      : {
          ...snapshotFields,
          oledFrameRevision:
            revision > 0
              ? createOledFrameRevision(revision)
              : (revision as unknown as NativeRuntimeSnapshot['oledFrameRevision']),
        };
  return {
    type: 'snapshot',
    snapshot: withRevision as NativeRuntimeSnapshot,
  };
}

export function harness(): Harness {
  let response: RuntimeRunnerMessage[] = [];
  let emitAsync: Harness['emitAsync'] = () => {};
  let emitAudioLoad: Harness['emitAudioLoad'] = () => {};
  const runtime = createSimulatorRuntime(new FakeScheduler(), {
    runtimeDispatch: async () => response,
    asyncRuntimeBatchListener: (handler) => {
      emitAsync = handler;
    },
    audioLoadService: {
      listenAudioLoad: async (handler) => {
        emitAudioLoad = handler;
        return () => {};
      },
    },
  });
  return {
    runtime,
    setResponse: (messages) => {
      response = messages;
    },
    emitAsync: (seq, messages) => emitAsync(seq, messages),
    emitAudioLoad: (status) => emitAudioLoad(status),
  };
}

export async function send(
  harness: Harness,
  messages: RuntimeRunnerMessage[],
): Promise<void> {
  harness.setResponse(messages);
  harness.runtime.dispatch({ type: 'grid_press', x: 1, y: 1 });
  await wait();
}

export function pixelBytes(harness: Harness): Uint8Array {
  return harness.runtime.getSnapshot().frame.oled.pixels;
}

export async function acceptedHarness(revision = 1): Promise<Harness> {
  const result = harness();
  await send(result, [oledFrame(revision, 0x11), snapshot(revision)]);
  return result;
}

export async function wait(milliseconds = 0): Promise<void> {
  await new Promise<void>((resolve) => setTimeout(resolve, milliseconds));
}
