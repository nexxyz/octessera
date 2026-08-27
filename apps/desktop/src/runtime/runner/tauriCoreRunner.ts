import type {
  RuntimeHostMessage,
  RuntimeRunnerMessage,
} from '@octessera/device-contracts';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

type RuntimeMessagesBatch = {
  seq: number;
  messages: RuntimeRunnerMessage[];
};

type RuntimePayloadRecord = {
  seq?: unknown;
  messages?: unknown;
};

type RuntimePayloadKind = 'messages' | 'batch' | 'batches';

function normalizeRuntimeBatch(payload: unknown): RuntimeMessagesBatch {
  const record = (
    typeof payload === 'object' && payload !== null ? payload : {}
  ) as RuntimePayloadRecord;
  const seq = Number(record.seq ?? 0);
  return {
    seq: Number.isSafeInteger(seq) && seq >= 0 ? seq : 0,
    messages: Array.isArray(record.messages)
      ? (record.messages as RuntimeRunnerMessage[])
      : [],
  };
}

export function normalizeRuntimePayload(
  payload: unknown,
  kind: 'messages',
): RuntimeRunnerMessage[];
export function normalizeRuntimePayload(
  payload: unknown,
  kind: 'batch',
): RuntimeMessagesBatch;
export function normalizeRuntimePayload(
  payload: unknown,
  kind: 'batches',
): RuntimeMessagesBatch[];
export function normalizeRuntimePayload(
  payload: unknown,
  kind: RuntimePayloadKind,
): RuntimeRunnerMessage[] | RuntimeMessagesBatch | RuntimeMessagesBatch[] {
  if (kind === 'messages') {
    return Array.isArray(payload) ? (payload as RuntimeRunnerMessage[]) : [];
  }
  if (kind === 'batches') {
    return Array.isArray(payload)
      ? payload.map((batch) => normalizeRuntimeBatch(batch))
      : [];
  }
  return normalizeRuntimeBatch(payload);
}

const IPC_TIMEOUT = 4_000;

function withTimeout<R>(promise: Promise<R>, ms: number): Promise<R> {
  return Promise.race([
    promise,
    new Promise<R>((_, reject) =>
      setTimeout(
        () => reject(new Error(`Tauri IPC timed out after ${ms}ms`)),
        ms,
      ),
    ),
  ]);
}

class TauriCoreRunnerClient {
  async dispatchRuntime(
    message: RuntimeHostMessage,
  ): Promise<RuntimeRunnerMessage[]> {
    const payload = await withTimeout(
      invoke<unknown>('runtime_dispatch', { message }),
      IPC_TIMEOUT,
    );
    return normalizeRuntimePayload(payload, 'messages');
  }

  async drainRuntimeMessages(): Promise<RuntimeMessagesBatch[]> {
    const payload = await withTimeout(
      invoke<unknown>('runtime_drain_messages'),
      IPC_TIMEOUT,
    );
    return normalizeRuntimePayload(payload, 'batches');
  }

  async listenRuntimeMessages(
    handler: (batch: RuntimeMessagesBatch) => void,
  ): Promise<() => void> {
    if (typeof window === 'undefined' || !('__TAURI_INTERNALS__' in window))
      return () => {};
    const unlisten = await listen<unknown>('runtime_messages', (evt) => {
      handler(normalizeRuntimePayload(evt.payload, 'batch'));
    });
    return unlisten;
  }
}

export const tauriCoreRunner = new TauriCoreRunnerClient();
