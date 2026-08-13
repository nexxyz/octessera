import {
  type DeviceInput,
  isPositiveOledFrameRevision,
  type LocalBootstrapSnapshot,
  type NativeRuntimeSnapshot,
  type RuntimeHostMessage,
  type RuntimeRunnerMessage,
  type RuntimeSnapshot,
  type RuntimeStatus,
} from '@octessera/device-contracts';
import {
  TauriAudioLoadService,
  type AudioLoadService,
  type AudioLoadStatus,
} from '../audio/audioLoadEvents';
import {
  createIntervalRuntimeScheduler,
  type RuntimeScheduler,
} from './runtimeScheduler';
import { tauriCoreRunner } from './runner/tauriCoreRunner';
import {
  createInitialRuntimeSnapshot,
  createRuntimeSnapshotCache,
  mergeSnapshotSettings,
  snapshotFromCore,
} from './simulatorSnapshot';
import {
  scheduleStartupSplashRefresh,
  type StartupSplashTimer,
} from './simulatorStartupSplash';
import type { InputAction, RuntimeListener, SimulatorSnapshot } from './types';
import {
  batchContainsFault,
  createAsyncRuntimeBatchSuppression,
} from './asyncRuntimeBatchSuppression';
import {
  acceptOledFrameReference,
  acceptedOledFrame,
  createOledFrameCache,
  ingestOledFrame,
  markOledFrameFault,
} from './oledFrameCache';

type SimulatorRuntime = {
  dispatch(input: DeviceInput): void;
  dispatchAction(action: InputAction): void;
  start(): void;
  stop(): void;
  subscribe(listener: RuntimeListener): () => void;
  getSnapshot(): SimulatorSnapshot;
};

type EncoderTurnInput = Extract<DeviceInput, { type: 'encoder_turn' }>;
type EncoderId = NonNullable<EncoderTurnInput['id']>;
type DesktopRunnerMessage = RuntimeRunnerMessage;

type RuntimeDeps = {
  audioLoadService?: AudioLoadService;
  runtimeDispatch?: (
    message: RuntimeHostMessage,
  ) => Promise<RuntimeRunnerMessage[]>;
  asyncRuntimeBatchListener?: (
    handler: (seq: number, messages: RuntimeRunnerMessage[]) => void,
  ) => void;
};

export function shouldApplyRuntimeBatch(
  lastSeq: number,
  seq: number,
  nowMs: number,
  ignoreUntilMs: number,
  messages: RuntimeRunnerMessage[],
): boolean {
  return (
    seq > lastSeq && (nowMs >= ignoreUntilMs || batchContainsFault(messages))
  );
}

const TAURI_DISPLAY_DRAIN_MS = 66;

