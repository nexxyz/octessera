import {
  type DeviceInput,
  type RuntimeHostMessage,
  type RuntimeRunnerMessage,
} from '@octessera/device-contracts';
import {
  batchContainsFault,
  createAsyncRuntimeBatchSuppression,
} from './asyncRuntimeBatchSuppression';
import {
  createIntervalRuntimeScheduler,
  type RuntimeScheduler,
} from './runtimeScheduler';
import { tauriCoreRunner } from './runner/tauriCoreRunner';

export type EncoderTurnInput = Extract<DeviceInput, { type: 'encoder_turn' }>;
type EncoderId = NonNullable<EncoderTurnInput['id']>;

export type RuntimeConnection = {
  dispatchInput(input: DeviceInput): void;
  dispatchEncoderTurn(input: EncoderTurnInput): void;
  enqueue(message: RuntimeHostMessage): void;
  start(): void;
  stop(beforeSchedulerStop: () => void): void;
};

export type RuntimeConnectionOptions = {
  scheduler?: RuntimeScheduler;
  runtimeDispatch?: (
    message: RuntimeHostMessage,
  ) => Promise<RuntimeRunnerMessage[]>;
  asyncRuntimeBatchListener?: (
    handler: (seq: number, messages: RuntimeRunnerMessage[]) => void,
  ) => void;
  processMessages: (
    messages: RuntimeRunnerMessage[],
    allowAsyncOledSplit: boolean,
  ) => boolean;
  publishSnapshot: () => void;
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

export function createRuntimeConnection(
  options: RuntimeConnectionOptions,
): RuntimeConnection {
  const scheduler = options.scheduler ?? createIntervalRuntimeScheduler(8);
  const isTauri =
    typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  const runtimeDispatch =
    options.runtimeDispatch ??
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
  let lastAsyncRuntimeSeq = 0;
  let lastTauriDrainAt = 0;
  let tauriDrainInFlight = false;
  let runtimeListenerActive = true;
  let runtimeListenerPending = false;
  let runtimeUnlisten: (() => void) | null = null;
  const asyncBatchSuppression = createAsyncRuntimeBatchSuppression(
    (batch) =>
      applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now()),
    (seq) => {
      lastAsyncRuntimeSeq = seq;
    },
  );
  const pendingEncoderTurns: Array<{ id: EncoderId; delta: number }> = [];
  let pendingEncoderTimer: ReturnType<typeof setTimeout> | null = null;
  const queuedRuntimeMessages: RuntimeHostMessage[] = [];
  let runtimeDispatchInFlight = false;

  function applyAsyncRuntimeBatch(
    seq: number,
    messages: RuntimeRunnerMessage[],
    nowMs: number,
  ): void {
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
    options.processMessages(messages, true);
    options.publishSnapshot();
  }

  function listenForTauriRuntimeMessages(): void {
    if (
      !isTauri ||
      options.runtimeDispatch ||
      runtimeListenerPending ||
      runtimeUnlisten !== null
    )
      return;
    runtimeListenerPending = true;
    void tauriCoreRunner
      .listenRuntimeMessages((batch) => {
        if (!runtimeListenerActive) return;
        applyAsyncRuntimeBatch(batch.seq, batch.messages, performance.now());
      })
      .then((unlisten) => {
        runtimeListenerPending = false;
        if (!runtimeListenerActive) {
          unlisten();
          return;
        }
        runtimeUnlisten = unlisten;
      })
      .catch((err) => {
        runtimeListenerPending = false;
        console.error('[Runtime] listenRuntimeMessages failed:', err);
      });
  }

  if (options.asyncRuntimeBatchListener) {
    options.asyncRuntimeBatchListener((seq, messages) => {
      if (runtimeListenerActive)
        applyAsyncRuntimeBatch(seq, messages, performance.now());
    });
  } else {
    listenForTauriRuntimeMessages();
  }

  function drainQueuedRuntimeMessages(): void {
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
          options.processMessages(messages, false),
        );
        options.publishSnapshot();
      })
      .catch((err) => {
        console.error('[Runtime] runtimeDispatch failed:', err);
        options.publishSnapshot();
      })
      .finally(() => {
        runtimeDispatchInFlight = false;
        drainQueuedRuntimeMessages();
      });
  }

  function flushPendingEncoderTurns(forceQueue = false): void {
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
      enqueue({
        type: 'device_input',
        input: { type: 'encoder_turn', id, delta },
      });
    }
  }

  function dispatchEncoderTurn(input: EncoderTurnInput): void {
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

  function enqueue(message: RuntimeHostMessage): void {
    queuedRuntimeMessages.push(message);
    drainQueuedRuntimeMessages();
  }

  function maybeDrainTauriRuntimeMessages(nowMs: number): void {
    if (!isTauri || options.runtimeDispatch || tauriDrainInFlight) return;
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

  return {
    dispatchInput(input) {
      flushPendingEncoderTurns(true);
      enqueue({ type: 'device_input', input });
    },
    dispatchEncoderTurn,
    enqueue,
    start() {
      runtimeListenerActive = true;
      listenForTauriRuntimeMessages();
      asyncBatchSuppression.beginDirectDispatch(performance.now());
      void dispatchRuntime({
        type: 'transport_pulse_step',
        pulses: 0,
        source: 'internal',
        requestSnapshot: true,
      })
        .then((messages) => {
          asyncBatchSuppression.rememberDirectResponse(messages);
          options.processMessages(messages, false);
          asyncBatchSuppression.completeDirectDispatch(true);
          options.publishSnapshot();
        })
        .catch((err) => {
          console.error('[Runtime] initial pulse_step failed:', err);
          options.publishSnapshot();
        });
      scheduler.start((nowMs) => {
        maybeDrainTauriRuntimeMessages(nowMs);
      });
      options.publishSnapshot();
    },
    stop(beforeSchedulerStop) {
      flushPendingEncoderTurns(true);
      asyncBatchSuppression.clear();
      runtimeListenerActive = false;
      runtimeUnlisten?.();
      runtimeUnlisten = null;
      beforeSchedulerStop();
      scheduler.stop();
    },
  };
}
