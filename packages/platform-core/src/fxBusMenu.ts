import type { MenuNode } from "./index";
import { FX_SLOT_TYPES, GLOBAL_FX_SLOT_TYPES } from "./fxDefaults";
import { PAN_POSITION_MAX, PLATFORM_CAPS } from "./platformCaps";
import { fxBusLabel } from "./coreUtils";

type FxSlotContext = {
  typeKey: string;
  paramsPrefix: string;
  typeOptions: string[];
  duckSourceOptions?: string[];
};

function duckSourceOptions(busIdx: number): string[] {
  const selfBus = `B${busIdx + 1}`;
  return [
    ...Array.from({ length: PLATFORM_CAPS.instrumentCount }, (_, i) => `I${i + 1}`),
    ...Array.from({ length: PLATFORM_CAPS.busCount }, (_, i) => `B${i + 1}`).filter((label) => label !== selfBus)
  ];
}

function readType(config: any, typeKey: string): string {
  const parts = typeKey.split(".");
  let cursor: any = config;
  for (const part of parts) cursor = cursor?.[part];
  return String(cursor ?? "none");
}

function effectVisible(typeKey: string, type: string) {
  return (config: any) => readType(config, typeKey) === type;
}

function effectIn(typeKey: string, types: string[]) {
  return (config: any) => types.includes(readType(config, typeKey));
}

