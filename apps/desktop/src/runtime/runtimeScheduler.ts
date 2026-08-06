export type RuntimeScheduler = {
  start(onTick: (nowMs: number, elapsedMs: number) => void): void;
  stop(): void;
};

export function createIntervalRuntimeScheduler(
  intervalMs: number,
): RuntimeScheduler {
  let timer: number | null = null;
  let lastMs = 0;

  return {
    start(onTick) {
      if (timer !== null) return;
      lastMs = performance.now();
      timer = window.setInterval(() => {
        const nowMs = performance.now();
        const elapsedMs = Math.max(0, nowMs - lastMs);
        lastMs = nowMs;
        onTick(nowMs, elapsedMs);
      }, intervalMs);
    },
    stop() {
      if (timer === null) return;
      window.clearInterval(timer);
      timer = null;
    },
  };
}
