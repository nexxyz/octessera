import test from 'node:test';
import assert from 'node:assert/strict';
import type { RuntimeStatus } from '@octessera/device-contracts';
import { toastPaused } from '../src/ui/RuntimeStatusToaster';

const errorStatus = (
  code: 'unavailable' | 'audio_thread_failed',
): RuntimeStatus => ({
  state: 'error',
  transport: 'stopped',
  currentPpqnPulse: 0,
  pendingResync: false,
  syncSource: 'internal',
  error: {
    domain: 'audio',
    code,
    operation: 'audio_thread',
    recovery: 'retry',
  },
});

test('a hovered toast stays paused until it is dismissed or replaced', () => {
  const first = errorStatus('unavailable');
  assert.equal(toastPaused(first, first), true);
  assert.equal(toastPaused(first, null), false);
  assert.equal(toastPaused(first, errorStatus('audio_thread_failed')), false);
  assert.equal(toastPaused(first, { ...first }), false);
  assert.equal(toastPaused(null, first), false);
});
