import type { RuntimeRunnerMessage } from '@octessera/device-contracts';

export type AsyncRuntimeBatch = {
  seq: number;
  messages: RuntimeRunnerMessage[];
};

export type AsyncRuntimeBatchSuppression = {
  beginDirectDispatch(nowMs: number): void;
  completeDirectDispatch(hasSnapshot: boolean): void;
  rememberDirectResponse(messages: RuntimeRunnerMessage[]): void;
  handleAsyncBatch(
    batch: AsyncRuntimeBatch,
    nowMs: number,
  ): 'apply' | 'queued' | 'duplicate';
  clear(): void;
};

const ASYNC_RUNTIME_SUPPRESS_MS = 120;
const MAX_SUPPRESSED_ASYNC_BATCHES = 64;

export function batchContainsFault(messages: RuntimeRunnerMessage[]): boolean {
  return messages.some(
    (message) =>
      (message.type === 'runtime_status' &&
        message.status.error !== undefined) ||
      (message.type === 'snapshot' &&
        message.snapshot.runtimeError !== undefined),
  );
}

export function createAsyncRuntimeBatchSuppression(
  replay: (batch: AsyncRuntimeBatch) => void,
  duplicate: (seq: number) => void,
): AsyncRuntimeBatchSuppression {
  let ignoreUntilMs = 0;
  let queued: AsyncRuntimeBatch[] = [];
  let directResponses: RuntimeRunnerMessage[][] = [];
  let replayTimer: ReturnType<typeof setTimeout> | null = null;

  function scheduleReplay() {
    if (replayTimer !== null || queued.length === 0) return;
    const delay = Math.max(0, ignoreUntilMs - performance.now()) + 1;
    replayTimer = setTimeout(() => {
      replayTimer = null;
      if (performance.now() < ignoreUntilMs) {
        scheduleReplay();
        return;
      }
      const batches = queued;
      queued = [];
      for (const batch of batches) {
        if (
          directResponses.some((response) =>
            sameMessages(response, batch.messages),
          )
        ) {
          duplicate(batch.seq);
        } else {
          replay(batch);
        }
      }
      directResponses = [];
    }, delay);
  }

  return {
    beginDirectDispatch(nowMs) {
      if (nowMs >= ignoreUntilMs) directResponses = [];
      ignoreUntilMs = nowMs + ASYNC_RUNTIME_SUPPRESS_MS;
    },
    completeDirectDispatch(hasSnapshot) {
      if (!hasSnapshot) {
        ignoreUntilMs = 0;
        scheduleReplay();
      }
    },
    rememberDirectResponse(messages) {
      if (messages.length === 0) return;
      directResponses.push(messages);
      if (directResponses.length > MAX_SUPPRESSED_ASYNC_BATCHES) {
        directResponses.shift();
      }
    },
    handleAsyncBatch(batch, nowMs) {
      if (nowMs >= ignoreUntilMs || batchContainsFault(batch.messages)) {
        return 'apply';
      }
      if (
        directResponses.some((response) =>
          sameMessages(response, batch.messages),
        )
      ) {
        duplicate(batch.seq);
        return 'duplicate';
      }
      queued.push(batch);
      if (queued.length > MAX_SUPPRESSED_ASYNC_BATCHES) queued.shift();
      scheduleReplay();
      return 'queued';
    },
    clear() {
      queued = [];
      directResponses = [];
      if (replayTimer !== null) clearTimeout(replayTimer);
      replayTimer = null;
    },
  };
}

function sameMessages(
  left: RuntimeRunnerMessage[],
  right: RuntimeRunnerMessage[],
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}
