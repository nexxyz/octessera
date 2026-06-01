import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

export type DlaState = {
  cells: boolean[];
  triggerTypes: CellTriggerType[];
  spawnInterval: number;
  spawnStep: number;
  tickCounter: number;
};

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

function hasAdjacentCluster(cells: boolean[], x: number, y: number): boolean {
  for (let dy = -1; dy <= 1; dy++) {
    for (let dx = -1; dx <= 1; dx++) {
      if (dx === 0 && dy === 0) continue;
      const nx = x + dx;
      const ny = y + dy;
      if (nx < 0 || nx >= GRID_WIDTH || ny < 0 || ny >= GRID_HEIGHT) continue;
      if (cells[idx(nx, ny)]) return true;
    }
  }
  return false;
}

export type DlaConfig = {
  spawnInterval?: number;
};

export const dlaBehavior: BehaviorEngine<DlaState, DlaConfig> = {
  id: "dla",
  interpretInputTransitions: true,
  init(config) {
    const cells = new Array(CELL_COUNT).fill(false);
    const cx = Math.floor(GRID_WIDTH / 2);
    const cy = Math.floor(GRID_HEIGHT / 2);
    cells[idx(cx, cy)] = true;
    if (cx + 1 < GRID_WIDTH) cells[idx(cx + 1, cy)] = true;
    if (cy + 1 < GRID_HEIGHT) cells[idx(cx, cy + 1)] = true;
    return {
      cells,
      triggerTypes: new Array(CELL_COUNT).fill("none") as CellTriggerType[],
      spawnInterval: config.spawnInterval ?? 2,
      spawnStep: 0,
      tickCounter: 0,
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "seedCluster") {
      const cells = state.cells.slice();
      const cx = Math.floor(Math.random() * GRID_WIDTH);
      const cy = Math.floor(Math.random() * GRID_HEIGHT);
      cells[idx(cx, cy)] = true;
      if (cx + 1 < GRID_WIDTH) cells[idx(cx + 1, cy)] = true;
      if (cy + 1 < GRID_HEIGHT) cells[idx(cx, cy + 1)] = true;
      return { ...state, cells };
    }
    if (input.type !== "grid_press") return state;
    const i = idx(input.x, input.y);
    const cells = state.cells.slice();
    cells[i] = !cells[i];
    return { ...state, cells };
  },
  onTick(state) {
    const cells = state.cells.slice();
    const tickCounter = state.tickCounter + 1;
    if (state.spawnInterval > 0 && (tickCounter - 1) % state.spawnInterval === state.spawnStep % state.spawnInterval) {
      const edge = Math.floor(Math.random() * (2 * (GRID_WIDTH + GRID_HEIGHT)));
      let sx: number;
      let sy: number;
      if (edge < GRID_WIDTH) {
        sx = edge; sy = 0;
      } else if (edge < GRID_WIDTH + GRID_HEIGHT) {
        sx = GRID_WIDTH - 1; sy = edge - GRID_WIDTH;
      } else if (edge < 2 * GRID_WIDTH + GRID_HEIGHT) {
        sx = edge - GRID_WIDTH - GRID_HEIGHT; sy = GRID_HEIGHT - 1;
      } else {
        sx = 0; sy = edge - 2 * GRID_WIDTH - GRID_HEIGHT;
      }
      let px = sx;
      let py = sy;
      for (let step = 0; step < 200; step++) {
        if (hasAdjacentCluster(cells, px, py)) {
          cells[idx(px, py)] = true;
          break;
        }
        const dir = Math.floor(Math.random() * 4);
        if (dir === 0 && px > 0) px--;
        else if (dir === 1 && px < GRID_WIDTH - 1) px++;
        else if (dir === 2 && py > 0) py--;
        else if (dir === 3 && py < GRID_HEIGHT - 1) py++;
      }
    }
    const tt = new Array<CellTriggerType>(CELL_COUNT).fill("none");
    for (let i = 0; i < CELL_COUNT; i++) {
      if (cells[i] && state.cells[i]) tt[i] = "stable";
      else if (cells[i]) tt[i] = "activate";
      else if (state.cells[i]) tt[i] = "deactivate";
    }
    return { cells, triggerTypes: tt, spawnInterval: state.spawnInterval, spawnStep: state.spawnStep, tickCounter };
  },
  renderModel(state) {
    const count = state.cells.filter(Boolean).length;
    return {
      name: "dla",
      statusLine: `Cells: ${count}`,
      cells: state.cells,
      triggerTypes: state.triggerTypes,
    };
  },
  configMenu() {
    return [
      { key: "spawnInterval", label: "Spawn Interval", type: "number", min: 1, max: 20, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "seedCluster", label: "Seed Cluster", type: "action" }
    ];
  },
  serialize(state) { return state; },
  deserialize(data) { return data as DlaState; },
};
