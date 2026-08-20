import { type DeviceInput } from '@octessera/device-contracts';
import {
  TauriAudioLoadService,
  type AudioLoadService,
  type AudioLoadStatus,
} from '../audio/audioLoadEvents';
import {
  createRuntimeConnection,
  type RuntimeConnectionOptions,
} from './simulatorRuntimeConnection';
import { decodeRuntimeMessages } from './simulatorRuntimeInbound';
import { createRuntimeLifecycle } from './simulatorRuntimeLifecycle';
import { createRuntimeReconciliation } from './simulatorRuntimeReconciliation';
import {
  createIntervalRuntimeScheduler,
  type RuntimeScheduler,
} from './runtimeScheduler';
import type { InputAction, RuntimeListener, SimulatorSnapshot } from './types';

export { shouldApplyRuntimeBatch } from './simulatorRuntimeConnection';

type SimulatorRuntime = {
  dispatch(input: DeviceInput): void;
  dispatchAction(action: InputAction): void;
  start(): void;
  stop(): void;
  subscribe(listener: RuntimeListener): () => void;
  getSnapshot(): SimulatorSnapshot;
};

type RuntimeDeps = {
  audioLoadService?: AudioLoadService;
  runtimeDispatch?: RuntimeConnectionOptions['runtimeDispatch'];
  asyncRuntimeBatchListener?: RuntimeConnectionOptions['asyncRuntimeBatchListener'];
};

export function createSimulatorRuntime(
  scheduler: RuntimeScheduler = createIntervalRuntimeScheduler(8),
  deps: RuntimeDeps = {},
): SimulatorRuntime {
  let audioLoad: AudioLoadStatus = { ratio: 0, voiceSteal: false };

  const lifecycle = createRuntimeLifecycle({
    getSnapshot: () => reconciliation.getSnapshot(audioLoad),
    onAudioLoad: (status) => {
      const previousVisible = audioLoad.ratio >= 0.85 || audioLoad.voiceSteal;
      const previousVoiceSteal = audioLoad.voiceSteal;
      const nextVisible = status.ratio >= 0.85 || status.voiceSteal;
      audioLoad = {
        ratio: Math.max(0, Math.min(2, status.ratio)),
        voiceSteal: status.voiceSteal,
      };
      if (
        previousVisible !== nextVisible ||
        previousVoiceSteal !== status.voiceSteal
      ) {
        lifecycle.publish();
      }
    },
    cleanupPresentation: () => reconciliation.stop(),
  });

  const reconciliation = createRuntimeReconciliation({
    mirrorRuntimeMessage: (message) => connection.enqueue(message),
    publishSnapshot: () => lifecycle.publish(),
  });

  const connectionOptions: RuntimeConnectionOptions = {
    scheduler,
    runtimeDispatch: deps.runtimeDispatch,
    asyncRuntimeBatchListener: deps.asyncRuntimeBatchListener,
    processMessages: (messages, allowAsyncOledSplit) => {
      const hasSnapshot = decodeRuntimeMessages(
        messages,
        {
          onOledFrame: reconciliation.reconcileOledFrame,
          onSnapshot: reconciliation.reconcileSnapshot,
          onRuntimeStatus: reconciliation.reconcileRuntimeStatus,
        },
        allowAsyncOledSplit,
      );
      reconciliation.finishMessageBatch();
      return hasSnapshot;
    },
    publishSnapshot: () => lifecycle.publish(),
  };
  const connection = createRuntimeConnection(connectionOptions);
  lifecycle.listenAudioLoad(
    deps.audioLoadService ?? new TauriAudioLoadService(),
  );

  function dispatchInputAction(action: InputAction): void {
    if (action.type === 'emergency_brake') {
      connection.dispatchInput({ type: 'button_s', pressed: true });
      return;
    }
    if (action.type === 'shift') {
      connection.dispatchInput({
        type: 'button_shift',
        pressed: action.active,
      });
      return;
    }
    if (action.type === 'fn') {
      connection.dispatchInput({ type: 'button_fn', pressed: action.active });
      return;
    }
    if (action.input.type === 'encoder_turn') {
      connection.dispatchEncoderTurn(action.input);
    } else {
      connection.dispatchInput(action.input);
    }
  }

  return {
    dispatch(input) {
      if (input.type === 'encoder_turn') {
        connection.dispatchEncoderTurn(input);
      } else {
        connection.dispatchInput(input);
      }
    },
    dispatchAction: dispatchInputAction,
    start() {
      lifecycle.start();
      connection.start();
    },
    stop() {
      lifecycle.stop((beforeSchedulerStop) =>
        connection.stop(beforeSchedulerStop),
      );
    },
    subscribe: lifecycle.subscribe,
    getSnapshot() {
      return reconciliation.getSnapshot(audioLoad);
    },
  };
}
