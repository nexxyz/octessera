import type { MenuNode } from "./index";
import { FX_SLOT_TYPES } from "./fxDefaults";
import { PAN_POSITION_MAX, PLATFORM_CAPS } from "./platformCaps";
import { fxBusLabel } from "./coreUtils";

function duckSourceOptions(busIdx: number): string[] {
  const selfBus = `B${busIdx + 1}`;
  return [
    ...Array.from({ length: PLATFORM_CAPS.instrumentCount }, (_, i) => `I${i + 1}`),
    ...Array.from({ length: PLATFORM_CAPS.busCount }, (_, i) => `B${i + 1}`).filter(label => label !== selfBus)
  ];
}

const effectVisible = (busIdx: number, slotKey: string, type: string) => (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === type;

function fxSlotNode(busIdx: number, slotIdx: 1 | 2): MenuNode {
  const slotKey = slotIdx === 1 ? "slot1" : "slot2";
  return {
    kind: "group",
    label: `Slot ${slotIdx}`,
    children: [
      { kind: "enum", label: "Type", key: `mixer.buses.${busIdx}.${slotKey}.type`, options: FX_SLOT_TYPES },
      {
        kind: "group",
        label: "duck",
        visible: (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === "duck",
        children: [
          { kind: "enum", label: "Source", key: `mixer.buses.${busIdx}.${slotKey}.params.source`, options: duckSourceOptions(busIdx) },
          { kind: "number", label: "Threshold", key: `mixer.buses.${busIdx}.${slotKey}.params.threshold`, min: 0, max: 1, step: 0.01 },
          { kind: "number", label: "Amount %", key: `mixer.buses.${busIdx}.${slotKey}.params.amountPct`, min: 0, max: 100, step: 1 },
          { kind: "number", label: "Attack ms", key: `mixer.buses.${busIdx}.${slotKey}.params.attackMs`, min: 1, max: 500, step: 1 },
          { kind: "number", label: "Release ms", key: `mixer.buses.${busIdx}.${slotKey}.params.releaseMs`, min: 1, max: 5000, step: 5 }
        ]
      },
      {
        kind: "group",
        label: "delay",
        visible: (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === "delay",
        children: [
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 },
          { kind: "number", label: "Time ms", key: `mixer.buses.${busIdx}.${slotKey}.params.timeMs`, min: 1, max: 2000, step: 5 },
          { kind: "number", label: "Feedback", key: `mixer.buses.${busIdx}.${slotKey}.params.feedback`, min: 0, max: 0.98, step: 0.01 }
        ]
      },
      {
        kind: "group",
        label: "tremolo",
        visible: (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === "tremolo",
        children: [
          { kind: "number", label: "Rate Hz", key: `mixer.buses.${busIdx}.${slotKey}.params.rateHz`, min: 0.05, max: 40, step: 0.05 },
          { kind: "number", label: "Depth %", key: `mixer.buses.${busIdx}.${slotKey}.params.depthPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "saturator",
        visible: (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === "saturator",
        children: [
          { kind: "number", label: "Drive", key: `mixer.buses.${busIdx}.${slotKey}.params.drive`, min: 0, max: 20, step: 0.1 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "distortion",
        visible: (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === "distortion",
        children: [
          { kind: "number", label: "Drive", key: `mixer.buses.${busIdx}.${slotKey}.params.drive`, min: 0, max: 50, step: 0.5 },
          { kind: "number", label: "Clip", key: `mixer.buses.${busIdx}.${slotKey}.params.clip`, min: 0.05, max: 2, step: 0.05 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "bitcrusher",
        visible: (c: any) => c.mixer?.buses?.[busIdx]?.[slotKey]?.type === "bitcrusher",
        children: [
          { kind: "number", label: "Bits", key: `mixer.buses.${busIdx}.${slotKey}.params.bits`, min: 1, max: 16, step: 1 },
          { kind: "number", label: "Rate Div", key: `mixer.buses.${busIdx}.${slotKey}.params.rateDiv`, min: 1, max: 128, step: 1 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "mod delay",
        visible: (c: any) => ["vibrato", "chorus", "flanger"].includes(c.mixer?.buses?.[busIdx]?.[slotKey]?.type),
        children: [
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 },
          { kind: "number", label: "Rate Hz", key: `mixer.buses.${busIdx}.${slotKey}.params.rateHz`, min: 0.02, max: 20, step: 0.05 },
          { kind: "number", label: "Depth ms", key: `mixer.buses.${busIdx}.${slotKey}.params.depthMs`, min: 0, max: 40, step: 0.1 },
          { kind: "number", label: "Base ms", key: `mixer.buses.${busIdx}.${slotKey}.params.baseMs`, min: 0.1, max: 80, step: 0.1 },
          { kind: "number", label: "Feedback", key: `mixer.buses.${busIdx}.${slotKey}.params.feedback`, min: -0.95, max: 0.95, step: 0.01 }
        ]
      },
      {
        kind: "group",
        label: "filter lfo",
        visible: (c: any) => ["filter_lfo", "wah"].includes(c.mixer?.buses?.[busIdx]?.[slotKey]?.type),
        children: [
          { kind: "number", label: "Rate Hz", key: `mixer.buses.${busIdx}.${slotKey}.params.rateHz`, min: 0.02, max: 20, step: 0.05 },
          { kind: "number", label: "Center Hz", key: `mixer.buses.${busIdx}.${slotKey}.params.centerHz`, min: 40, max: 12000, step: 20 },
          { kind: "number", label: "Depth %", key: `mixer.buses.${busIdx}.${slotKey}.params.depthPct`, min: 0, max: 100, step: 1 },
          { kind: "number", label: "Q", key: `mixer.buses.${busIdx}.${slotKey}.params.q`, min: 0.25, max: 20, step: 0.25 }
        ]
      },
      {
        kind: "group",
        label: "reverb",
        visible: effectVisible(busIdx, slotKey, "reverb"),
        children: [
          { kind: "number", label: "Decay", key: `mixer.buses.${busIdx}.${slotKey}.params.decay`, min: 0, max: 0.995, step: 0.005 },
          { kind: "number", label: "Damp", key: `mixer.buses.${busIdx}.${slotKey}.params.damp`, min: 0, max: 0.98, step: 0.01 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "auto-pan",
        visible: effectVisible(busIdx, slotKey, "auto_pan"),
        children: [
          { kind: "number", label: "Rate Hz", key: `mixer.buses.${busIdx}.${slotKey}.params.rateHz`, min: 0.02, max: 20, step: 0.05 },
          { kind: "number", label: "Depth %", key: `mixer.buses.${busIdx}.${slotKey}.params.depthPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "glitch",
        visible: effectVisible(busIdx, slotKey, "glitch"),
        children: [
          { kind: "number", label: "Chance %", key: `mixer.buses.${busIdx}.${slotKey}.params.chancePct`, min: 0, max: 100, step: 1 },
          { kind: "number", label: "Slice ms", key: `mixer.buses.${busIdx}.${slotKey}.params.sliceMs`, min: 5, max: 500, step: 5 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "compressor",
        visible: effectVisible(busIdx, slotKey, "compressor"),
        children: [
          { kind: "number", label: "Threshold dB", key: `mixer.buses.${busIdx}.${slotKey}.params.thresholdDb`, min: -60, max: 0, step: 0.5 },
          { kind: "number", label: "Ratio", key: `mixer.buses.${busIdx}.${slotKey}.params.ratio`, min: 1, max: 20, step: 0.5 },
          { kind: "number", label: "Attack ms", key: `mixer.buses.${busIdx}.${slotKey}.params.attackMs`, min: 0.1, max: 200, step: 1 },
          { kind: "number", label: "Release ms", key: `mixer.buses.${busIdx}.${slotKey}.params.releaseMs`, min: 5, max: 2000, step: 5 },
          { kind: "number", label: "Makeup dB", key: `mixer.buses.${busIdx}.${slotKey}.params.makeupDb`, min: 0, max: 24, step: 0.5 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      },
      {
        kind: "group",
        label: "eq",
        visible: effectVisible(busIdx, slotKey, "eq"),
        children: [
          { kind: "number", label: "Low Gain dB", key: `mixer.buses.${busIdx}.${slotKey}.params.lowGainDb`, min: -12, max: 12, step: 0.5 },
          { kind: "number", label: "Mid Gain dB", key: `mixer.buses.${busIdx}.${slotKey}.params.midGainDb`, min: -12, max: 12, step: 0.5 },
          { kind: "number", label: "High Gain dB", key: `mixer.buses.${busIdx}.${slotKey}.params.highGainDb`, min: -12, max: 12, step: 0.5 },
          { kind: "number", label: "Mid Freq Hz", key: `mixer.buses.${busIdx}.${slotKey}.params.midFreqHz`, min: 40, max: 8000, step: 10 },
          { kind: "number", label: "Mid Q", key: `mixer.buses.${busIdx}.${slotKey}.params.midQ`, min: 0.25, max: 20, step: 0.25 },
          { kind: "number", label: "Mix %", key: `mixer.buses.${busIdx}.${slotKey}.params.mixPct`, min: 0, max: 100, step: 1 }
        ]
      }
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
        fxSlotNode(busIdx, 1),
        fxSlotNode(busIdx, 2),
        { kind: "number", label: "Pan Pos", key: `mixer.buses.${busIdx}.panPos`, min: 0, max: PAN_POSITION_MAX, step: 1 },
        { kind: "bool", label: "Auto Name", key: `mixer.buses.${busIdx}.autoName` },
        { kind: "text", label: "Name", key: `mixer.buses.${busIdx}.name`, maxLen: 32 }
      ]
    }))
  };
}
