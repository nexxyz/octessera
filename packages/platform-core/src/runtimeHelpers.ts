import { getBehavior } from "@cellsymphony/behavior-api";
import { type LedCell } from "@cellsymphony/device-contracts";
import type { CellTriggerIntent } from "@cellsymphony/interpretation-core";
import type { MusicalEvent } from "@cellsymphony/musical-events";
import { clamp, mod } from "./coreUtils";
import { clampPanPosition, clampPartIndex, PAN_CENTER_POS, PAN_POSITION_MAX, GRID_DOMAIN, PLATFORM_CAPS, scalePanPosition, sectionCount } from "./platformCaps";
import { momentaryFxColor } from "./momentaryFx";
import type { DanceMode, PlatformState } from "./platformTypes";

export const DANCE_PAN_COLORS: Record<string, LedCell> = {
  direct: { r: 255, g: 255, b: 255 },
  fx_bus_1: { r: 190, g: 80, b: 255 },
  fx_bus_2: { r: 0, g: 210, b: 255 },
  fx_bus_3: { r: 0, g: 230, b: 120 },
  fx_bus_4: { r: 255, g: 160, b: 0 }
};

/** Convert a Touch grid X press coordinate to a stored panPos (right cell of the two-cell marker). */
export function touchPanPosFromGridX(x: number, width: number = PLATFORM_CAPS.gridWidth): number {
  const markerCount = Math.max(1, width - 1);
  const cell = clamp(Math.floor(x), 0, width - 1);
  const centerRight = Math.floor(width / 2);
  const marker = cell === centerRight ? centerRight - 1 : cell > centerRight ? cell - 1 : cell;
  return scalePanPosition(marker, markerCount);
}

/** Convert a stored panPos to the left cell X index for the two-cell LED marker. */
export function touchPanMarkerLeftCell(panPos: number, width: number = PLATFORM_CAPS.gridWidth): number {
  const markerCount = Math.max(1, width - 1);
  return clamp(Math.round((clampPanPosition(panPos) / PAN_POSITION_MAX) * (markerCount - 1)), 0, markerCount - 1);
}

export type ResolvedDancePan = {
  route: "direct" | "bus";
  busIndex: number;
  panPos: number;
  color: LedCell;
};

export function resolveDancePanTarget(state: PlatformState<unknown>, row: number): ResolvedDancePan {
  const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? (state.runtimeConfig as any).instruments as any[] : [];
  const inst = instruments[row] ?? {};
  const mixer = inst.mixer ?? {};
  const route: string = mixer.route ?? "direct";
  const busMatch = /^fx_bus_(\d+)$/.exec(route);
  if (busMatch) {
    const busIdx = Number(busMatch[1]) - 1;
    const buses: any[] = Array.isArray((state.runtimeConfig as any).mixer?.buses) ? (state.runtimeConfig as any).mixer.buses as any[] : [];
    const bus = buses[busIdx];
    const panPos = clampPanPosition(bus?.panPos ?? PAN_CENTER_POS);
    const color = DANCE_PAN_COLORS[`fx_bus_${busIdx + 1}`] ?? DANCE_PAN_COLORS.direct;
    return { route: "bus", busIndex: busIdx, panPos, color };
  }
  const panPos = clampPanPosition(mixer.panPos ?? PAN_CENTER_POS);
  return { route: "direct", busIndex: -1, panPos, color: DANCE_PAN_COLORS.direct };
}

export function dedupeSimultaneousNotes(events: MusicalEvent[]): MusicalEvent[] {
  const out: MusicalEvent[] = [];
  const seen = new Map<string, number>();
  for (const event of events) {
    if (event.type !== "note_on") {
      out.push(event);
      continue;
    }
    const key = `${event.channel}:${event.note}`;
    const idx = seen.get(key);
    if (idx === undefined) {
      seen.set(key, out.length);
      out.push(event);
      continue;
    }
    const existing = out[idx];
    if (existing.type === "note_on") {
      out[idx] = { ...existing, velocity: Math.max(existing.velocity, event.velocity), durationMs: Math.max(existing.durationMs ?? 0, event.durationMs ?? 0) };
    }
  }
  return out;
}