function fxParamGroups(ctx: FxSlotContext): MenuNode[] {
  const groups: MenuNode[] = [];
  if (ctx.typeOptions.includes("duck") && ctx.duckSourceOptions) {
    groups.push({
      kind: "group",
      label: "duck",
      flat: true,
      visible: effectVisible(ctx.typeKey, "duck"),
      children: [
        { kind: "enum", label: "Source", key: `${ctx.paramsPrefix}.source`, options: ctx.duckSourceOptions },
        { kind: "number", label: "Threshold", key: `${ctx.paramsPrefix}.threshold`, min: 0, max: 1, step: 0.01 },
        { kind: "number", label: "Amount %", key: `${ctx.paramsPrefix}.amountPct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Attack ms", key: `${ctx.paramsPrefix}.attackMs`, min: 1, max: 500, step: 1 },
        { kind: "number", label: "Release ms", key: `${ctx.paramsPrefix}.releaseMs`, min: 1, max: 5000, step: 5 }
      ]
    });
  }
  if (ctx.typeOptions.includes("delay")) {
    groups.push({
      kind: "group",
      label: "delay",
      flat: true,
      visible: effectVisible(ctx.typeKey, "delay"),
      children: [
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Time ms", key: `${ctx.paramsPrefix}.timeMs`, min: 1, max: 2000, step: 5 },
        { kind: "number", label: "Feedback", key: `${ctx.paramsPrefix}.feedback`, min: 0, max: 0.98, step: 0.01 }
      ]
    });
  }
  if (ctx.typeOptions.includes("tremolo")) {
    groups.push({
      kind: "group",
      label: "tremolo",
      flat: true,
      visible: effectVisible(ctx.typeKey, "tremolo"),
      children: [
        { kind: "number", label: "Rate Hz", key: `${ctx.paramsPrefix}.rateHz`, min: 0.05, max: 40, step: 0.05 },
        { kind: "number", label: "Depth %", key: `${ctx.paramsPrefix}.depthPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("saturator")) {
    groups.push({
      kind: "group",
      label: "saturator",
      flat: true,
      visible: effectVisible(ctx.typeKey, "saturator"),
      children: [
        { kind: "number", label: "Drive", key: `${ctx.paramsPrefix}.drive`, min: 0, max: 20, step: 0.1 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("distortion")) {
    groups.push({
      kind: "group",
      label: "distortion",
      flat: true,
      visible: effectVisible(ctx.typeKey, "distortion"),
      children: [
        { kind: "number", label: "Drive", key: `${ctx.paramsPrefix}.drive`, min: 0, max: 50, step: 0.5 },
        { kind: "number", label: "Clip", key: `${ctx.paramsPrefix}.clip`, min: 0.05, max: 2, step: 0.05 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("bitcrusher")) {
    groups.push({
      kind: "group",
      label: "bitcrusher",
      flat: true,
      visible: effectVisible(ctx.typeKey, "bitcrusher"),
      children: [
        { kind: "number", label: "Bits", key: `${ctx.paramsPrefix}.bits`, min: 1, max: 16, step: 1 },
        { kind: "number", label: "Rate Div", key: `${ctx.paramsPrefix}.rateDiv`, min: 1, max: 128, step: 1 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.some((type) => ["vibrato", "chorus", "flanger"].includes(type))) {
    groups.push({
      kind: "group",
      label: "mod delay",
      flat: true,
      visible: effectIn(ctx.typeKey, ["vibrato", "chorus", "flanger"]),
      children: [
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Rate Hz", key: `${ctx.paramsPrefix}.rateHz`, min: 0.02, max: 20, step: 0.05 },
        { kind: "number", label: "Depth ms", key: `${ctx.paramsPrefix}.depthMs`, min: 0, max: 40, step: 0.1 },
        { kind: "number", label: "Base ms", key: `${ctx.paramsPrefix}.baseMs`, min: 0.1, max: 80, step: 0.1 },
        { kind: "number", label: "Feedback", key: `${ctx.paramsPrefix}.feedback`, min: -0.95, max: 0.95, step: 0.01, displayStyle: "marker" }
      ]
    });
  }
  if (ctx.typeOptions.some((type) => ["filter_lfo", "wah"].includes(type))) {
    groups.push({
      kind: "group",
      label: "filter lfo",
      flat: true,
      visible: effectIn(ctx.typeKey, ["filter_lfo", "wah"]),
      children: [
        { kind: "number", label: "Rate Hz", key: `${ctx.paramsPrefix}.rateHz`, min: 0.02, max: 20, step: 0.05 },
        { kind: "number", label: "Center Hz", key: `${ctx.paramsPrefix}.centerHz`, min: 40, max: 12000, step: 20 },
        { kind: "number", label: "Depth %", key: `${ctx.paramsPrefix}.depthPct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Q", key: `${ctx.paramsPrefix}.q`, min: 0.25, max: 20, step: 0.25 }
      ]
    });
  }
  if (ctx.typeOptions.includes("reverb")) {
    groups.push({
      kind: "group",
      label: "reverb",
      flat: true,
      visible: effectVisible(ctx.typeKey, "reverb"),
      children: [
        { kind: "number", label: "Decay", key: `${ctx.paramsPrefix}.decay`, min: 0, max: 0.995, step: 0.005 },
        { kind: "number", label: "Damp", key: `${ctx.paramsPrefix}.damp`, min: 0, max: 0.98, step: 0.01 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("auto_pan")) {
    groups.push({
      kind: "group",
      label: "auto-pan",
      flat: true,
      visible: effectVisible(ctx.typeKey, "auto_pan"),
      children: [
        { kind: "number", label: "Rate Hz", key: `${ctx.paramsPrefix}.rateHz`, min: 0.02, max: 20, step: 0.05 },
        { kind: "number", label: "Depth %", key: `${ctx.paramsPrefix}.depthPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("glitch")) {
    groups.push({
      kind: "group",
      label: "glitch",
      flat: true,
      visible: effectVisible(ctx.typeKey, "glitch"),
      children: [
        { kind: "number", label: "Chance %", key: `${ctx.paramsPrefix}.chancePct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Slice ms", key: `${ctx.paramsPrefix}.sliceMs`, min: 5, max: 500, step: 5 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("compressor")) {
    groups.push({
      kind: "group",
      label: "compressor",
      flat: true,
      visible: effectVisible(ctx.typeKey, "compressor"),
      children: [
        { kind: "number", label: "Threshold dB", key: `${ctx.paramsPrefix}.thresholdDb`, min: -60, max: 0, step: 0.5 },
        { kind: "number", label: "Ratio", key: `${ctx.paramsPrefix}.ratio`, min: 1, max: 20, step: 0.5 },
        { kind: "number", label: "Attack ms", key: `${ctx.paramsPrefix}.attackMs`, min: 0.1, max: 200, step: 1 },
        { kind: "number", label: "Release ms", key: `${ctx.paramsPrefix}.releaseMs`, min: 5, max: 2000, step: 5 },
        { kind: "number", label: "Makeup dB", key: `${ctx.paramsPrefix}.makeupDb`, min: 0, max: 24, step: 0.5 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("eq")) {
    groups.push({
      kind: "group",
      label: "eq",
      flat: true,
      visible: effectVisible(ctx.typeKey, "eq"),
      children: [
        { kind: "number", label: "Low Gain dB", key: `${ctx.paramsPrefix}.lowGainDb`, min: -12, max: 12, step: 0.5, displayStyle: "marker" },
        { kind: "number", label: "Mid Gain dB", key: `${ctx.paramsPrefix}.midGainDb`, min: -12, max: 12, step: 0.5, displayStyle: "marker" },
        { kind: "number", label: "High Gain dB", key: `${ctx.paramsPrefix}.highGainDb`, min: -12, max: 12, step: 0.5, displayStyle: "marker" },
        { kind: "number", label: "Mid Freq Hz", key: `${ctx.paramsPrefix}.midFreqHz`, min: 40, max: 8000, step: 10 },
        { kind: "number", label: "Mid Q", key: `${ctx.paramsPrefix}.midQ`, min: 0.25, max: 20, step: 0.25 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  if (ctx.typeOptions.includes("vinyl")) {
    groups.push({
      kind: "group",
      label: "vinyl",
      flat: true,
      visible: effectVisible(ctx.typeKey, "vinyl"),
      children: [
        { kind: "number", label: "Saturation %", key: `${ctx.paramsPrefix}.saturationPct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Crackle %", key: `${ctx.paramsPrefix}.cracklePct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Warp Depth %", key: `${ctx.paramsPrefix}.warpDepthPct`, min: 0, max: 100, step: 1 },
        { kind: "number", label: "Mix %", key: `${ctx.paramsPrefix}.mixPct`, min: 0, max: 100, step: 1 }
      ]
    });
  }
  return groups;
}

function fxSlotNode(label: string, ctx: FxSlotContext): MenuNode {
  return {
    kind: "group",
    label,
    children: [
      { kind: "enum", label: "Type", key: ctx.typeKey, options: ctx.typeOptions },
      ...fxParamGroups(ctx)
    ]
  };
}

export function fxBusesMenuNode(state?: any): MenuNode {
  const busGroupLabel = (busIdx: number): string => {
    const bus = state?.runtimeConfig?.mixer?.buses?.[busIdx];
    return bus ? fxBusLabel(busIdx, bus) : `Bus ${busIdx + 1}`;
  };
  return {
    kind: "group",
    label: "FX Buses",
    children: Array.from({ length: PLATFORM_CAPS.busCount }, (_, busIdx): MenuNode => ({
      kind: "group",
      label: busGroupLabel(busIdx),
      children: [
        fxSlotNode("Slot 1", {
          typeKey: `mixer.buses.${busIdx}.slot1.type`,
          paramsPrefix: `mixer.buses.${busIdx}.slot1.params`,
          typeOptions: FX_SLOT_TYPES,
          duckSourceOptions: duckSourceOptions(busIdx)
        }),
        fxSlotNode("Slot 2", {
          typeKey: `mixer.buses.${busIdx}.slot2.type`,
          paramsPrefix: `mixer.buses.${busIdx}.slot2.params`,
          typeOptions: FX_SLOT_TYPES,
          duckSourceOptions: duckSourceOptions(busIdx)
        }),
        { kind: "number", label: "Pan Pos", key: `mixer.buses.${busIdx}.panPos`, min: 0, max: PAN_POSITION_MAX, step: 1, displayStyle: "marker" },
        { kind: "bool", label: "Auto Name", key: `mixer.buses.${busIdx}.autoName` },
        { kind: "text", label: "Name", key: `mixer.buses.${busIdx}.name`, maxLen: 32 }
      ]
    }))
  };
}

export function globalFxMenuNode(): MenuNode {
  return {
    kind: "group",
    label: "Global FX",
    children: Array.from({ length: PLATFORM_CAPS.globalFxSlotCount }, (_, slotIdx) => fxSlotNode(`Slot ${slotIdx + 1}`, {
      typeKey: `mixer.master.slots.${slotIdx}.type`,
      paramsPrefix: `mixer.master.slots.${slotIdx}.params`,
      typeOptions: GLOBAL_FX_SLOT_TYPES
    }))
  };
}
