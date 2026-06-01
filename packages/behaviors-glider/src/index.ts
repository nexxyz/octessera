import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

const GLIDER_OFFSETS: Array<[number, number]> = [
  [1, 0], [2, 1], [0, 2], [1, 2], [2, 2]
];

export type GliderState = {
  cells: boolean[];
  triggerTypes: CellTriggerType[];
  spawnInterval: number;
  spawnStep: number;
  tickCounter: number;
};

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

export type GliderConfig = {
  spawnInterval?: number;
};

export const gliderBehavior: BehaviorEngine<GliderState, GliderConfig> = {
  id: "glider",
  interpretInputTransitions: true,
  init(config) {
    return {
      cells: new Array(CELL_COUNT).fill(false),
      triggerTypes: new Array(CELL_COUNT).fill("none") as CellTriggerType[],
      spawnInterval: config.spawnInterval ?? 8,
      spawnStep: 0,
      tickCounter: 0,
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "spawnGlider") {
      const cells = state.cells.slice();
      const ox = Math.floor(Math.random() * (GRID_WIDTH - 2));
      const oy = Math.floor(Math.random() * (GRID_HEIGHT - 2));
      for (const [dx, dy] of GLIDER_OFFSETS) {
        cells[idx(ox + dx, oy + dy)] = true;
      }
      return { ...state, cells };
    }
    if (input.type === "grid_press") {
      const cells = state.cells.slice();
      const ox = Math.min(input.x, GRID_WIDTH - 3);
      const oy = Math.min(input.y, GRID_HEIGHT - 3);
      for (const [dx, dy] of GLIDER_OFFSETS) {
        cells[idx(ox + dx, oy + dy)] = true;
      }
      return { ...state, cells };
    }
    return state;
  },
  onTick(state) {
    const cells = state.cells.slice();
    const tickCounter = state.tickCounter + 1;

    if (state.spawnInterval > 0 && (tickCounter - 1) % state.spawnInterval === state.spawnStep % state.spawnInterval) {
      const ox = Math.floor(Math.random() * (GRID_WIDTH - 2));
      const oy = Math.floor(Math.random() * (GRID_HEIGHT - 2));
      for (const [dx, dy] of GLIDER_OFFSETS) {
        cells[idx(ox + dx, oy + dy)] = true;
      }
    }

    const next = new Array<boolean>(CELL_COUNT).fill(false);
    for (let y = 0; y < GRID_HEIGHT; y++) {
      for (let x = 0; x < GRID_WIDTH; x++) {
        const i = idx(x, y);
        let neighbors = 0;
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            if (dx === 0 && dy === 0) continue;
            if (cells[idx((x + dx + GRID_WIDTH) % GRID_WIDTH, (y + dy + GRID_HEIGHT) % GRID_HEIGHT)]) neighbors++;
          }
        }
        if (cells[i]) {
          next[i] = neighbors === 2 || neighbors === 3;
        } else {
          next[i] = neighbors === 3;
        }
      }
    }

    const tt = new Array<CellTriggerType>(CELL_COUNT).fill("none");
    for (let i = 0; i < CELL_COUNT; i++) {
      if (next[i] && state.cells[i]) tt[i] = "stable";
      else if (next[i]) tt[i] = "activate";
      else if (state.cells[i]) tt[i] = "deactivate";
    }

    return { cells: next, triggerTypes: tt, spawnInterval: state.spawnInterval, spawnStep: state.spawnStep, tickCounter };
  },
  renderModel(state) {
    const count = state.cells.filter(Boolean).length;
    return {
      name: "glider",
      statusLine: `Cells: ${count}`,
      cells: state.cells,
      triggerTypes: state.triggerTypes,
    };
  },
  configMenu() {
    return [
      { key: "spawnInterval", label: "Spawn Interval", type: "number", min: 0, max: 30, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "spawnGlider", label: "Spawn Glider", type: "action" },
    ];
  },
  serialize(state) { return state; },
  deserialize(data) { return data as GliderState; },
};
