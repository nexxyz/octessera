import { listBehaviorIds, type BehaviorEngine } from "@cellsymphony/behavior-api";
import type { AuxTurnBinding, MenuNode, PlatformState } from "./index";
import type { AuxPressBinding } from "./platformTypes";
import { instrumentLabel, partLabel } from "./coreUtils";
import { SYNTH_PRESETS } from "./synthPresets";
import { clampSampleSlotIndex, instrumentIndexOptions, PAN_POSITION_MAX, PLATFORM_CAPS, sampleSlotOptions, scanSectionOptions } from "./platformCaps";
import { fxBusesMenuNode, globalFxMenuNode } from "./fxBusMenu";
import { defaultMomentaryFxParams, MOMENTARY_FX_TYPES } from "./momentaryFx";
import { buildParamSkeleton } from "./menuParamTree";
import { buildL2SenseGroup, paramTargetGroup } from "./menuSense";

type MenuTreeDeps<TState> = {
  resolveBehavior: (id: string) => BehaviorEngine<any, any>;
  axisGroup: (label: string, prefix: string, defaultStep: number) => MenuNode;
  presetListNodes: (state: PlatformState<TState>, mode: "load" | "delete") => MenuNode[];
  presetRenameNodes: (state: PlatformState<TState>) => MenuNode[];
  midiOutputNodes: (state: PlatformState<TState>) => MenuNode[];
  midiInputNodes: (state: PlatformState<TState>) => MenuNode[];
  sampleBrowserNodes: (state: PlatformState<TState>, instrumentSlot: number, sampleSlot: number) => MenuNode[];
};


function partLabelFn<TState>(state: PlatformState<TState>, idx: number): string {
  const part = ((state.runtimeConfig as any).parts ?? [])[idx] ?? {};
  return partLabel(idx, part as any);
}

function l1PartGroup<TState>(state: PlatformState<TState>, deps: MenuTreeDeps<TState>, idx: number): MenuNode {
  const prefix = `parts.${idx}`;
  const part: any = ((state.runtimeConfig as any).parts ?? [])[idx] ?? {};
  const behaviorId = String(part?.l1?.behaviorId ?? "life");
  const engine = deps.resolveBehavior(behaviorId);
  const engineState = (state as any).partStates?.[idx] ?? state.behaviorState;
  const configNodes: MenuNode[] = [];
  if (engine.configMenu) {
    const items = engine.configMenu(engineState as any);
    for (const item of items) {
      if (item.type === "number") {
        configNodes.push({ kind: "number", label: item.label, key: `${prefix}.l1.behaviorConfig.${item.key}`, min: item.min ?? 0, max: item.max ?? 127, step: item.step ?? 1 });
      } else if (item.type === "bool") {
        configNodes.push({ kind: "bool", label: item.label, key: `${prefix}.l1.behaviorConfig.${item.key}` });
      } else if (item.type === "enum") {
        configNodes.push({ kind: "enum", label: item.label, key: `${prefix}.l1.behaviorConfig.${item.key}`, options: item.options ?? [] });
      } else if (item.type === "action") {
        configNodes.push({ kind: "action", label: item.label, action: { type: "behavior_action", behaviorId: engine.id, actionType: item.key } });
      }
    }
  }
  return {
    kind: "group",
    label: partLabelFn(state, idx),
    children: [
      { kind: "enum", label: "Behavior", key: `${prefix}.l1.behaviorId`, options: listBehaviorIds() },
      { kind: "enum", label: "Step Rate", key: `${prefix}.l1.stepRate`, options: ["1/16", "1/8", "1/4", "1/2", "1/1"] },
      ...configNodes,
      { kind: "bool", label: "Save Grid State", key: `${prefix}.l1.saveGridState` },
      { kind: "bool", label: "Auto Name", key: `${prefix}.autoName` },
      { kind: "text", label: "Part Name", key: `${prefix}.name`, maxLen: 32 }
    ]
  } satisfies MenuNode;
}

