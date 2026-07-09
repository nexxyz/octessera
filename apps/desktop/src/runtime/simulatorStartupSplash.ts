import type { RuntimeHostMessage, RuntimeSnapshot } from "@octessera/device-contracts";

export type StartupSplashTimer = ReturnType<typeof setTimeout> | null;

export function scheduleStartupSplashRefresh(
  snapshot: RuntimeSnapshot,
  timer: StartupSplashTimer,
  mirrorRuntimeMessage: (message: RuntimeHostMessage) => void,
  clearTimer: () => void,
): StartupSplashTimer {
  const splash = String(snapshot.display.splash ?? "");
  if (splash !== "startup") {
    if (timer !== null) clearTimeout(timer);
    return null;
  }
  if (timer !== null) return timer;
  return setTimeout(() => {
    clearTimer();
    mirrorRuntimeMessage({
      type: "transport_pulse_step",
      pulses: 0,
      source: "internal",
      requestSnapshot: true,
    });
  }, 1600);
}
