import type { DeviceInput, GridInteraction } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";

export type CellTriggerType = "activate" | "stable" | "deactivate" | "scanned" | "none";

export type BehaviorContext = {
  bpm: number;
  emit: (event: MusicalEvent) => void;
};

export type BehaviorRenderModel = {
  name: string;
  statusLine: string;
  cells: boolean[];
  triggerTypes?: CellTriggerType[];
};

export type BehaviorConfigItem = {
  key: string;
  label: string;
  type: "number" | "bool" | "enum" | "action";
  min?: number;
  max?: number;
  step?: number;
  options?: string[];
};

export interface BehaviorEngine<State, Config> {
  id: string;
  init(config: Config): State;
  onInput(state: State, input: DeviceInput, context: BehaviorContext): State;
  onTick(state: State, context: BehaviorContext): State;
  renderModel(state: State): BehaviorRenderModel;
  serialize(state: State): unknown;
  deserialize(data: unknown): State;
  configMenu?(state: State): BehaviorConfigItem[];
  interpretInputTransitions?: boolean;
  gridInteraction?: GridInteraction;
}

export { registerBehavior, getBehavior, listBehaviorIds } from "./registry";
