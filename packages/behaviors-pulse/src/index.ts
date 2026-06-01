import type { BehaviorEngine, CellTriggerType } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

const CELL_COUNT = GRID_WIDTH * GRID_HEIGHT;

const SHAPE_NAMES = ["ring", "filled", "diamond", "cross", "x"] as const;
export type PulseShapeKind = typeof SHAPE_NAMES[number];

export type PulseState = {
  pulses: Array<{ ox: number; oy: number; radius: number; maxRadius: number }>;
  lifetimes: number[];
  triggerTypes: CellTriggerType[];
  pulseShape: PulseShapeKind;
  lifespan: number;
  maxRadius: number;
  autoPulseInterval: number;
  spawnStep: number;
  tickCounter: number;
};

export type PulseConfig = {
  pulseShape?: PulseShapeKind;
  lifespan?: number;
  maxRadius?: number;
  autoPulseInterval?: number;
};

function idx(x: number, y: number): number {
  return GRID_DOMAIN.indexOf({ x, y });
}

function inShape(cx: number, cy: number, ox: number, oy: number, r: number, shape: PulseShapeKind): boolean {
  const dx = cx - ox;
  const dy = cy - oy;
  const dist = Math.sqrt(dx * dx + dy * dy);
  const adx = Math.abs(dx);
  const ady = Math.abs(dy);
  switch (shape) {
    case "ring":
      return Math.abs(dist - r) < 0.6;
    case "filled":
      return dist <= r + 0.5;
    case "diamond":
      return adx + ady <= r;
    case "cross":
      return (adx === r && ady <= 1) || (ady === r && adx <= 1);
    case "x":
      return adx === ady && adx <= r;
  }
}

export const shapesBehavior: BehaviorEngine<PulseState, PulseConfig> = {
  id: "shapes",
  interpretInputTransitions: true,
  init(config) {
    return {
      pulses: [],
      lifetimes: new Array(CELL_COUNT).fill(0),
      triggerTypes: new Array(CELL_COUNT).fill("none") as CellTriggerType[],
      pulseShape: config.pulseShape ?? "ring",
      lifespan: config.lifespan ?? 3,
      maxRadius: config.maxRadius ?? 12,
      autoPulseInterval: config.autoPulseInterval ?? 0,
      spawnStep: 0,
      tickCounter: 0,
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type === "behavior_action" && input.actionType === "spawnPulse") {
      const ox = Math.floor(Math.random() * GRID_WIDTH);
      const oy = Math.floor(Math.random() * GRID_HEIGHT);
      const lifetimes = state.lifetimes.slice();
      const coords = [...shapeCells(state.pulseShape, ox, oy, 0)];
      for (const i of coords) {
        lifetimes[i] = state.lifespan;
      }
      return {
        ...state,
        pulses: [...state.pulses, { ox, oy, radius: 0, maxRadius: state.maxRadius }],
        lifetimes,
      };
    }
    if (input.type !== "grid_press") return state;
    const lifetimes = state.lifetimes.slice();
    const coords = [...shapeCells(state.pulseShape, input.x, input.y, 0)];
    for (const i of coords) {
      lifetimes[i] = state.lifespan;
    }
    return {
      ...state,
      pulses: [...state.pulses, { ox: input.x, oy: input.y, radius: 0, maxRadius: state.maxRadius }],
      lifetimes,
    };
  },
  onTick(state) {
    const prevLifetimes = state.lifetimes;

    const expandedPulses = state.pulses.map(p => ({ ...p, radius: p.radius + 1 }));

    const wavefront = new Set<number>();
    for (const p of expandedPulses) {
      if (p.radius > p.maxRadius) continue;
      const cur = shapeCells(state.pulseShape, p.ox, p.oy, p.radius);
      const prev = new Set(shapeCells(state.pulseShape, p.ox, p.oy, p.radius - 1));
      for (const i of cur) {
        if (!prev.has(i)) wavefront.add(i);
      }
    }

    const pulses = expandedPulses.filter(p => p.radius <= p.maxRadius);

    const lifetimes = prevLifetimes.slice();
    for (let i = 0; i < CELL_COUNT; i++) {
      if (lifetimes[i] > 0) lifetimes[i]--;
    }
    for (const i of wavefront) {
      lifetimes[i] = state.lifespan;
    }

    const tt = new Array<CellTriggerType>(CELL_COUNT).fill("none");
    for (let i = 0; i < CELL_COUNT; i++) {
      const nowOn = lifetimes[i] > 0;
      const wasOn = prevLifetimes[i] > 0;
      if (nowOn && !wasOn) tt[i] = "activate";
      else if (!nowOn && wasOn) tt[i] = "deactivate";
      else if (nowOn && wasOn) tt[i] = "stable";
    }

    const tickCounter = state.tickCounter + 1;
    let finalPulses = pulses;
    if (state.autoPulseInterval > 0 && (tickCounter - 1) % state.autoPulseInterval === state.spawnStep % state.autoPulseInterval) {
      const ox = Math.floor(Math.random() * GRID_WIDTH);
      const oy = Math.floor(Math.random() * GRID_HEIGHT);
      const coords = shapeCells(state.pulseShape, ox, oy, 0);
      for (const i of coords) {
        lifetimes[i] = state.lifespan;
      }
      finalPulses = [...pulses, { ox, oy, radius: 0, maxRadius: state.maxRadius }];
    }

    return {
      ...state,
      pulses: finalPulses,
      lifetimes,
      triggerTypes: tt,
      tickCounter,
    };
  },
  renderModel(state) {
    return {
      name: "shapes",
      statusLine: `${state.pulses.length} pulse${state.pulses.length !== 1 ? "s" : ""} [${state.pulseShape}]`,
      cells: state.lifetimes.map(l => l > 0),
      triggerTypes: state.triggerTypes,
    };
  },
  configMenu() {
    return [
      { key: "pulseShape", label: "Shape", type: "enum", options: [...SHAPE_NAMES] },
      { key: "lifespan", label: "Lifespan", type: "number", min: 1, max: 12, step: 1 },
      { key: "maxRadius", label: "Max Radius", type: "number", min: 4, max: 32, step: 1 },
      { key: "autoPulseInterval", label: "Spawn Interval", type: "number", min: 0, max: 20, step: 1 },
      { key: "spawnStep", label: "Spawn Step", type: "number", min: 0, max: 63, step: 1 },
      { key: "spawnPulse", label: "Spawn Pulse", type: "action" },
    ];
  },
  serialize(state) { return state; },
  deserialize(data) { return data as PulseState; },
};

function shapeCells(shape: PulseShapeKind, ox: number, oy: number, r: number): number[] {
  const out: number[] = [];
  for (let y = 0; y < GRID_HEIGHT; y++) {
    for (let x = 0; x < GRID_WIDTH; x++) {
      if (inShape(x, y, ox, oy, r, shape)) {
        out.push(idx(x, y));
      }
    }
  }
  return out;
}
