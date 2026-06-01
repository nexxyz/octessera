import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

export type RaindropsState = {
  drops: Array<{ x: number; y: number }>;
  rings: Array<{ ox: number; oy: number; radius: number }>;
  cells: boolean[];
  triggerTypes: CellTriggerType[];
  autoDropInterval: number;
  splashRadius: number;
  spawnStep: number;
  tickCounter: number;
};

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

function inRing(cx: number, cy: number, ox: number, oy: number, r: number): boolean {
  const dx = cx - ox;
  const dy = cy - oy;
  const dist = Math.sqrt(dx * dx + dy * dy);
  return Math.abs(dist - r) < 0.6;
}

export type RaindropsConfig = {
  autoDropInterval?: number;
  splashRadius?: number;
};

export const raindropsBehavior: BehaviorEngine<RaindropsState, RaindropsConfig> = {
  id: "raindrops",
  interpretInputTransitions: true,
  init(config) {
    return {
      drops: [],
      rings: [],
      cells: new Array(CELL_COUNT).fill(false),
      triggerTypes: new Array(CELL_COUNT).fill("none") as CellTriggerType[],
      autoDropInterval: config.autoDropInterval ?? 3,
      splashRadius: config.splashRadius ?? 6,
      spawnStep: 0,
      tickCounter: 0,
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "dropNow") {
      return { ...state, drops: [...state.drops, { x: Math.floor(Math.random() * GRID_WIDTH), y: 0 }] };
    }
    if (input.type !== "grid_press") return state;
    if (input.y === 0) {
      return { ...state, drops: [...state.drops, { x: input.x, y: 0 }] };
    }
    return { ...state, rings: [...state.rings, { ox: input.x, oy: input.y, radius: 0 }] };
  },
  onTick(state) {
    const tickCounter = state.tickCounter + 1;
    let drops = state.drops.map(d => ({ ...d, y: d.y + 1 }));
    let rings = state.rings.map(r => ({ ...r, radius: r.radius + 1 })).filter(r => r.radius <= state.splashRadius);

    const landed: Array<{ x: number }> = [];
    drops = drops.filter(d => {
      if (d.y >= GRID_HEIGHT - 1) {
        landed.push({ x: d.x });
        return false;
      }
      return true;
    });
    if (state.splashRadius > 0) {
      for (const l of landed) {
        rings.push({ ox: l.x, oy: GRID_HEIGHT - 1, radius: 0 });
      }
    }

    if (state.autoDropInterval > 0 && (tickCounter - 1) % state.autoDropInterval === state.spawnStep % state.autoDropInterval) {
      drops.push({ x: Math.floor(Math.random() * GRID_WIDTH), y: 0 });
    }

    const cells = new Array<boolean>(CELL_COUNT).fill(false);
    for (const d of drops) cells[idx(d.x, d.y)] = true;
    for (const r of rings) {
      for (let y = 0; y < GRID_HEIGHT; y++) {
        for (let x = 0; x < GRID_WIDTH; x++) {
          if (inRing(x, y, r.ox, r.oy, r.radius)) {
            cells[idx(x, y)] = true;
          }
        }
      }
    }

    const tt = new Array<CellTriggerType>(CELL_COUNT).fill("none");
    for (let i = 0; i < CELL_COUNT; i++) {
      if (cells[i] && state.cells[i]) tt[i] = "stable";
      else if (cells[i]) tt[i] = "activate";
      else if (state.cells[i]) tt[i] = "deactivate";
    }
    return { drops, rings, cells, triggerTypes: tt, autoDropInterval: state.autoDropInterval, splashRadius: state.splashRadius, spawnStep: state.spawnStep, tickCounter };
  },
  renderModel(state) {
    return {
      name: "raindrops",
      statusLine: `Drops:${state.drops.length} Rings:${state.rings.length}`,
      cells: state.cells,
      triggerTypes: state.triggerTypes,
    };
  },
  configMenu() {
    return [
      { key: "autoDropInterval", label: "Spawn Interval", type: "number", min: 1, max: 20, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "splashRadius", label: "Splash Radius", type: "number", min: 0, max: 12, step: 1 },
      { key: "dropNow", label: "Drop Now", type: "action" },
    ];
  },
  serialize(state) { return state; },
  deserialize(data) { return data as RaindropsState; },
};
