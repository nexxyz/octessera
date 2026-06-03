import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { type DisplayFrame, type SimulatorFrame } from "@cellsymphony/device-contracts";
import type { AuxTurnBinding, BarValue, PlatformState } from "./platformTypes";
import { OLED_TEXT_LINES } from "./platformTypes";
import { nowMs } from "./timing";
import { cellsToLeds, sampleAssignmentToLeds, touchModeToLeds } from "./runtimeHelpers";
import { paramModOverlayToLeds } from "./paramMod";
import { renderOledFrame } from "./oledRender";
import { logoSepia128Rgb565be } from "./oledAssets/logoSepia128_rgb565be";
import { logo128Rgb565be } from "./oledAssets/logo128_rgb565be";
import { PLATFORM_CAPS } from "./platformCaps";

type OledLines = { lines: string[]; colors: number[] };

type Args<TState> = {
  state: PlatformState<TState>;
  activePart: number;
  engine: BehaviorEngine<any, unknown>;
  model: { name: string; cells: boolean[]; triggerTypes?: import("@cellsymphony/behavior-api").CellTriggerType[] };
  menuView: { path: string; lines: string[]; colors: number[]; barValues: (BarValue | null)[] };
  scanCursor: { axis: "rows" | "columns"; index: number; sections?: unknown } | null;
  toOledLines: (display: DisplayFrame) => OledLines;
  audioLoad?: { ratio: number; voiceSteal: boolean };
  ghostCells?: boolean[];
  paramModBinding?: AuxTurnBinding | null;
};

function audioLoadIndicator(status: { ratio: number; voiceSteal: boolean } | undefined): "yellow" | "red" | undefined {
  if (!status) return undefined;
  if (status.ratio >= 0.85) return "red";
  if (status.ratio >= 0.6 || status.voiceSteal) return "yellow";
  return undefined;
}

export function buildSimulatorFrame<TState>(args: Args<TState>): SimulatorFrame {
  const { state, activePart, model, menuView, scanCursor, toOledLines } = args;
  const baseDisplay: DisplayFrame = {
    page: menuView.path,
    title: menuView.path,
    editing: state.menu.editing,
    lines: menuView.lines,
    colors: menuView.colors
  };
  const oledLines = toOledLines(baseDisplay);
  const maxBodyLines = OLED_TEXT_LINES - 2;
  const alignedBarValues: (BarValue | null)[] = [
    null,
    ...menuView.barValues.slice(0, maxBodyLines)
  ].slice(0, OLED_TEXT_LINES);
  const transportIcon: "play" | "pause" | "stop" = state.transport.playing ? "play" : state.system.stopLatched ? "stop" : "pause";
  const now = nowMs();
  const toast = state.system.toast && state.system.toast.untilMs > now ? state.system.toast.message : null;
  const toastStartedAtMs = state.system.toast && state.system.toast.untilMs > now ? state.system.toast.startedAtMs : undefined;
  const oled = renderOledFrame({
    lines: oledLines.lines,
    barValues: alignedBarValues,
    off: state.system.oledMode === "off",
    splash:
      state.system.oledMode === "splash"
        ? state.system.oledSplashText === "Starting up"
          ? { pixelsRgb565be: logo128Rgb565be, topText: "", bottomText: "Starting up" }
          : { pixelsRgb565be: logoSepia128Rgb565be, topText: state.system.oledSplashText, bottomText: null }
        : undefined,
    transportIcon,
    transportFlash: state.system.transportFlash,
    eventDotOn: state.system.eventBlipUntilMs > now,
    audioLoadIndicator: audioLoadIndicator(args.audioLoad),
    toast,
    toastStartedAtMs,
    renderNowMs: now,
    lineColors: oledLines.colors,
    autoSaveFlash: (state.system as any).autoSaveFlash ?? "none"
  });
  const sampleAssign = state.system.sampleAssign;
  const assignLeds = (() => {
   if (!sampleAssign) return null;
     const inst = (state.runtimeConfig as any).instruments?.[sampleAssign.instrumentSlot];
     if (!inst || inst.type !== "sampler") return null;
    const assignments = Array.isArray(inst.sample?.assignments) ? inst.sample.assignments : [];
    const levels = inst.sample?.velocityLevelsEnabled === true;
    return sampleAssignmentToLeds(assignments, sampleAssign.sampleSlot, levels, state.runtimeConfig.gridBrightness / 100);
  })();
  const touchLeds = touchModeToLeds(state, state.runtimeConfig.gridBrightness / 100, args.ghostCells);
  const paramModLeds = paramModOverlayToLeds(state, args.paramModBinding ?? null, state.runtimeConfig.gridBrightness / 100);
  return {
    display: baseDisplay,
    oled,
    leds: {
      width: PLATFORM_CAPS.gridWidth,
      height: PLATFORM_CAPS.gridHeight,
      cells: assignLeds ?? touchLeds ?? paramModLeds ?? cellsToLeds(model.cells, model.triggerTypes, scanCursor, state.runtimeConfig.gridBrightness / 100, state.system.fnHeld, activePart, args.ghostCells, state.system.touchMode, (state.runtimeConfig as any).parts)
    },
    transport: state.transport,
    activeBehavior: model.name,
    gridInteraction: args.engine.gridInteraction ?? "paint"
  };
}