export function buildMenuTree<TState>(state: PlatformState<TState>, deps: MenuTreeDeps<TState>): MenuNode {
  const instrumentSlotOptions = instrumentIndexOptions();
  const sampleSlots = sampleSlotOptions();

  const partCount = PLATFORM_CAPS.partCount;
  const instLabel = (idx: number): string => instrumentLabel(state, idx);
  const selectedFxType = ((state.runtimeConfig as any).touchFx?.selected?.fxType ?? "stutter") as any;
  const selectedFxParams = (state.runtimeConfig as any).touchFx?.selected?.params ?? defaultMomentaryFxParams(selectedFxType);
  const selectedFxTargetKey = String((state.runtimeConfig as any).touchFx?.selected?.targetKey ?? "master");
  const selectedFxConfig = { fxType: selectedFxType, params: structuredClone(selectedFxParams), targetKey: selectedFxTargetKey };
  const momentaryTargetOptions = [
    "master",
    ...Array.from({ length: PLATFORM_CAPS.busCount }, (_, i) => `fx_bus_${i + 1}`),
    ...Array.from({ length: PLATFORM_CAPS.instrumentCount }, (_, i) => `instrument_${i + 1}`)
  ];

      const rootChildren: MenuNode[] = [
      { kind: "group", label: "L1: Life", children: Array.from({ length: partCount }, (_, idx) => l1PartGroup(state, deps, idx)) },
      buildL2SenseGroup(state, { resolveBehavior: deps.resolveBehavior, axisGroup: deps.axisGroup }, partCount, instrumentSlotOptions, sampleSlots, selectedFxConfig, partLabelFn),
      {
        kind: "group",
        label: "L3: Voice",
        children: [
          {
            kind: "group",
            label: "Instruments",
            children: Array.from({ length: instrumentSlotOptions.length }, (_, idx) => {
              const prefix = `instruments.${idx}`;
              return {
                kind: "group",
                label: instLabel(idx),
                children: [
                  { kind: "enum", label: "Type", key: `${prefix}.type`, options: ["none", "synth", "sampler", "midi"] },
                  { kind: "enum", label: "Note Behavior", key: `${prefix}.noteBehavior`, options: ["oneshot", "hold"] },
                  {
                    kind: "group",
                    label: "Synth",
                    flat: true,
                    visible: (c: any) => (c.instruments?.[idx]?.type ?? "synth") === "synth",
                    children: [
                      {
                        kind: "group",
                        label: "Preset",
                        children: [
                          {
                            kind: "group",
                            label: "Load",
                        children: SYNTH_PRESETS.map((preset) => ({
                          kind: "action",
                          label: preset.label,
                          action: { type: "synth_preset_load", slot: idx, presetId: preset.id, presetLabel: preset.label }
                        }))
                          }
                        ]
                      },
                      {
                        kind: "group",
                        label: "Oscillator",
                        children: [
                          {
                            kind: "group",
                            label: "Osc 1",
                            children: [
                              { kind: "enum", label: "Wave", key: `${prefix}.synth.osc1.waveform`, options: ["sine", "triangle", "saw", "square", "pulse"] },
                              { kind: "number", label: "Octave", key: `${prefix}.synth.osc1.octave`, min: -2, max: 2, step: 1 },
                              { kind: "number", label: "Level", key: `${prefix}.synth.osc1.levelPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Detune", key: `${prefix}.synth.osc1.detuneCents`, min: -50, max: 50, step: 1, displayStyle: "marker" },
                              { kind: "number", label: "Pulse Width", key: `${prefix}.synth.osc1.pulseWidthPct`, min: 5, max: 95, step: 1 }
                            ]
                          },
                          {
                            kind: "group",
                            label: "Osc 2",
                            children: [
                              { kind: "enum", label: "Wave", key: `${prefix}.synth.osc2.waveform`, options: ["sine", "triangle", "saw", "square", "pulse"] },
                              { kind: "number", label: "Octave", key: `${prefix}.synth.osc2.octave`, min: -2, max: 2, step: 1 },
                              { kind: "number", label: "Level", key: `${prefix}.synth.osc2.levelPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Detune", key: `${prefix}.synth.osc2.detuneCents`, min: -50, max: 50, step: 1, displayStyle: "marker" },
                              { kind: "number", label: "Pulse Width", key: `${prefix}.synth.osc2.pulseWidthPct`, min: 5, max: 95, step: 1 }
                            ]
                          }
                        ]
                      },
                      {
                        kind: "group",
                        label: "Filter",
                        children: [
                          { kind: "enum", label: "Type", key: `${prefix}.synth.filter.type`, options: ["lowpass", "highpass", "bandpass", "notch"] },
                          { kind: "number", label: "Cutoff", key: `${prefix}.synth.filter.cutoffHz`, min: 0, max: 255, step: 1 },
                          { kind: "number", label: "Res", key: `${prefix}.synth.filter.resonance`, min: 0, max: 255, step: 1 },
                          { kind: "number", label: "Env Amt", key: `${prefix}.synth.filter.envAmountPct`, min: -100, max: 100, step: 1, displayStyle: "marker" },
                          { kind: "number", label: "Key Track", key: `${prefix}.synth.filter.keyTrackingPct`, min: 0, max: 100, step: 1 },
                          {
                            kind: "group",
                            label: "Envelope",
                            children: [
                              { kind: "number", label: "Attack", key: `${prefix}.synth.filterEnv.attackMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Decay", key: `${prefix}.synth.filterEnv.decayMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Sustain", key: `${prefix}.synth.filterEnv.sustainPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Release", key: `${prefix}.synth.filterEnv.releaseMs`, min: 0, max: 8000, step: 5 }
                            ]
                          }
                        ]
                      },
                      {
                        kind: "group",
                        label: "Volume",
                        children: [
                          {
                            kind: "group",
                            label: "Amp",
                            children: [
                              { kind: "number", label: "Gain", key: `${prefix}.synth.amp.gainPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Vel Sens", key: `${prefix}.synth.amp.velocitySensitivityPct`, min: 0, max: 100, step: 1 }
                            ]
                          },
                          {
                            kind: "group",
                            label: "Envelope",
                            children: [
                              { kind: "number", label: "Attack", key: `${prefix}.synth.ampEnv.attackMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Decay", key: `${prefix}.synth.ampEnv.decayMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Sustain", key: `${prefix}.synth.ampEnv.sustainPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Release", key: `${prefix}.synth.ampEnv.releaseMs`, min: 0, max: 8000, step: 5 }
                            ]
                          }
                        ]
                      }
                    ]
                  },
                  {
                    kind: "group",
                    label: "Sampler",
                    flat: true,
                    visible: (c: any) => c.instruments?.[idx]?.type === "sampler",
                    children: [
                      { kind: "enum", label: "Sample Slot", key: `${prefix}.sample.selectedSlot`, options: sampleSlots },
                      {
                        kind: "group",
                        label: "Choose Sample",
                        children: (s) => {
                          const ss = clampSampleSlotIndex((s.runtimeConfig as any).instruments?.[idx]?.sample?.selectedSlot ?? 0);
                          return deps.sampleBrowserNodes(s, idx, ss);
                        }
                      },
                      {
                        kind: "action",
                        label: "Assign",
                        action: {
                          type: "sample_assign_enter",
                          instrumentSlot: idx,
                          sampleSlot: clampSampleSlotIndex((state.runtimeConfig as any).instruments?.[idx]?.sample?.selectedSlot ?? 0)
                        }
                      },
                      { kind: "bool", label: "Velocity Levels", key: `${prefix}.sample.velocityLevelsEnabled` },
                      { kind: "number", label: "Level High", key: `${prefix}.sample.velocityLevels.high`, min: 1, max: 127, step: 1, visible: (c: any) => c.instruments?.[idx]?.sample?.velocityLevelsEnabled === true },
                      { kind: "number", label: "Level Medium", key: `${prefix}.sample.velocityLevels.medium`, min: 1, max: 127, step: 1, visible: (c: any) => c.instruments?.[idx]?.sample?.velocityLevelsEnabled === true },
                      { kind: "number", label: "Level Low", key: `${prefix}.sample.velocityLevels.low`, min: 1, max: 127, step: 1, visible: (c: any) => c.instruments?.[idx]?.sample?.velocityLevelsEnabled === true },
                      { kind: "number", label: "Base Velocity", key: `${prefix}.sample.baseVelocity`, min: 1, max: 127, step: 1 },
                      { kind: "number", label: "Tune Semis", key: `${prefix}.sample.tuneSemis`, min: -24, max: 24, step: 1, displayStyle: "marker" },
                      {
                        kind: "group",
                        label: "Filter",
                        children: [
                          { kind: "enum", label: "Type", key: `${prefix}.sample.filter.type`, options: ["lowpass", "highpass", "bandpass", "notch"] },
                          { kind: "number", label: "Cutoff", key: `${prefix}.sample.filter.cutoffHz`, min: 0, max: 255, step: 1 },
                          { kind: "number", label: "Res", key: `${prefix}.sample.filter.resonance`, min: 0, max: 255, step: 1 },
                          { kind: "number", label: "Env Amt", key: `${prefix}.sample.filter.envAmountPct`, min: -100, max: 100, step: 1, displayStyle: "marker" },
                          { kind: "number", label: "Key Track", key: `${prefix}.sample.filter.keyTrackingPct`, min: 0, max: 100, step: 1 },
                          {
                            kind: "group",
                            label: "Envelope",
                            children: [
                              { kind: "number", label: "Attack", key: `${prefix}.sample.filterEnv.attackMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Decay", key: `${prefix}.sample.filterEnv.decayMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Sustain", key: `${prefix}.sample.filterEnv.sustainPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Release", key: `${prefix}.sample.filterEnv.releaseMs`, min: 0, max: 8000, step: 5 }
                            ]
                          }
                        ]
                      },
                      {
                        kind: "group",
                        label: "Volume",
                        children: [
                          {
                            kind: "group",
                            label: "Amp",
                            children: [
                              { kind: "number", label: "Gain", key: `${prefix}.sample.amp.gainPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Vel Sens", key: `${prefix}.sample.amp.velocitySensitivityPct`, min: 0, max: 100, step: 1 }
                            ]
                          },
                          {
                            kind: "group",
                            label: "Envelope",
                            children: [
                              { kind: "number", label: "Attack", key: `${prefix}.sample.ampEnv.attackMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Decay", key: `${prefix}.sample.ampEnv.decayMs`, min: 0, max: 5000, step: 5 },
                              { kind: "number", label: "Sustain", key: `${prefix}.sample.ampEnv.sustainPct`, min: 0, max: 100, step: 1 },
                              { kind: "number", label: "Release", key: `${prefix}.sample.ampEnv.releaseMs`, min: 0, max: 8000, step: 5 }
                            ]
                          }
                        ]
                      }
                    ]
                  },
                  {
                    kind: "group",
                    label: "Note Settings",
                    flat: true,
                    visible: (c: any) => c.instruments?.[idx]?.type === "midi",
                    children: [
                      { kind: "number", label: "Velocity", key: `${prefix}.midiEngine.velocity`, min: 1, max: 127, step: 1 },
                      { kind: "number", label: "Duration", key: `${prefix}.midiEngine.durationMs`, min: 10, max: 2000, step: 10 }
                    ]
                  },
                  {
                    kind: "group",
                    label: "Mixer",
                    visible: (c: any) => c.instruments?.[idx]?.type !== "midi",
                    children: [
                      { kind: "enum", label: "Route", key: `${prefix}.mixer.route`, options: ["direct", ...Array.from({ length: PLATFORM_CAPS.busCount }, (_, i) => `fx_bus_${i + 1}`)] },
                      { kind: "number", label: "Volume", key: `${prefix}.mixer.volume`, min: 0, max: 100, step: 1, displayStyle: "bar" },
                      { kind: "number", label: "Pan Pos", key: `${prefix}.mixer.panPos`, min: 0, max: PAN_POSITION_MAX, step: 1, displayStyle: "marker" }
                    ]
                  },
                  { kind: "action", label: "Clone", action: { type: "instrument_clone", slot: idx } },
                  { kind: "action", label: "Reset", action: { type: "instrument_reset", slot: idx } },
                  {
                    kind: "group",
                    label: "MIDI",
                    children: [
                      { kind: "bool", label: "Enabled", key: `${prefix}.midi.enabled` },
                      { kind: "number", label: "Channel", key: `${prefix}.midi.channel`, min: 0, max: 15, step: 1 }
                    ]
                  },
                  { kind: "bool", label: "Auto Name", key: `${prefix}.autoName` },
                  { kind: "text", label: "Name", key: `${prefix}.name`, maxLen: 32 }
                ]
              } satisfies MenuNode;
            })
          },
          fxBusesMenuNode(state),
          globalFxMenuNode()
        ]
      },
      {
        kind: "group",
        label: "L4: Dance",
        children: (state) => {
          const dm = (state.runtimeConfig as any).danceMode ?? state.system.danceMode;
          const page: MenuNode[] = [];
          if (dm === "fx") {
            page.push({
              kind: "group",
              label: "FX Page",
              flat: true,
              children: [
                { kind: "enum", label: "FX Type", key: "touchFx.selected.fxType", options: MOMENTARY_FX_TYPES },
                { kind: "enum", label: "Target", key: "touchFx.selected.targetKey", options: momentaryTargetOptions },
                { kind: "number", label: "Rate Hz", key: "touchFx.selected.params.rateHz", min: 1, max: 32, step: 1, visible: (c: any) => c.touchFx?.selected?.fxType === "stutter" },
                { kind: "number", label: "Depth", key: "touchFx.selected.params.depthPct", min: 0, max: 100, step: 1, visible: (c: any) => c.touchFx?.selected?.fxType === "stutter" },
                { kind: "number", label: "Release Ms", key: "touchFx.selected.params.releaseMs", min: 10, max: 5000, step: 10, visible: (c: any) => c.touchFx?.selected?.fxType === "freeze" },
                { kind: "number", label: "Mix", key: "touchFx.selected.params.mixPct", min: 0, max: 100, step: 1, visible: (c: any) => c.touchFx?.selected?.fxType === "freeze" || c.touchFx?.selected?.fxType === "pitch_shift" },
                { kind: "number", label: "Cutoff", key: "touchFx.selected.params.cutoffPct", min: 0, max: 100, step: 1, visible: (c: any) => c.touchFx?.selected?.fxType === "filter_sweep" },
                { kind: "number", label: "Res", key: "touchFx.selected.params.resonancePct", min: 0, max: 100, step: 1, visible: (c: any) => c.touchFx?.selected?.fxType === "filter_sweep" },
                { kind: "number", label: "Sweep In", key: "touchFx.selected.params.sweepInMs", min: 10, max: 3000, step: 10, visible: (c: any) => c.touchFx?.selected?.fxType === "filter_sweep" },
                { kind: "number", label: "Sweep Out", key: "touchFx.selected.params.sweepOutMs", min: 10, max: 3000, step: 10, visible: (c: any) => c.touchFx?.selected?.fxType === "filter_sweep" },
                { kind: "number", label: "Semitones", key: "touchFx.selected.params.semitones", min: -24, max: 24, step: 1, displayStyle: "marker", visible: (c: any) => c.touchFx?.selected?.fxType === "pitch_shift" },
                { kind: "number", label: "Cents", key: "touchFx.selected.params.cents", min: -100, max: 100, step: 1, displayStyle: "marker", visible: (c: any) => c.touchFx?.selected?.fxType === "pitch_shift" },
                { kind: "action", label: "Map to Grid", action: { type: "fx_assign_enter", config: selectedFxConfig } }
              ]
            });
          } else if (dm === "trigger-gate") {
            page.push({
              kind: "group",
              label: "Trigger Gate",
              flat: true,
              children: [
                { kind: "group", label: "Mode Grid", children: [] }
              ]
            });
          } else if (dm === "xy") {
            const activeIdx = (state.runtimeConfig as any).activePartIndex ?? 0;
            page.push(
              paramTargetGroup(
                "X Axis",
                (s) => (((s.runtimeConfig as any).parts?.[(s.runtimeConfig as any).activePartIndex ?? 0]?.xy?.x ?? null) as AuxTurnBinding | null),
                (binding) => ({ type: "xy_set_target", axis: "x", binding })
              ),
              { kind: "bool", label: "Invert X", key: `parts.${activeIdx}.xy.xInvert` },
              paramTargetGroup(
                "Y Axis",
                (s) => (((s.runtimeConfig as any).parts?.[(s.runtimeConfig as any).activePartIndex ?? 0]?.xy?.y ?? null) as AuxTurnBinding | null),
                (binding) => ({ type: "xy_set_target", axis: "y", binding })
              ),
              { kind: "bool", label: "Invert Y", key: `parts.${activeIdx}.xy.yInvert` },
              { kind: "enum", label: "Release", key: "xyRelease", options: ["sample-hold", "reset-center"] }
            );
          }
          return [
            { kind: "enum", label: "Dance Page", key: "danceMode", options: ["none", "mix", "pan", "fx", "trigger-gate", "xy"] },
            { kind: "number", label: "BPM", key: "transport.bpm", min: 40, max: 240, step: 1 },
            ...page
          ];
        }
      },
      { kind: "spacer" },
      {
        kind: "group",
        label: "System",
        children: [
          {
            kind: "group",
            label: "Saves",
            children: [
              {
                kind: "group",
                label: "Library",
                children: [
                  { kind: "group", label: "Save As", children: [{ kind: "text", label: "Name", key: "system.draftName", maxLen: 32, onExitSaveAction: { type: "preset_save" } }, { kind: "action", label: "Save", action: { type: "preset_save" } }] },
                  { kind: "action", label: "Save Current", action: { type: "preset_save_current" } },
                  { kind: "group", label: "Load", children: (s) => deps.presetListNodes(s, "load") },
                  { kind: "group", label: "Rename", children: (s) => deps.presetRenameNodes(s) },
                  { kind: "group", label: "Delete", children: (s) => deps.presetListNodes(s, "delete") },
                  { kind: "action", label: "Refresh List", action: { type: "refresh_presets" } }
                ]
              },
              { kind: "group", label: "Default", children: [{ kind: "action", label: "Save Default", action: { type: "default_save" } }, { kind: "action", label: "Load Default", action: { type: "default_load" } }, { kind: "bool", label: "Auto Save", key: "autoSaveDefault" }] },
              { kind: "group", label: "Factory", children: [{ kind: "action", label: "Load Fact. Default", action: { type: "factory_load" } }] }
            ]
          },
          {
            kind: "group",
            label: "Sound",
            children: [
              { kind: "number", label: "Master Vol", key: "masterVolume", min: 0, max: 100, step: 1 },
              { kind: "number", label: "Note Length", key: "sound.noteLengthMs", min: 30, max: 2000, step: 10 },
              { kind: "number", label: "Velocity Scale", key: "sound.velocityScalePct", min: 0, max: 200, step: 5 },
              { kind: "enum", label: "Velocity Curve", key: "sound.velocityCurve", options: ["linear", "soft", "hard"] },
          { kind: "enum", label: "Voice Stealing", key: "sound.voiceStealingMode", options: ["off", "lenient", "balanced", "aggressive"] }
            ]
          },
          {
            kind: "group",
            label: "MIDI",
            children: [
              { kind: "bool", label: "Enabled", key: "midi.enabled" },
              { kind: "action", label: "Panic", action: { type: "midi_panic" } },
              { kind: "group", label: "MIDI Out", children: (s) => deps.midiOutputNodes(s) },
              { kind: "group", label: "MIDI In", children: (s) => deps.midiInputNodes(s) },
              { kind: "group", label: "Sync & Clock", children: [{ kind: "enum", label: "Sync Mode", key: "midi.syncMode", options: ["internal", "external"] }, { kind: "bool", label: "Clock Out", key: "midi.clockOutEnabled" }, { kind: "bool", label: "Clock In", key: "midi.clockInEnabled" }, { kind: "bool", label: "Respond Start/Stop", key: "midi.respondToStartStop" }] }
            ]
          },
          { kind: "group", label: "UI", children: [{ kind: "bool", label: "Ghost Cells", key: "ghostCells" }, { kind: "bool", label: "Input Events While Paused", key: "inputEventsWhilePaused" }, { kind: "enum", label: "Numeric Display", key: "numericDisplayMode", options: ["bar", "numbers", "bar+numbers"] }, { kind: "number", label: "Screen Sleep", key: "screenSleepSeconds", min: 0, max: 600, step: 10 }, { kind: "number", label: "Display Brightness", key: "displayBrightness", min: 10, max: 100, step: 5, displayStyle: "bar" }, { kind: "number", label: "Grid Brightness", key: "gridBrightness", min: 10, max: 100, step: 5, displayStyle: "bar" }, { kind: "number", label: "Button Brightness", key: "buttonBrightness", min: 10, max: 100, step: 5, displayStyle: "bar" }] }
        ]
      }
    ];
  buildParamSkeleton(rootChildren);
  return { kind: "group", label: "Root", children: rootChildren };
}
