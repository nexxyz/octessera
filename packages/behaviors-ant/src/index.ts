import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

export type AntState = {
  ants: Array<{ x: number; y: number; dir: number }>;
  cells: boolean[];
  triggerTypes: CellTriggerType[];
  maxAnts: number;
  autoSpawnInterval: number;
  spawnStep: number;
  tickCounter: number;
};

export type AntConfig = {
  maxAnts?: number;
  autoSpawnInterval?: number;
};

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

export const antBehavior: BehaviorEngine<AntState, AntConfig> = {
  id: "ant",
  interpretInputTransitions: true,
  init(config) {
    return {
      ants: [],
      cells: new Array(CELL_COUNT).fill(false),
      triggerTypes: new Array(CELL_COUNT).fill("none") as CellTriggerType[],
      maxAnts: config.maxAnts ?? 50,
      autoSpawnInterval: config.autoSpawnInterval ?? 0,
      spawnStep: 0,
      tickCounter: 0,
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "spawnAnt") {
      if (state.ants.length >= state.maxAnts) return state;
      return { ...state, ants: [...state.ants, { x: Math.floor(Math.random() * GRID_WIDTH), y: Math.floor(Math.random() * GRID_HEIGHT), dir: 0 }] };
    }
    if (input.type !== "grid_press") return state;
    if (state.ants.length >= state.maxAnts) return state;
    return { ...state, ants: [...state.ants, { x: input.x, y: input.y, dir: 0 }] };
  },
  onTick(state) {
    const cells = state.cells.slice();
    const tickCounter = state.tickCounter + 1;
    const ants = state.ants.map(ant => {
      const i = idx(ant.x, ant.y);
      const isBlack = state.cells[i];
      const newDir = isBlack ? (ant.dir + 3) % 4 : (ant.dir + 1) % 4;
      let nx = ant.x;
      let ny = ant.y;
      if (newDir === 0) ny = (ant.y - 1 + GRID_HEIGHT) % GRID_HEIGHT;
      else if (newDir === 1) nx = (ant.x + 1) % GRID_WIDTH;
      else if (newDir === 2) ny = (ant.y + 1) % GRID_HEIGHT;
      else nx = (ant.x - 1 + GRID_WIDTH) % GRID_WIDTH;
      return { x: nx, y: ny, dir: newDir };
    });
    for (const ant of state.ants) {
      cells[idx(ant.x, ant.y)] = !cells[idx(ant.x, ant.y)];
    }

    let finalAnts = ants;
    if (state.autoSpawnInterval > 0 && (tickCounter - 1) % state.autoSpawnInterval === state.spawnStep % state.autoSpawnInterval && ants.length < state.maxAnts) {
      finalAnts = [...ants, { x: Math.floor(Math.random() * GRID_WIDTH), y: Math.floor(Math.random() * GRID_HEIGHT), dir: 0 }];
    }

    const tt = new Array<CellTriggerType>(CELL_COUNT).fill("none");
    for (let i = 0; i < CELL_COUNT; i++) {
      if (state.cells[i] === cells[i]) continue;
      tt[i] = cells[i] ? "activate" : "deactivate";
    }
    return { ants: finalAnts, cells, triggerTypes: tt, maxAnts: state.maxAnts, autoSpawnInterval: state.autoSpawnInterval, spawnStep: state.spawnStep, tickCounter };
  },
  renderModel(state) {
    const vis = state.cells.slice();
    for (const ant of state.ants) vis[idx(ant.x, ant.y)] = true;
    return {
      name: "ant",
      statusLine: `${state.ants.length} ant${state.ants.length !== 1 ? "s" : ""}`,
      cells: vis,
      triggerTypes: state.triggerTypes,
    };
  },
  configMenu() {
    return [
      { key: "maxAnts", label: "Max Ants", type: "number", min: 1, max: 100, step: 1 },
      { key: "autoSpawnInterval", label: "Spawn Interval", type: "number", min: 0, max: 20, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "spawnAnt", label: "Spawn Ant", type: "action" },
    ];
  },
  serialize(state) { return state; },
  deserialize(data) { return data as AntState; },
};
