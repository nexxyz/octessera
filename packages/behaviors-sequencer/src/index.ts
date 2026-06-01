import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import { GRID_DOMAIN, GRID_HEIGHT, GRID_WIDTH, type DeviceInput } from "@cellsymphony/device-contracts";

export type SequencerState = {
  width: typeof GRID_WIDTH;
  height: typeof GRID_HEIGHT;
  cells: boolean[];
};

export const sequencerBehavior: BehaviorEngine<SequencerState, {}> = {
  id: "sequencer",
  init() {
    return {
      width: GRID_WIDTH,
      height: GRID_HEIGHT,
      cells: new Array(GRID_WIDTH * GRID_HEIGHT).fill(false)
    };
  },
  onInput(state, input: DeviceInput) {
    if (input.type !== "grid_press") return state;
    return { ...state, cells: GRID_DOMAIN.toggle(state.cells, { x: input.x, y: input.y }) };
  },
  onTick(state) {
    return state;
  },
  renderModel(state) {
    return {
      name: "sequencer",
      statusLine: "Manual",
      cells: state.cells
    };
  },
  serialize(state) {
    return state;
  },
  deserialize(data) {
    return data as SequencerState;
  }
};