export function createSimulatorRuntime(
  scheduler: RuntimeScheduler = createIntervalRuntimeScheduler(8),
  deps: RuntimeDeps = {},
): SimulatorRuntime {
  const isTauri =
    typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  const runtimeDispatch =
    deps.runtimeDispatch ??
    (isTauri
      ? (message: RuntimeHostMessage) =>
          tauriCoreRunner.dispatchRuntime(message)
      : null);
  if (!runtimeDispatch) {
    throw new Error(
      'Desktop runtime requires Tauri native runtime or an injected native dispatch',
    );
  }
  const dispatchRuntime = runtimeDispatch;

  let latestFrame: RuntimeSnapshot | LocalBootstrapSnapshot =
    createInitialRuntimeSnapshot();
  let audioLoad: AudioLoadStatus = { ratio: 0, voiceSteal: false };
  let runtimeStatus: RuntimeStatus | null = null;
  let lastAsyncRuntimeSeq = 0;
  let lastTauriDrainAt = 0;
  let tauriDrainInFlight = false;
  const asyncBatchSuppression = createAsyncRuntimeBatchSuppression(
    (batch) =>
      applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now()),
    (seq) => {
      lastAsyncRuntimeSeq = seq;
    },
  );
  const snapshotCache = createRuntimeSnapshotCache();
  const oledFrameCache = createOledFrameCache();
  const pendingEncoderTurns: Array<{ id: EncoderId; delta: number }> = [];
  let pendingEncoderTimer: ReturnType<typeof setTimeout> | null = null;
  let startupSplashTimer: StartupSplashTimer = null;
  const queuedRuntimeMessages: RuntimeHostMessage[] = [];
  let runtimeDispatchInFlight = false;
  const listeners = new Set<RuntimeListener>();
  const audioLoadService = deps.audioLoadService ?? new TauriAudioLoadService();

  if (deps.asyncRuntimeBatchListener) {
    deps.asyncRuntimeBatchListener((seq, messages) => {
      applyAsyncRuntimeBatch(seq, messages, performance.now());
    });
  } else if (isTauri && !deps.runtimeDispatch) {
    void tauriCoreRunner
      .listenRuntimeMessages((batch) => {
        applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now());
      })
      .catch((err) => {
        console.error('[Runtime] listenRuntimeMessages failed:', err);
      });
  }

  void audioLoadService.listenAudioLoad((status) => {
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
      publishSnapshot();
    }
  });

  function publishSnapshot() {
    const snapshot = snapshotFromCore(
      latestFrame,
      snapshotCache,
      {
        audioLoad,
        runtimeStatus,
        oledFrameFault: oledFrameCache.fault,
        oledFrameAvailable: oledFrameCache.acceptedPixels !== null,
      },
      acceptedOledFrame(oledFrameCache),
    );
    for (const listener of listeners) listener(snapshot);
  }

  function processRunnerMessages(messages: DesktopRunnerMessage[]) {
    let hasSnapshot = false;
    for (const message of messages) {
      hasSnapshot ||= message.type === 'snapshot';
      processRunnerMessage(message);
    }
    return hasSnapshot;
  }

  function processRunnerMessage(message: DesktopRunnerMessage) {
    if (message.type === 'oled_frame') {
      ingestOledFrame(oledFrameCache, message);
      return;
    }
    if (message.type === 'snapshot') {
      const revision = message.snapshot.oledFrameRevision as unknown;
      if (isPositiveOledFrameRevision(revision)) {
        acceptOledFrameReference(oledFrameCache, revision);
      } else {
        markOledFrameFault(
          oledFrameCache,
          revision === undefined ? 'missing' : 'malformed',
        );
      }
      applySnapshotMessage(message.snapshot);
      return;
    }
    if (message.type === 'runtime_status') {
      runtimeStatus = message.status;
      return;
    }
  }

  function applySnapshotMessage(snapshot: NativeRuntimeSnapshot) {
    mergeSnapshotSettings(snapshot, latestFrame);
    latestFrame = {
      ...snapshot,
      oled: acceptedOledFrame(oledFrameCache),
    };
    startupSplashTimer = scheduleStartupSplashRefresh(
      latestFrame,
      startupSplashTimer,
      mirrorRuntimeMessage,
      () => {
        startupSplashTimer = null;
      },
    );
  }

  function drainQueuedRuntimeMessages() {
    if (runtimeDispatchInFlight) return;
    const message = queuedRuntimeMessages.shift();
    if (!message) {
      if (pendingEncoderTurns.length > 0 && pendingEncoderTimer === null) {
        flushPendingEncoderTurns();
      }
      return;
    }
    asyncBatchSuppression.beginDirectDispatch(performance.now());
    runtimeDispatchInFlight = true;
    void dispatchRuntime(message)
      .then((messages) => {
        asyncBatchSuppression.rememberDirectResponse(messages);
        asyncBatchSuppression.completeDirectDispatch(
          processRunnerMessages(messages),
        );
        publishSnapshot();
      })
      .catch((err) => {
        console.error('[Runtime] runtimeDispatch failed:', err);
        publishSnapshot();
      })
      .finally(() => {
        runtimeDispatchInFlight = false;
        drainQueuedRuntimeMessages();
      });
  }

  function mirrorRuntimeMessage(message: RuntimeHostMessage) {
    queuedRuntimeMessages.push(message);
    drainQueuedRuntimeMessages();
  }

  function applyAsyncRuntimeBatch(
    seq: number,
    messages: RuntimeRunnerMessage[],
    nowMs: number,
  ) {
    if (seq <= lastAsyncRuntimeSeq) return;
    if (
      asyncBatchSuppression.handleAsyncBatch({ seq, messages }, nowMs) !==
      'apply'
    ) {
      return;
    }
    if (!shouldApplyRuntimeBatch(lastAsyncRuntimeSeq, seq, nowMs, 0, messages))
      return;
    lastAsyncRuntimeSeq = seq;
    processRunnerMessages(messages);
    publishSnapshot();
  }

  function maybeDrainTauriRuntimeMessages(nowMs: number) {
    if (!isTauri || deps.runtimeDispatch || tauriDrainInFlight) return;
    if (nowMs - lastTauriDrainAt < TAURI_DISPLAY_DRAIN_MS) return;
    tauriDrainInFlight = true;
    lastTauriDrainAt = nowMs;
    void tauriCoreRunner
      .drainRuntimeMessages()
      .then((batches) => {
        for (const batch of batches)
          applyAsyncRuntimeBatch(batch.seq, batch.messages, nowMs);
      })
      .catch((err) =>
        console.error('[Runtime] drainRuntimeMessages failed:', err),
      )
      .finally(() => {
        tauriDrainInFlight = false;
      });
  }

  function dispatchToRunner(input: DeviceInput) {
    flushPendingEncoderTurns(true);
    mirrorRuntimeMessage({ type: 'device_input', input });
  }

  function flushPendingEncoderTurns(forceQueue = false) {
    if (pendingEncoderTimer !== null) {
      clearTimeout(pendingEncoderTimer);
      pendingEncoderTimer = null;
    }
    if (
      !forceQueue &&
      (runtimeDispatchInFlight || queuedRuntimeMessages.length > 0)
    ) {
      pendingEncoderTimer = setTimeout(() => flushPendingEncoderTurns(), 8);
      return;
    }
    const turns = pendingEncoderTurns.splice(0);
    for (const { id, delta } of turns) {
      if (delta === 0) continue;
      mirrorRuntimeMessage({
        type: 'device_input',
        input: { type: 'encoder_turn', id, delta },
      });
    }
  }

  function dispatchEncoderTurn(input: EncoderTurnInput) {
    const id = input.id ?? 'main';
    const last = pendingEncoderTurns.at(-1);
    if (
      last &&
      last.id === id &&
      Math.sign(last.delta) === Math.sign(input.delta)
    ) {
      last.delta = Math.max(-127, Math.min(127, last.delta + input.delta));
    } else {
      pendingEncoderTurns.push({
        id,
        delta: Math.max(-127, Math.min(127, input.delta)),
      });
    }
    if (pendingEncoderTimer !== null) return;
    pendingEncoderTimer = setTimeout(flushPendingEncoderTurns, 8);
  }

  return {
    dispatch(input) {
      if (input.type === 'encoder_turn') {
        dispatchEncoderTurn(input);
      } else {
        dispatchToRunner(input);
      }
    },
    dispatchAction(action) {
      dispatchInputAction(action);
    },
    start() {
      asyncBatchSuppression.beginDirectDispatch(performance.now());
      void dispatchRuntime({
        type: 'transport_pulse_step',
        pulses: 0,
        source: 'internal',
        requestSnapshot: true,
      })
        .then((messages) => {
          asyncBatchSuppression.rememberDirectResponse(messages);
          processRunnerMessages(messages);
          asyncBatchSuppression.completeDirectDispatch(true);
          publishSnapshot();
        })
        .catch((err) => {
          console.error('[Runtime] initial pulse_step failed:', err);
          publishSnapshot();
        });
      scheduler.start((nowMs) => {
        maybeDrainTauriRuntimeMessages(nowMs);
      });
      publishSnapshot();
    },
    stop() {
      flushPendingEncoderTurns(true);
      asyncBatchSuppression.clear();
      if (startupSplashTimer !== null) {
        clearTimeout(startupSplashTimer);
        startupSplashTimer = null;
      }
      publishSnapshot();
      scheduler.stop();
    },
    subscribe(listener) {
      listeners.add(listener);
      listener(
        snapshotFromCore(
          latestFrame,
          snapshotCache,
          {
            audioLoad,
            runtimeStatus,
            oledFrameFault: oledFrameCache.fault,
            oledFrameAvailable: oledFrameCache.acceptedPixels !== null,
          },
          acceptedOledFrame(oledFrameCache),
        ),
      );
      return () => listeners.delete(listener);
    },
    getSnapshot() {
      return snapshotFromCore(
        latestFrame,
        snapshotCache,
        {
          audioLoad,
          runtimeStatus,
          oledFrameFault: oledFrameCache.fault,
          oledFrameAvailable: oledFrameCache.acceptedPixels !== null,
        },
        acceptedOledFrame(oledFrameCache),
      );
    },
  };

  function dispatchInputAction(action: InputAction) {
    if (action.type === 'emergency_brake') {
      dispatchToRunner({ type: 'button_s', pressed: true });
      return;
    }
    if (action.type === 'shift') {
      dispatchToRunner({ type: 'button_shift', pressed: action.active });
      return;
    }
    if (action.type === 'fn') {
      dispatchToRunner({ type: 'button_fn', pressed: action.active });
      return;
    }
    if (action.input.type === 'encoder_turn') {
      dispatchEncoderTurn(action.input);
    } else {
      dispatchToRunner(action.input);
    }
  }
}
