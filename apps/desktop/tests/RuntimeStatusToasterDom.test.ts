import test from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import type { RuntimeStatus } from '@octessera/device-contracts';
import { installDom, installFakeTimers, fire, mount } from './domTestSupport';

const win = installDom();
const timers = installFakeTimers(win);
const { RuntimeStatusToaster } = await import('../src/ui/RuntimeStatusToaster');

const errorStatus = (requestId: string): RuntimeStatus => ({
  state: 'error',
  transport: 'stopped',
  currentPpqnPulse: 0,
  pendingResync: false,
  syncSource: 'internal',
  error: {
    domain: 'audio',
    code: 'unavailable',
    operation: 'audio_thread',
    recovery: 'retry',
    requestId,
  },
});

const view = (status: RuntimeStatus | null) =>
  createElement(RuntimeStatusToaster, {
    status,
    oledFrameFault: null,
    oledFrameAvailable: true,
  });

test('a toast dismissed under the pointer does not freeze the next toast timeout', () => {
  const app = mount(view(errorStatus('first')));
  const toast = () => app.container.querySelector('.runtime-status-toast');
  assert.ok(toast());

  fire(toast()!, new win.MouseEvent('mouseover', { bubbles: true }));
  timers.advance(60_000);
  assert.ok(toast(), 'a hovered toast stays up');

  const dismiss = app.container.querySelector('.runtime-status-dismiss')!;
  fire(dismiss, new win.MouseEvent('click', { bubbles: true }));
  assert.equal(toast(), null);

  app.render(view(errorStatus('second')));
  assert.ok(toast(), 'a new error shows a new toast');
  timers.advance(7000);
  assert.equal(toast(), null, 'the new toast times out on its own');
  app.unmount();
});