export function toGridSnapshot(model: { cells: boolean[] }): { width: number; height: number; cells: boolean[] } {
  return { width: PLATFORM_CAPS.gridWidth, height: PLATFORM_CAPS.gridHeight, cells: model.cells };
}

export function ghostCellsForInactiveParts<TState>(state: PlatformState<TState>, activePart: number, cellCount: number): boolean[] {
  const out = Array.from({ length: cellCount }, () => false);
  const parts: any[] = Array.isArray((state.runtimeConfig as any).parts) ? (state.runtimeConfig as any).parts : [];
  const partStates: any[] = Array.isArray((state as any).partStates) ? ((state as any).partStates as any[]) : [];
  for (let idx = 0; idx < parts.length; idx += 1) {
    if (idx === activePart) continue;
    const behavior = getBehavior(String(parts[idx]?.l1?.behaviorId ?? "none"));
    if (!behavior || partStates[idx] === undefined) continue;
    const cells = behavior.renderModel(partStates[idx]).cells;
    for (let i = 0; i < Math.min(cellCount, cells.length); i += 1) out[i] ||= Boolean(cells[i]);
  }
  return out;
}

export function cellsToLeds(
  cells: boolean[],
  triggerTypes: import("@cellsymphony/behavior-api").CellTriggerType[] | undefined,
  scanCursor: { axis: "rows" | "columns"; index: number; sections?: unknown } | null,
  brightness: number,
  fnHeld: boolean = false,
  activePartIndex: number = 0,
  ghostCells?: boolean[],
  activeDanceMode: DanceMode = "none",
  selectedDanceMode: DanceMode = "none",
  parts: unknown[] = []
): LedCell[] {
  const b = clamp(brightness, 0.1, 1);
  const OFF_BG: LedCell = { r: 15, g: 15, b: 22 };
  const OFF_CURSOR: LedCell = { r: 70, g: 70, b: 76 };
  const out = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => scaleLed(OFF_BG, b));
  for (let yWorld = 0; yWorld < PLATFORM_CAPS.gridHeight; yWorld += 1) {
    for (let x = 0; x < PLATFORM_CAPS.gridWidth; x += 1) {
      const worldIndex = GRID_DOMAIN.indexOf({ x, y: yWorld });
      const screenIndex = GRID_DOMAIN.toDisplayIndex({ x, y: yWorld });
      const alive = !!cells[worldIndex];
      const inCursor = scanCursor !== null && isInScanCursor(x, yWorld, scanCursor);
      if (!alive) {
        if (ghostCells?.[worldIndex]) {
          out[screenIndex] = scaleLed(inCursor ? { r: 45, g: 70, b: 55 } : { r: 0, g: 46, b: 25 }, b);
          continue;
        }
        out[screenIndex] = scaleLed(inCursor ? OFF_CURSOR : OFF_BG, b);
        continue;
      }
      const type = triggerTypes?.[worldIndex] ?? "stable";
      switch (type) {
        case "activate":
          out[screenIndex] = scaleLed({ r: 255, g: 255, b: 255 }, b);
          break;
        case "deactivate":
          out[screenIndex] = scaleLed({ r: 128, g: 128, b: 128 }, b);
          break;
        case "scanned":
          out[screenIndex] = scaleLed({ r: 255, g: 0, b: 0 }, b);
          break;
        default:
          out[screenIndex] = scaleLed({ r: 0, g: 255, b: 120 }, b);
      }
    }
  }
  overlayFnNavigation(out, b, fnHeld, activePartIndex, activeDanceMode, selectedDanceMode, parts);
   return out;
 }

 export function paramModToLeds<TState>(state: PlatformState<TState>): LedCell[] {
   const b = clamp(Number((state.runtimeConfig as any).gridBrightness ?? 75) / 100, 0.1, 1);
   const OFF_BG: LedCell = { r: 15, g: 15, b: 22 };
   const out = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => scaleLed(OFF_BG, b));
   const activePart = clampPartIndex((state.runtimeConfig as any).activePartIndex ?? 0);
   const part = ((state.runtimeConfig as any).parts ?? [])[activePart] ?? {};
   const modSlots = part.modSlots ?? { slots: [] };
   for (let slotIdx = 0; slotIdx < modSlots.slots.length; slotIdx++) {
     const slot = modSlots.slots[slotIdx];
     const mapping = slot.mapping ?? { x: 0, y: 0, mode: "modulation" };
     const isSlot1 = slotIdx === 0;
     const isOrigin = isSlot1 ? (mapping.x === 0 && mapping.y === 0) : (mapping.x === 1 && mapping.y === 1);
     const originCell = isSlot1 ? { x: 0, y: 0 } : { x: 1, y: 1 };
     if (slot.key) {
       for (let y = 0; y < PLATFORM_CAPS.gridHeight; y++) {
         for (let x = 0; x < PLATFORM_CAPS.gridWidth; x++) {
           const screenIndex = GRID_DOMAIN.toDisplayIndex({ x, y });
           const isMapping = isSlot1 ? x === mapping.x && y === mapping.y : x === mapping.x && y === mapping.y;
           const isAxis = isSlot1 ? (x === 0 || y === 0) : (x === 1 || y === 1);
           const isOriginCell = x === originCell.x && y === originCell.y;
           const isSlotArea = isSlot1 ? (x === 0 || y === 0) : (x === 1 || y === 1);
           const isMappingArea = isSlot1 ? (x === mapping.x || y === mapping.y) : (x === mapping.x || y === mapping.y);
           const isCurrentOrigin = isSlot1 ? (x === 0 && y === 0) : (x === 1 && y === 1);
           const isMappingOrigin = isSlot1 ? (x === 0 && y === 0) : (x === 1 && y === 1);
           if (isOriginCell) {
             out[screenIndex] = scaleLed({ r: 255, g: 255, b: 255 }, b);
           } else if (isSlot1 && isMappingArea && !isOriginCell) {
             const invert = slot.invert ? "red" : "green";
             out[screenIndex] = scaleLed({ r: invert === "red" ? 255 : 0, g: invert === "red" ? 0 : 255, b: 120 }, b);
           } else if (isSlot1 && !isSlotArea && !isOriginCell) {
             out[screenIndex] = scaleLed({ r: Math.round(out[screenIndex].r * 0.25), g: Math.round(out[screenIndex].g * 0.25), b: Math.round(out[screenIndex].b * 0.25) }, b);
           } else if (!isSlot1 && isMappingArea && !isOriginCell) {
             const invert = slot.invert ? "red" : "green";
             out[screenIndex] = scaleLed({ r: invert === "red" ? 255 : 0, g: invert === "red" ? 0 : 255, b: 120 }, b);
           } else if (!isSlot1 && !isSlotArea && !isOriginCell) {
             out[screenIndex] = scaleLed({ r: Math.round(out[screenIndex].r * 0.25), g: Math.round(out[screenIndex].g * 0.25), b: Math.round(out[screenIndex].b * 0.25) }, b);
           }
         }
       }
     }
   }
   return out;
 }

 function overlayFnNavigation(out: LedCell[], brightness: number, fnHeld: boolean, activePartIndex: number, activeDanceMode: DanceMode, selectedDanceMode: DanceMode, parts: unknown[] = []): void {
  if (!fnHeld) return;
  // dim non-navigation cells (neither left column nor right column)
  for (let y = 0; y < PLATFORM_CAPS.gridHeight; y += 1) {
    for (let x = 1; x < PLATFORM_CAPS.gridWidth - 1; x += 1) {
      const idx = GRID_DOMAIN.toDisplayIndex({ x, y });
      out[idx] = { r: Math.round(out[idx].r * 0.25), g: Math.round(out[idx].g * 0.25), b: Math.round(out[idx].b * 0.25) };
    }
  }
  const layerCount = Math.min(PLATFORM_CAPS.partCount, PLATFORM_CAPS.gridHeight);
  const inDance = activeDanceMode !== "none";
  for (let layer = 0; layer < layerCount; layer += 1) {
    const screenIndex = GRID_DOMAIN.toDisplayIndex({ x: 0, y: layer });
    const isActive = !inDance && layer === activePartIndex;
    const hasBehavior = String((parts[layer] as any)?.l1?.behaviorId ?? "none") !== "none";
    if (isActive) out[screenIndex] = scaleLed({ r: 0, g: 210, b: 210 }, brightness);
    else if (hasBehavior) out[screenIndex] = scaleLed({ r: 40, g: 180, b: 40 }, brightness);
    else out[screenIndex] = { r: Math.round(out[screenIndex].r * 0.25), g: Math.round(out[screenIndex].g * 0.25), b: Math.round(out[screenIndex].b * 0.25) };
  }
  const pages: DanceMode[] = ["mix", "pan", "fx", "trigger-gate", "xy"];
  for (let row = 0; row < PLATFORM_CAPS.gridHeight; row += 1) {
    const screenIndex = GRID_DOMAIN.toDisplayIndex({ x: PLATFORM_CAPS.gridWidth - 1, y: row });
    const page = pages[row];
    if (page === undefined) {
      out[screenIndex] = { r: Math.round(out[screenIndex].r * 0.25), g: Math.round(out[screenIndex].g * 0.25), b: Math.round(out[screenIndex].b * 0.25) };
      continue;
    }
    const color = page === selectedDanceMode ? { r: 0, g: 210, b: 210 } : { r: 180, g: 180, b: 180 };
    out[screenIndex] = scaleLed(color, brightness);
  }
}

