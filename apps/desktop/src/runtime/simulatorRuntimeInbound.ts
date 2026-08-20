import type {
  NativeRuntimeSnapshot,
  RuntimeOledFrameMessage,
  RuntimeRunnerMessage,
  RuntimeStatus,
} from '@octessera/device-contracts';

export type RuntimeMessageHandlers = {
  onOledFrame: (
    message: RuntimeOledFrameMessage,
    allowAsyncOledSplit: boolean,
  ) => void;
  onSnapshot: (
    snapshot: NativeRuntimeSnapshot,
    allowAsyncOledSplit: boolean,
  ) => void;
  onRuntimeStatus: (status: RuntimeStatus) => void;
};

export function decodeRuntimeMessages(
  messages: RuntimeRunnerMessage[],
  handlers: RuntimeMessageHandlers,
  allowAsyncOledSplit = false,
): boolean {
  let hasSnapshot = false;
  for (const message of messages) {
    if (message.type === 'oled_frame') {
      handlers.onOledFrame(message, allowAsyncOledSplit);
    } else if (message.type === 'snapshot') {
      hasSnapshot = true;
      handlers.onSnapshot(message.snapshot, allowAsyncOledSplit);
    } else if (message.type === 'runtime_status') {
      handlers.onRuntimeStatus(message.status);
    }
  }
  return hasSnapshot;
}
