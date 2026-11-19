import { act, type ReactElement } from 'react';
import { createRoot } from 'react-dom/client';
import { Window } from 'happy-dom';

const DOM_GLOBALS = [
  'document',
  'navigator',
  'Node',
  'Element',
  'HTMLElement',
  'HTMLCanvasElement',
  'ImageData',
  'Event',
  'KeyboardEvent',
  'MouseEvent',
  'WheelEvent',
] as const;

export function installDom(): Window {
  const win = new Window({ url: 'http://localhost/' });
  const define = (name: string, value: unknown) =>
    Object.defineProperty(globalThis, name, {
      value,
      configurable: true,
      writable: true,
    });
  define('window', win);
  for (const name of DOM_GLOBALS) define(name, win[name]);
  define('IS_REACT_ACT_ENVIRONMENT', true);
  return win;
}

export type FakeTimers = {
  advance(ms: number): void;
};

export function installFakeTimers(win: Window): FakeTimers {
  let now = 0;
  let nextId = 1;
  const timers = new Map<number, { at: number; fn: () => void }>();
  const target = win as unknown as Record<string, unknown>;
  target.setTimeout = (fn: () => void, ms = 0) => {
    const id = nextId++;
    timers.set(id, { at: now + ms, fn });
    return id;
  };
  target.clearTimeout = (id: number) => {
    timers.delete(id);
  };
  return {
    advance(ms) {
      const end = now + ms;
      for (;;) {
        const due = [...timers.entries()]
          .filter(([, timer]) => timer.at <= end)
          .sort((a, b) => a[1].at - b[1].at)[0];
        if (!due) break;
        timers.delete(due[0]);
        now = Math.max(now, due[1].at);
        act(() => due[1].fn());
      }
      now = end;
    },
  };
}

export type Mounted = {
  container: HTMLElement;
  render(element: ReactElement): void;
  unmount(): void;
};

export function mount(element: ReactElement): Mounted {
  const container = document.createElement('div');
  document.body.appendChild(container);
  const root = createRoot(container);
  act(() => root.render(element));
  return {
    container,
    render: (next) => act(() => root.render(next)),
    unmount: () => {
      act(() => root.unmount());
      container.remove();
    },
  };
}

export function fire(target: EventTarget, event: Event): void {
  act(() => {
    target.dispatchEvent(event);
  });
}

export async function waitFor(check: () => boolean): Promise<void> {
  for (let i = 0; i < 200; i += 1) {
    if (check()) return;
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 5));
    });
  }
  throw new Error('waitFor timed out');
}
