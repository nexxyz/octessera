import assert from 'node:assert/strict';
import test from 'node:test';
import {
  SHARED_RUNTIME_CONTRACT_FIXTURES,
  type RuntimeRunnerMessage,
} from '@octessera/device-contracts';
import { createSimulatorRuntime } from '../src/runtime/simulatorRuntime';
import type { AudioLoadStatus } from '../src/audio/audioLoadEvents';
import type { RuntimeScheduler } from '../src/runtime/runtimeScheduler';

const CONTRACT_FIXTURE = SHARED_RUNTIME_CONTRACT_FIXTURES[0]!;

class RecordingScheduler implements RuntimeScheduler {
  starts = 0;
  stops = 0;

  start(): void {
    this.starts += 1;
  }

  stop(): void {
    this.stops += 1;
  }
}

function waitTurn(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

test('simulator preserves native message ordering in one published response', async () => {
  const seen: Array<{ type: string }> = [];
  const published: Array<{
    title: string;
    oledAvailable: boolean;
    runtimeStatus: string | null;
  }> = [];
  const runtime = createSimulatorRuntime(new RecordingScheduler(), {
    runtimeDispatch: async (message) => {
      seen.push({ type: message.type });
      return CONTRACT_FIXTURE.runnerMessages;
    },
  });
  runtime.subscribe((snapshot) => {
    published.push({
      title: snapshot.frame.display.title,
      oledAvailable: snapshot.oledFrameAvailable,
      runtimeStatus: snapshot.runtimeStatus?.state ?? null,
    });
  });

  runtime.dispatch({ type: 'grid_press', x: 2, y: 5 });
  await waitTurn();

  assert.deepEqual(seen, [{ type: 'device_input' }]);
  assert.deepEqual(published.at(-1), {
    title: 'Build',
    oledAvailable: true,
    runtimeStatus: 'idle',
  });
});

test('simulator routes native OLED and status updates without desktop interpretation', async () => {
  const runtime = createSimulatorRuntime(new RecordingScheduler(), {
    runtimeDispatch: async () =>
      [
        CONTRACT_FIXTURE.runnerMessages[0]!,
        CONTRACT_FIXTURE.runnerMessages[1]!,
        {
          type: 'runtime_status',
          status: {
            state: 'error',
            transport: 'stopped',
            currentPpqnPulse: 0,
            pendingResync: false,
            syncSource: 'internal',
            error: {
              domain: 'audio',
              code: 'operation_failed',
              operation: 'audio_thread',
              recovery: 'stop_and_silence',
              message: 'audio stopped',
            },
          },
        },
      ] satisfies RuntimeRunnerMessage[],
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 1 });
  await waitTurn();

  const snapshot = runtime.getSnapshot();
  assert.equal(snapshot.oledFrameAvailable, true);
  assert.equal(snapshot.runtimeStatus?.state, 'error');
  assert.equal(snapshot.runtimeStatus?.error?.message, 'audio stopped');
});

test('simulator does not create a desktop fallback for native output messages', async () => {
  const outputMessages =
    SHARED_RUNTIME_CONTRACT_FIXTURES[1]!.runnerMessages.filter(
      (message) => message.type !== 'runtime_status',
    );
  const runtime = createSimulatorRuntime(new RecordingScheduler(), {
    runtimeDispatch: async () => outputMessages,
  });

  runtime.dispatch({ type: 'grid_press', x: 1, y: 1 });
  await waitTurn();

  const snapshot = runtime.getSnapshot();
  assert.equal(snapshot.frame.display.title, 'Boot');
  assert.equal(snapshot.runtimeStatus, null);
  assert.equal(snapshot.oledFrameAvailable, false);
});

test('simulator teardown stops transport and native subscriptions', async () => {
  const scheduler = new RecordingScheduler();
  let emitAsync: (
    seq: number,
    messages: RuntimeRunnerMessage[],
  ) => void = () => {};
  let emitAudioLoad: (status: AudioLoadStatus) => void = () => {};
  let unlistenCount = 0;
  const runtime = createSimulatorRuntime(scheduler, {
    runtimeDispatch: async () => [],
    asyncRuntimeBatchListener: (handler) => {
      emitAsync = handler;
    },
    audioLoadService: {
      listenAudioLoad: async (handler) => {
        emitAudioLoad = handler;
        return () => {
          unlistenCount += 1;
        };
      },
    },
  });
  await waitTurn();

  runtime.start();
  await waitTurn();
  runtime.stop();
  const afterStop = runtime.getSnapshot();
  emitAsync(1, [CONTRACT_FIXTURE.runnerMessages[2]!]);
  emitAudioLoad({
    ratio: 1,
    voiceSteal: true,
    workerUtilization: 1,
    highCpuSteady: true,
    missedQuantumFlash: false,
  });
  await waitTurn();

  assert.equal(scheduler.starts, 1);
  assert.equal(scheduler.stops, 1);
  assert.equal(unlistenCount, 1);
  assert.equal(runtime.getSnapshot().runtimeStatus, afterStop.runtimeStatus);
  assert.deepEqual(runtime.getSnapshot().audioLoad, {
    ratio: 0,
    voiceSteal: false,
    workerUtilization: undefined,
    highCpuSteady: false,
    missedQuantumFlash: false,
  });
});

test('desktop applies an audio load message to aggregate and voice-steal presentation', async () => {
  let emitAudioLoad: (status: AudioLoadStatus) => void = () => {};
  const runtime = createSimulatorRuntime(new RecordingScheduler(), {
    runtimeDispatch: async () => [],
    audioLoadService: {
      listenAudioLoad: async (handler) => {
        emitAudioLoad = handler;
        return () => {};
      },
    },
  });
  const published: boolean[] = [];
  runtime.subscribe((snapshot) => {
    published.push(snapshot.audioLoad.voiceSteal);
  });
  const publishedBeforeLoad = published.length;

  emitAudioLoad({
    ratio: 0.72,
    voiceSteal: true,
    workerUtilization: undefined,
    highCpuSteady: false,
    missedQuantumFlash: false,
  });
  await waitTurn();

  assert.deepEqual(runtime.getSnapshot().audioLoad, {
    ratio: 0.72,
    voiceSteal: true,
    workerUtilization: undefined,
    highCpuSteady: false,
    missedQuantumFlash: false,
  });
  assert.equal(published.length, publishedBeforeLoad + 1);
  assert.equal(published.at(-1), true);
});

test('simulator requires the native dispatch boundary', () => {
  assert.throws(
    () => createSimulatorRuntime(new RecordingScheduler()),
    /requires Tauri native runtime or an injected native dispatch/,
  );
});
