import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

export type BounceState = {
  balls: Array<{ x: number; y: number; vx: number; vy: number }>;
  cells: boolean[];
  triggerTypes: CellTriggerType[];
  maxBalls: number;
  spawnInterval: number;
  spawnStep: number;
  tickCounter: number;
};

export type BounceConfig = {
  maxBalls?: number;
  spawnInterval?: number;
};

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

export const bounceBehavior: BehaviorEngine<BounceState, BounceConfig> = {
  id: "bounce",
  interpretInputTransitions: true,
  init(config) {
    return {
      balls: [],
      cells: new Array(CELL_COUNT).fill(false),
      triggerTypes: new Array(CELL_COUNT).fill("none") as CellTriggerType[],
      maxBalls: config.maxBalls ?? 60,
      spawnInterval: config.spawnInterval ?? 0,
      spawnStep: 0,
      tickCounter: 0,
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "addBall") {
      if (state.balls.length >= state.maxBalls) return state;
      return {
        ...state,
        balls: [...state.balls, {
          x: Math.floor(Math.random() * GRID_WIDTH),
          y: Math.floor(Math.random() * GRID_HEIGHT),
          vx: (Math.random() - 0.5) * 2,
          vy: (Math.random() - 0.5) * 2,
        }],
      };
    }
    if (input.type !== "grid_press") return state;
    if (state.balls.length >= state.maxBalls) return state;
    return {
      ...state,
      balls: [...state.balls, {
        x: input.x,
        y: input.y,
        vx: (Math.random() - 0.5) * 2,
        vy: (Math.random() - 0.5) * 2,
      }],
    };
  },
  onTick(state) {
    const tickCounter = (state.tickCounter + 1) | 0;
    const spawnInterval = Math.max(0, Math.floor(state.spawnInterval));
    let balls0 = state.balls;
    if (spawnInterval > 0 && state.balls.length < state.maxBalls && (tickCounter - 1) % spawnInterval === state.spawnStep % spawnInterval) {
      balls0 = [...state.balls, {
        x: Math.floor(Math.random() * GRID_WIDTH),
        y: Math.floor(Math.random() * GRID_HEIGHT),
        vx: (Math.random() - 0.5) * 2,
        vy: (Math.random() - 0.5) * 2,
      }];
    }
    const balls = balls0.map(b => {
      let x = b.x + b.vx;
      let y = b.y + b.vy;
      let vx = b.vx;
      let vy = b.vy;
      if (x < 0) { x = -x; vx = -vx; }
      if (x >= GRID_WIDTH) { x = 2 * (GRID_WIDTH - 1) - x; vx = -vx; }
      if (y < 0) { y = -y; vy = -vy; }
      if (y >= GRID_HEIGHT) { y = 2 * (GRID_HEIGHT - 1) - y; vy = -vy; }
      return { x, y, vx, vy };
    });
    const cells = new Array<boolean>(CELL_COUNT).fill(false);
    for (const b of balls) {
      const cx = Math.min(GRID_WIDTH - 1, Math.max(0, Math.round(b.x)));
      const cy = Math.min(GRID_HEIGHT - 1, Math.max(0, Math.round(b.y)));
      cells[idx(cx, cy)] = true;
    }
    const tt = new Array<CellTriggerType>(CELL_COUNT).fill("none");
    for (let i = 0; i < CELL_COUNT; i++) {
      if (cells[i] && state.cells[i]) tt[i] = "stable";
      else if (cells[i]) tt[i] = "activate";
      else if (state.cells[i]) tt[i] = "deactivate";
    }
    return { balls, cells, triggerTypes: tt, maxBalls: state.maxBalls, spawnInterval: state.spawnInterval, spawnStep: state.spawnStep, tickCounter };
  },
  renderModel(state) {
    return {
      name: "bounce",
      statusLine: `${state.balls.length} ball${state.balls.length !== 1 ? "s" : ""}`,
      cells: state.cells,
      triggerTypes: state.triggerTypes,
    };
  },
  configMenu() {
    return [
      { key: "spawnInterval", label: "Spawn Interval", type: "number", min: 0, max: 30, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "maxBalls", label: "Max Balls", type: "number", min: 1, max: 100, step: 1 },
      { key: "addBall", label: "Add Ball", type: "action" },
    ];
  },
  serialize(state) { return state; },
  deserialize(data) { return data as BounceState; },
};
