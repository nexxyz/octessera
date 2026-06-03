export type MappableParam = {
  key: string;
  label: string;
  kind: "number" | "enum" | "bool";
  min?: number;
  max?: number;
  step?: number;
  options?: string[];
};

export function getMappableParams(): MappableParam[] {
  return [
    { key: "filterCutoff", label: "Filter Cutoff", kind: "number", min: 20, max: 20000, step: 1 },
    { key: "filterResonance", label: "Filter Resonance", kind: "number", min: 0, max: 20, step: 0.1 },
    { key: "velocityScalePct", label: "Velocity Scale %", kind: "number", min: 0, max: 100, step: 1 },
    { key: "gainPct", label: "Gain %", kind: "number", min: 0, max: 100, step: 1 },
    { key: "noteLengthMs", label: "Note Length ms", kind: "number", min: 1, max: 2000, step: 1 },
  ];
}