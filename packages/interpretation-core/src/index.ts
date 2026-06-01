export type GridSnapshot = {
  width: number;
  height: number;
  cells: boolean[];
};

export type CellTransitionKind = "activate" | "deactivate";
export type CellTriggerKind = CellTransitionKind | "stable" | "scanned" | "scanned_empty";

export type CellTransition = {
  x: number;
  y: number;
  kind: CellTransitionKind;
};

export type TickStrategy =
  | { mode: "whole_grid_transitions" }
  | { mode: "whole_grid_active" }
  | { mode: "scan_column_active"; sections?: number }
  | { mode: "scan_row_active"; sections?: number };

export type AxisStrategy =
  | { mode: "scale_step"; step: number }
  | { mode: "timing_only" }
  | { mode: "ignore" };

export type InterpretationProfile = {
  id: string;
  event: { enabled: boolean };
  state: { enabled: boolean; tick: TickStrategy };
  x: AxisStrategy;
  y: AxisStrategy;
};

export type CellTriggerIntent = {
  x: number;
  y: number;
  kind: CellTriggerKind;
  degree: number;
};

export function extractTransitions(previous: GridSnapshot, next: GridSnapshot): CellTransition[] {
  const transitions: CellTransition[] = [];
  const len = Math.min(previous.cells.length, next.cells.length);
  for (let i = 0; i < len; i += 1) {
    const before = previous.cells[i];
    const after = next.cells[i];
    if (before === after) {
      continue;
    }
    const x = i % previous.width;
    const y = Math.floor(i / previous.width);
    transitions.push({ x, y, kind: after ? "activate" : "deactivate" });
  }
  return transitions;
}

export function interpretGrid(
  previous: GridSnapshot,
  next: GridSnapshot,
  tick: number,
  profile: InterpretationProfile
): CellTriggerIntent[] {
  const eventCandidates = profile.event.enabled
    ? selectEventCandidates(previous, next)
    : [];
  const stateCandidates = profile.state.enabled
    ? selectStateCandidates(next, tick, profile.state.tick)
    : [];

  return [...eventCandidates, ...stateCandidates].map((intent) => ({
    ...intent,
    degree: computeDegree(next.height, intent.x, intent.y, profile)
  }));
}

function selectEventCandidates(
  previous: GridSnapshot,
  next: GridSnapshot
): Array<{ x: number; y: number; kind: CellTriggerKind }> {
  return extractTransitions(previous, next);
}

function selectStateCandidates(
  next: GridSnapshot,
  tick: number,
  strategy: TickStrategy
): Array<{ x: number; y: number; kind: CellTriggerKind }> {
  if (strategy.mode === "whole_grid_active") {
    const out: Array<{ x: number; y: number; kind: CellTriggerKind }> = [];
    for (let y = 0; y < next.height; y += 1) {
      for (let x = 0; x < next.width; x += 1) {
        if (next.cells[y * next.width + x]) {
          out.push({ x, y, kind: "scanned" });
        }
      }
    }
    return out;
  }

  if (strategy.mode === "scan_column_active") {
    const sections = sectionCount(strategy.sections, next.width);
    if (sections > 1) return scanColumnSections(next, tick, sections);
    const column = tick % next.width;
    const out: Array<{ x: number; y: number; kind: CellTriggerKind }> = [];
    for (let y = 0; y < next.height; y += 1) {
        if (next.cells[y * next.width + column]) {
          out.push({ x: column, y, kind: "scanned" });
        } else {
          out.push({ x: column, y, kind: "scanned_empty" });
        }
      }
      return out;
  }

  if (strategy.mode === "whole_grid_transitions") {
    return [];
  }

  const sections = sectionCount(strategy.sections, next.height);
  if (sections > 1) return scanRowSections(next, tick, sections);

  const row = tick % next.height;
  const out: Array<{ x: number; y: number; kind: CellTriggerKind }> = [];
  for (let x = 0; x < next.width; x += 1) {
    if (next.cells[row * next.width + x]) {
      out.push({ x, y: row, kind: "scanned" });
    } else {
      out.push({ x, y: row, kind: "scanned_empty" });
    }
  }
  return out;
}

function scanRowSections(next: GridSnapshot, tick: number, sections: number): Array<{ x: number; y: number; kind: CellTriggerKind }> {
  const sectionHeight = Math.max(1, Math.floor(next.height / sections));
  const step = tick % (next.width * sections);
  const section = Math.floor(step / next.width);
  const x = step % next.width;
  const firstY = next.height - (section + 1) * sectionHeight;
  const out: Array<{ x: number; y: number; kind: CellTriggerKind }> = [];
  for (let dy = 0; dy < sectionHeight && firstY + dy < next.height; dy += 1) {
    const y = firstY + dy;
    out.push({ x, y, kind: next.cells[y * next.width + x] ? "scanned" : "scanned_empty" });
  }
  return out;
}

function scanColumnSections(next: GridSnapshot, tick: number, sections: number): Array<{ x: number; y: number; kind: CellTriggerKind }> {
  const sectionWidth = Math.max(1, Math.floor(next.width / sections));
  const step = tick % (next.height * sections);
  const section = Math.floor(step / next.height);
  const y = step % next.height;
  const firstX = section * sectionWidth;
  const out: Array<{ x: number; y: number; kind: CellTriggerKind }> = [];
  for (let dx = 0; dx < sectionWidth && firstX + dx < next.width; dx += 1) {
    const x = firstX + dx;
    out.push({ x, y, kind: next.cells[y * next.width + x] ? "scanned" : "scanned_empty" });
  }
  return out;
}

function sectionCount(value: number | undefined, size: number): number {
  if (value === 2 || value === 4 || value === 8) return Math.min(value, size);
  return 1;
}

function computeDegree(gridHeight: number, x: number, y: number, profile: InterpretationProfile): number {
  const rowFromBottom = Math.max(0, Math.min(gridHeight - 1, y));
  const xPart = axisValue(profile.x, x);
  const yPart = axisValue(profile.y, rowFromBottom);
  return xPart + yPart;
}

function axisValue(strategy: AxisStrategy, value: number): number {
  if (strategy.mode === "scale_step") {
    return value * Math.max(0, Math.floor(strategy.step));
  }
  return 0;
}