function isInScanCursor(x: number, y: number, cursor: { axis: "rows" | "columns"; index: number; sections?: unknown }): boolean {
  const sections = sectionCount(cursor.sections);
  if (sections <= 1) return (cursor.axis === "columns" && x === cursor.index) || (cursor.axis === "rows" && y === cursor.index);
  if (cursor.axis === "rows") {
    const sectionHeight = Math.max(1, Math.floor(PLATFORM_CAPS.gridHeight / sections));
    const step = mod(cursor.index, PLATFORM_CAPS.gridWidth * sections);
    const section = Math.floor(step / PLATFORM_CAPS.gridWidth);
    const firstY = PLATFORM_CAPS.gridHeight - (section + 1) * sectionHeight;
    return x === step % PLATFORM_CAPS.gridWidth && y >= firstY && y < firstY + sectionHeight;
  }
  const sectionWidth = Math.max(1, Math.floor(PLATFORM_CAPS.gridWidth / sections));
  const step = mod(cursor.index, PLATFORM_CAPS.gridHeight * sections);
  const section = Math.floor(step / PLATFORM_CAPS.gridHeight);
  return y === step % PLATFORM_CAPS.gridHeight && x >= section * sectionWidth && x < (section + 1) * sectionWidth;
}

export function sampleAssignmentToLeds(
  assignments: Array<{ x: number; y: number; sampleSlot: number; level?: "high" | "medium" | "low" }>,
  selectedSampleSlot: number,
  velocityLevelsEnabled: boolean,
  brightness: number
): LedCell[] {
  const b = Math.max(0, Math.min(1, brightness));
  const out: LedCell[] = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => ({ r: 0, g: 0, b: 0 }));
  for (const a of assignments) {
    if (a.x < 0 || a.x >= PLATFORM_CAPS.gridWidth || a.y < 0 || a.y >= PLATFORM_CAPS.gridHeight) continue;
    const screenIndex = GRID_DOMAIN.toDisplayIndex({ x: a.x, y: a.y });
    if (a.sampleSlot !== selectedSampleSlot) {
      out[screenIndex] = scaleLed({ r: 70, g: 70, b: 70 }, b);
      continue;
    }
    if (!velocityLevelsEnabled) {
      out[screenIndex] = scaleLed({ r: 220, g: 220, b: 220 }, b);
      continue;
    }
    if (a.level === "high") out[screenIndex] = scaleLed({ r: 220, g: 0, b: 0 }, b);
    else if (a.level === "medium") out[screenIndex] = scaleLed({ r: 220, g: 180, b: 0 }, b);
    else out[screenIndex] = scaleLed({ r: 0, g: 220, b: 0 }, b);
  }
  return out;
}

