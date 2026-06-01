import type { BehaviorEngine } from "./index";

const registry = new Map<string, BehaviorEngine<any, any>>();

export function registerBehavior(engine: BehaviorEngine<any, any>): void {
  registry.set(engine.id, engine);
}

export function getBehavior(id: string): BehaviorEngine<any, any> | undefined {
  return registry.get(id);
}

export function listBehaviorIds(): string[] {
  return Array.from(registry.keys());
}
