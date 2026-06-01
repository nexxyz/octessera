import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

export type LifeState = {
  width: typeof GRID_WIDTH;
  height: typeof GRID_HEIGHT;
  cells: boolean[];
  generation: number;
  randomCellsPerTick: number;
  randomTickInterval: number;
  spawnStep: number;
  tickCounter: number;
  triggerTypes: CellTriggerType[];
};

export type LifeConfig = {
  width?: typeof GRID_WIDTH;
  height?: typeof GRID_HEIGHT;
  randomCellsPerTick?: number;
  randomTickInterval?: number;
};

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

export const lifeBehavior: BehaviorEngine<LifeState, LifeConfig> = {
  id: "life",
  interpretInputTransitions: true,
  init(config): LifeState {
    return {
      width: GRID_WIDTH,
      height: GRID_HEIGHT,
      cells: new Array(CELL_COUNT).fill(false),
      generation: 0,
      randomCellsPerTick: config.randomCellsPerTick ?? 0,
      randomTickInterval: config.randomTickInterval ?? 1,
      spawnStep: 0,
      tickCounter: 0,
      triggerTypes: new Array(CELL_COUNT).fill("none")
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "spawnRandom") {
      const cells = state.cells.slice();
      const tt = state.triggerTypes.slice();
      for (let r = 0; r < 5; r++) {
        const rx = Math.floor(Math.random() * GRID_WIDTH);
        const ry = Math.floor(Math.random() * GRID_HEIGHT);
        const ri = GRID_DOMAIN.indexOf({ x: rx, y: ry });
        if (!cells[ri]) {
          cells[ri] = true;
          tt[ri] = "activate";
        }
      }
      return { ...state, cells, triggerTypes: tt };
    }
    if (input.type !== "grid_press") {
      return state;
    }
    if (input.x < 0 || input.x >= GRID_WIDTH || input.y < 0 || input.y >= GRID_HEIGHT) {
      return state;
    }
    const nextCells = GRID_DOMAIN.toggle(state.cells, { x: input.x, y: input.y });
    return {
      ...state,
      cells: nextCells
    };
  },
  onTick(state, context) {
    const nextCells = state.cells.slice();
    const triggerTypes = new Array<CellTriggerType>(CELL_COUNT).fill("none");

    for (let y = 0; y < GRID_HEIGHT; y += 1) {
      for (let x = 0; x < GRID_WIDTH; x += 1) {
        const i = GRID_DOMAIN.indexOf({ x, y });
        const alive = state.cells[i];
        const neighbors = countNeighbors(state.cells, x, y);
        const nextAlive = alive ? neighbors === 2 || neighbors === 3 : neighbors === 3;
        nextCells[i] = nextAlive;
        if (nextAlive && !alive) {
          triggerTypes[i] = "activate";
        } else if (!nextAlive && alive) {
          triggerTypes[i] = "deactivate";
        } else if (nextAlive && alive) {
          triggerTypes[i] = "stable";
        }
      }
    }

    let aliveCount = 0;
    for (let i = 0; i < CELL_COUNT; i += 1) {
      if (nextCells[i]) aliveCount += 1;
    }

    if (aliveCount > 0 && aliveCount % 12 === 0) {
      context.emit({ type: "note_on", channel: 0, note: 60 + (aliveCount % 12), velocity: 90, durationMs: 120 });
    }

    const nextTickCounter = state.tickCounter + 1;

    // Inject random cells if configured
    if (state.randomCellsPerTick > 0 && state.randomTickInterval > 0 && (nextTickCounter - 1) % state.randomTickInterval === state.spawnStep % state.randomTickInterval) {
      for (let r = 0; r < state.randomCellsPerTick; r += 1) {
        const rx = Math.floor(Math.random() * GRID_WIDTH);
        const ry = Math.floor(Math.random() * GRID_HEIGHT);
        const ri = GRID_DOMAIN.indexOf({ x: rx, y: ry });
        if (!nextCells[ri]) {
          nextCells[ri] = true;
          triggerTypes[ri] = "activate";
        }
      }
    }

    return {
      ...state,
      cells: nextCells,
      generation: state.generation + 1,
      tickCounter: nextTickCounter,
      triggerTypes
    };
  },
  renderModel(state) {
    return {
      name: "game of life",
      statusLine: `Gen ${state.generation}`,
      cells: state.cells,
      triggerTypes: state.triggerTypes
    };
  },
  configMenu() {
    return [
      { key: "randomCellsPerTick", label: "Spawn Count", type: "number", min: 0, max: 20, step: 1 },
      { key: "randomTickInterval", label: "Spawn Interval", type: "number", min: 1, max: 20, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "spawnRandom", label: "Spawn Random", type: "action" }
    ];
  },
  serialize(state) {
    return state;
  },
  deserialize(data) {
    const parsed = data as LifeState;
    return parsed;
  }
};

function countNeighbors(cells: boolean[], x: number, y: number): number {
  let count = 0;
  for (let oy = -1; oy <= 1; oy += 1) {
    for (let ox = -1; ox <= 1; ox += 1) {
      if (ox === 0 && oy === 0) {
        continue;
      }
      const nx = x + ox;
      const ny = y + oy;
      if (nx < 0 || nx >= GRID_WIDTH || ny < 0 || ny >= GRID_HEIGHT) {
        continue;
      }
      if (cells[GRID_DOMAIN.indexOf({ x: nx, y: ny })]) {
        count += 1;
      }
    }
  }
  return count;
}