export function danceModeToLeds<TState>(state: PlatformState<TState>, brightness: number, ghostCells?: boolean[]): LedCell[] | null {
  const mode = state.system.danceMode;
  if (mode === "none") return null;
  const b = clamp(brightness, 0.1, 1);
  const out: LedCell[] = Array.from({ length: PLATFORM_CAPS.gridWidth * PLATFORM_CAPS.gridHeight }, () => scaleLed({ r: 8, g: 8, b: 14 }, b));
  const instruments = Array.isArray((state.runtimeConfig as any).instruments) ? ((state.runtimeConfig as any).instruments as any[]) : [];

  if (mode === "mix") {
    const count = Math.min(instruments.length, PLATFORM_CAPS.gridWidth);
    for (let x = 0; x < count; x += 1) {
      const inst = instruments[x] ?? {};
      const isNone = (inst as any)?.type === "none";
      const mixer = inst.mixer ?? {};
      const y = clamp(Math.round(clamp(Number(mixer.volume ?? 100), 0, 100) / 100 * (PLATFORM_CAPS.gridHeight - 1)), 0, PLATFORM_CAPS.gridHeight - 1);
      const cell = scaleLed({ r: 0, g: 220, b: 90 }, b);
      out[GRID_DOMAIN.toDisplayIndex({ x, y })] = isNone ? scaleLed(cell, 0.25) : cell;
    }
    // Only show FN navigation overlay when FN is held without Shift for navigation
    // When both FN and Shift are held, suppress the overlay as this is a system navigation command
    const showFnOverlay = state.system.fnHeld && !state.system.shiftHeld;
    const selectedDanceMode = (state.runtimeConfig as any).danceMode && (state.runtimeConfig as any).danceMode !== "none"
      ? (state.runtimeConfig as any).danceMode
      : mode;
    overlayFnNavigation(out, b, showFnOverlay, (state.runtimeConfig as any).activePartIndex ?? 0, mode, selectedDanceMode, (state.runtimeConfig as any).parts);
    return out;
  }

  if (mode === "pan") {
    const count = Math.min(instruments.length, PLATFORM_CAPS.gridHeight);
    for (let y = 0; y < count; y += 1) {
      const inst = instruments[y] ?? {};
      const isNone = (inst as any)?.type === "none";
      const { panPos, color } = resolveDancePanTarget(state as PlatformState<unknown>, y);
      const left = touchPanMarkerLeftCell(panPos);
      const cell = scaleLed(color, b);
      const dimmed = isNone ? scaleLed(cell, 0.25) : cell;
      out[GRID_DOMAIN.toDisplayIndex({ x: left, y })] = dimmed;
      out[GRID_DOMAIN.toDisplayIndex({ x: left + 1, y })] = dimmed;
    }
    // Only show FN navigation overlay when FN is held without Shift for navigation
    // When both FN and Shift are held, suppress the overlay as this is a system navigation command
    const showFnOverlay = state.system.fnHeld && !state.system.shiftHeld;
    overlayFnNavigation(out, b, showFnOverlay, (state.runtimeConfig as any).activePartIndex ?? 0, mode, (state.runtimeConfig as any).parts);
    return out;
  }

  if (mode === "trigger-gate") {
    const parts = (state.runtimeConfig as any).parts ?? [];
    const activePartIndex = (state.runtimeConfig as any).activePartIndex ?? 0;
    const target = state.system.triggerGateTarget ?? "active";

    let renderParts: number[];
    if (target === "all") {
      renderParts = Array.from({ length: parts.length }, (_, i) => i);
    } else if (target === "active") {
      renderParts = [activePartIndex];
    } else {
      const pi = parseInt(target, 10);
      renderParts = [isFinite(pi) ? clamp(pi, 0, parts.length - 1) : activePartIndex];
    }

    for (let y = 0; y < PLATFORM_CAPS.gridHeight; y += 1) {
      for (let x = 0; x < PLATFORM_CAPS.gridWidth; x += 1) {
        const idx = y * PLATFORM_CAPS.gridWidth + x;
        if (target === "all") {
          let anyOn = false, anyOff = false;
          for (const pi of renderParts) {
            const g = (parts[pi] as any)?.l1?.triggerGates;
            if (Array.isArray(g) ? g[idx] === false : false) anyOff = true;
            else anyOn = true;
          }
          const color = anyOn && anyOff ? { r: 180, g: 120, b: 0 } : anyOn ? { r: 0, g: 190, b: 90 } : { r: 60, g: 20, b: 20 };
          out[GRID_DOMAIN.toDisplayIndex({ x, y })] = scaleLed(color, b);
        } else {
          const pi = renderParts[0];
          const gates = (parts[pi] as any)?.l1?.triggerGates;
          const enabled = Array.isArray(gates) ? gates[idx] !== false : true;
          out[GRID_DOMAIN.toDisplayIndex({ x, y })] = scaleLed(enabled ? { r: 0, g: 190, b: 90 } : { r: 60, g: 20, b: 20 }, b);
        }
      }
    }
    const showFnOverlay = state.system.fnHeld && !state.system.shiftHeld;
    overlayFnNavigation(out, b, showFnOverlay, activePartIndex, mode, parts);
    return out;
  }

  if (mode === "xy") {
    const touch = (state.runtimeConfig as any).xyTouch as { x: number; y: number; active: boolean } | undefined;
    if (touch) {
      const tx = clamp(Math.round(touch.x * (PLATFORM_CAPS.gridWidth - 1)), 0, PLATFORM_CAPS.gridWidth - 1);
      const ty = clamp(Math.round(touch.y * (PLATFORM_CAPS.gridHeight - 1)), 0, PLATFORM_CAPS.gridHeight - 1);
      const ci = GRID_DOMAIN.toDisplayIndex({ x: tx, y: ty });
      if (touch.active) {
        out[ci] = scaleLed({ r: 255, g: 255, b: 255 }, b);
      } else {
        out[ci] = scaleLed({ r: 100, g: 100, b: 100 }, b);
      }
    }
    const showFnOverlay = state.system.fnHeld && !state.system.shiftHeld;
    overlayFnNavigation(out, b, showFnOverlay, (state.runtimeConfig as any).activePartIndex ?? 0, mode, (state.runtimeConfig as any).parts);
    return out;
  }

  const assignments = Array.isArray((state.runtimeConfig as any).touchFx?.assignments) ? ((state.runtimeConfig as any).touchFx.assignments as any[]) : [];
  const activeFx = Array.isArray(state.system.activeFx) ? state.system.activeFx : [];
  const activeKeys = new Set(activeFx.map((fx) => `${fx.cellX}:${fx.cellY}`));
  const full = activeFx.length >= PLATFORM_CAPS.touchFxMaxConcurrent;
  for (let y = 0; y < PLATFORM_CAPS.gridHeight; y += 1) {
    for (let x = 0; x < PLATFORM_CAPS.gridWidth; x += 1) {
      const screenIndex = GRID_DOMAIN.toDisplayIndex({ x, y });
      const assignment = assignments.find((a) => a?.x === x && a?.y === y);
      if (!assignment || assignment.config?.fxType === "none") {
        const worldIndex = GRID_DOMAIN.indexOf({ x, y });
        if (ghostCells?.[worldIndex]) {
          out[screenIndex] = scaleLed({ r: 0, g: 46, b: 25 }, b);
        } else {
          out[screenIndex] = scaleLed({ r: 15, g: 15, b: 22 }, b);
        }
        continue;
      }
      const active = activeKeys.has(`${x}:${y}`);
      const color = momentaryFxColor(assignment.config.fxType);
      if (active) out[screenIndex] = scaleLed(color, b);
      else if (full) out[screenIndex] = scaleLed({ r: 40, g: 40, b: 40 }, b);
      else out[screenIndex] = scaleLed(color, b);
    }
  }
  // Only show FN navigation overlay when FN is held without Shift for navigation
  // When both FN and Shift are held, suppress the overlay as this is a system navigation command
  const showFnOverlay = state.system.fnHeld && !state.system.shiftHeld;
  overlayFnNavigation(out, b, showFnOverlay, (state.runtimeConfig as any).activePartIndex ?? 0, mode, (state.runtimeConfig as any).parts);
  return out;
}

function scaleLed(cell: LedCell, brightness: number): LedCell {
  return {
    r: Math.round(cell.r * brightness),
    g: Math.round(cell.g * brightness),
    b: Math.round(cell.b * brightness)
  };
}

export function filterTriggerGatedIntents(intents: CellTriggerIntent[], state: PlatformState<any>, partIdx: number): CellTriggerIntent[] {
  const activeIdx = ((state.runtimeConfig as any).activePartIndex ?? 0) as number;
  if (state.system.triggerMuted && partIdx === activeIdx) return [];
  const gates = (state.runtimeConfig as any)?.parts?.[partIdx]?.l1?.triggerGates as boolean[] | undefined;
  if (!gates) return intents;
  return intents.filter(intent => {
    const idx = intent.y * PLATFORM_CAPS.gridWidth + intent.x;
    return gates[idx] !== false;
  });
}
