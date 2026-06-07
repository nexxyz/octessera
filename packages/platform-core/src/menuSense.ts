import type { BehaviorEngine } from "@cellsymphony/behavior-api";
import type { AuxTurnBinding, MenuNode, PlatformState } from "./index";
import type { AuxPressBinding } from "./platformTypes";
import { PLATFORM_CAPS, scanSectionOptions } from "./platformCaps";
import { buildParamSkeleton, withParamActions } from "./menuParamTree";
import { compactSourcePathFromKey } from "./menuView";
import { auxPathFromPress } from "./auxBindings";

type SenseMenuDeps<TState> = {
  resolveBehavior: (id: string) => BehaviorEngine<any, any>;
  axisGroup: (label: string, prefix: string, defaultStep: number) => MenuNode;
};

function bindingDetail<TState>(state: PlatformState<TState>, binding: AuxTurnBinding | null | undefined): string | null {
  if (!binding?.key) return null;
  const path = compactSourcePathFromKey(state, binding.key);
  return path ?? binding.label ?? binding.key;
}

export function paramTargetGroup<TState>(
  label: string,
  detailBinding: (state: PlatformState<TState>) => AuxTurnBinding | null | undefined,
  onSelect: (binding: AuxTurnBinding | null) => any
): MenuNode {
  return {
    kind: "group",
    label,
    children: () => [
      { kind: "action", label: "(none)", action: onSelect(null) },
      ...withParamActions(buildParamSkeleton(), (binding) => onSelect(binding))
    ],
    detail: (state) => bindingDetail(state, detailBinding(state as PlatformState<TState>))
  } satisfies MenuNode;
}

function clickBindingDetail<TState>(state: PlatformState<TState>, binding: AuxPressBinding | null | undefined): string | null {
  if (!binding) return null;
  return auxPathFromPress(state, binding) ?? binding.label ?? null;
}

function clickActionGroup<TState>(
  label: string,
  detailBinding: (state: PlatformState<TState>) => AuxPressBinding | null | undefined,
  onSelect: (binding: AuxPressBinding | null) => any,
  actions: MenuNode[]
): MenuNode {
  return {
    kind: "group",
    label,
    children: [
      { kind: "action", label: "(none)", action: onSelect(null) },
      ...actions
    ],
    detail: (state) => clickBindingDetail(state, detailBinding(state as PlatformState<TState>))
  } satisfies MenuNode;
}

function bindableClickActions<TState>(
  state: PlatformState<TState>,
  onSelect: (binding: AuxPressBinding) => any,
  selectedFxConfig: { fxType: any; params: any; targetKey: string },
  sampleSlots: string[],
  resolveBehavior: (id: string) => any
): MenuNode[] {
  const partActions: MenuNode[] = Array.from({ length: PLATFORM_CAPS.partCount }, (_, idx) => {
    const part = ((state.runtimeConfig as any).parts ?? [])[idx] ?? {};
    const behaviorId = String(part?.l1?.behaviorId ?? "life");
    const behavior = resolveBehavior(behaviorId);
    const items = behavior.configMenu ? behavior.configMenu(behavior.init({})) : [];
    const actionNodes = items
      .filter((item: any) => item.type === "action")
      .map((item: any) => ({
        kind: "action" as const,
        label: item.label,
        action: onSelect({ kind: "behavior_action", actionType: item.key, label: item.label })
      }));
    return {
      kind: "group" as const,
      label: `P${idx + 1}`,
      children: actionNodes
    };
  }).filter((node) => Array.isArray(node.children) && node.children.length > 0);

  const sampleActionGroups: MenuNode[] = Array.from({ length: PLATFORM_CAPS.instrumentCount }, (_, instrumentSlot) => {
    const inst = ((state.runtimeConfig as any).instruments ?? [])[instrumentSlot];
    if (inst?.type !== "sampler") return null;
    return {
      kind: "group" as const,
      label: `I${instrumentSlot + 1}`,
      children: sampleSlots.map((_, sampleSlot) => ({
        kind: "action" as const,
        label: `Sample ${sampleSlot + 1}`,
        action: onSelect({
          kind: "menu_action",
          action: { type: "sample_assign_enter", instrumentSlot, sampleSlot },
          label: `Sample ${sampleSlot + 1}`
        })
      }))
    };
  }).filter(Boolean) as MenuNode[];

  return [
    { kind: "group", label: "Parts", children: partActions },
    { kind: "group", label: "Sample Assign", children: sampleActionGroups },
    {
      kind: "group",
      label: "FX Map",
      children: [
        {
          kind: "action",
          label: "Selected FX",
          action: onSelect({ kind: "menu_action", action: { type: "fx_assign_enter", config: selectedFxConfig }, label: "Selected FX" })
        }
      ]
    }
  ];
}

function l2PartGroup<TState>(
  state: PlatformState<TState>,
  deps: SenseMenuDeps<TState>,
  idx: number,
  instrumentSlotOptions: string[],
  partLabel: (state: PlatformState<TState>, idx: number) => string
): MenuNode {
  const prefix = `parts.${idx}`;
  return {
    kind: "group",
    label: partLabel(state, idx),
    children: [
      {
        kind: "group",
        label: "Scanning",
        children: [
          { kind: "enum", label: "Scan Mode", key: `${prefix}.l2.scanMode`, options: ["immediate", "scanning"] },
          { kind: "enum", label: "Scan Axis", key: `${prefix}.l2.scanAxis`, options: ["rows", "columns"], visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Scan Unit", key: `${prefix}.l2.scanUnit`, options: ["1/16", "1/8", "1/4", "1/2", "1/1"], visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Scan Direction", key: `${prefix}.l2.scanDirection`, options: ["forward", "reverse"], visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Sections", key: `${prefix}.l2.scanSections`, options: scanSectionOptions(), visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Action", key: `${prefix}.l2.mapping.scanned.action`, options: ["none", "note_on", "note_off"], visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Instrument", key: `${prefix}.l2.mapping.scanned.slot`, options: instrumentSlotOptions, visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Empty Action", key: `${prefix}.l2.mapping.scanned_empty.action`, options: ["none", "note_on", "note_off"], visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" },
          { kind: "enum", label: "Empty Instrument", key: `${prefix}.l2.mapping.scanned_empty.slot`, options: instrumentSlotOptions, visible: (c: any) => c.parts?.[idx]?.l2?.scanMode === "scanning" }
        ]
      },
      {
        kind: "group",
        label: "Events",
        children: [
          { kind: "bool", label: "Event Triggers", key: `${prefix}.l2.eventEnabled` },
          { kind: "enum", label: "Activate Action", key: `${prefix}.l2.mapping.activate.action`, options: ["none", "note_on", "note_off"], visible: (c: any) => c.parts?.[idx]?.l2?.eventEnabled },
          { kind: "enum", label: "Activate Instrument", key: `${prefix}.l2.mapping.activate.slot`, options: instrumentSlotOptions, visible: (c: any) => c.parts?.[idx]?.l2?.eventEnabled },
          { kind: "enum", label: "Stable Action", key: `${prefix}.l2.mapping.stable.action`, options: ["none", "note_on", "note_off"], visible: (c: any) => c.parts?.[idx]?.l2?.eventEnabled },
          { kind: "enum", label: "Stable Instrument", key: `${prefix}.l2.mapping.stable.slot`, options: instrumentSlotOptions, visible: (c: any) => c.parts?.[idx]?.l2?.eventEnabled },
          { kind: "enum", label: "Deactivate Action", key: `${prefix}.l2.mapping.deactivate.action`, options: ["none", "note_on", "note_off"], visible: (c: any) => c.parts?.[idx]?.l2?.eventEnabled },
          { kind: "enum", label: "Deactivate Instrument", key: `${prefix}.l2.mapping.deactivate.slot`, options: instrumentSlotOptions, visible: (c: any) => c.parts?.[idx]?.l2?.eventEnabled }
        ]
      },
      {
        kind: "group",
        label: "Trigger Prob.",
        children: [
          { kind: "enum", label: "Mode", key: `${prefix}.l2.triggerProbabilityMode`, options: ["zero", "custom", "full"] },
          { kind: "number", label: "Low Prob", key: `${prefix}.l2.triggerProbabilityLowPct`, min: 0, max: 100, step: 1 },
          { kind: "number", label: "High Prob", key: `${prefix}.l2.triggerProbabilityHighPct`, min: 0, max: 100, step: 1 },
          { kind: "action", label: "Map Probability Grid", action: { type: "trigger_probability_assign_enter", partIndex: idx } }
        ]
      },
      {
        kind: "group",
        label: "Mappings",
        excludeFromParamTree: true,
        children: [
          {
            kind: "group",
            label: "X Axis",
            children: [
              paramTargetGroup("Slot 1", (s) => (((s.runtimeConfig as any).parts?.[idx]?.paramMods?.x?.[0] ?? null) as AuxTurnBinding | null), (binding) => ({ type: "param_mod_set_target", partIndex: idx, axis: "x", slot: 0 as const, binding })),
              { kind: "bool", label: "Slot 1 Invert", key: `${prefix}.paramMods.x.0.invert` },
              paramTargetGroup("Slot 2", (s) => (((s.runtimeConfig as any).parts?.[idx]?.paramMods?.x?.[1] ?? null) as AuxTurnBinding | null), (binding) => ({ type: "param_mod_set_target", partIndex: idx, axis: "x", slot: 1 as const, binding })),
              { kind: "bool", label: "Slot 2 Invert", key: `${prefix}.paramMods.x.1.invert` }
            ]
          },
          {
            kind: "group",
            label: "Y Axis",
            children: [
              paramTargetGroup("Slot 1", (s) => (((s.runtimeConfig as any).parts?.[idx]?.paramMods?.y?.[0] ?? null) as AuxTurnBinding | null), (binding) => ({ type: "param_mod_set_target", partIndex: idx, axis: "y", slot: 0 as const, binding })),
              { kind: "bool", label: "Slot 1 Invert", key: `${prefix}.paramMods.y.0.invert` },
              paramTargetGroup("Slot 2", (s) => (((s.runtimeConfig as any).parts?.[idx]?.paramMods?.y?.[1] ?? null) as AuxTurnBinding | null), (binding) => ({ type: "param_mod_set_target", partIndex: idx, axis: "y", slot: 1 as const, binding })),
              { kind: "bool", label: "Slot 2 Invert", key: `${prefix}.paramMods.y.1.invert` }
            ]
          }
        ]
      },
      {
        kind: "group",
        label: "Note Mapping",
        children: [
          { kind: "number", label: "Lowest Note", key: `${prefix}.l2.pitch.lowestNote`, min: 0, max: 127, step: 1 },
          { kind: "number", label: "Highest Note", key: `${prefix}.l2.pitch.highestNote`, min: 0, max: 127, step: 1 },
          { kind: "number", label: "Starting Note", key: `${prefix}.l2.pitch.startingNote`, min: 0, max: 127, step: 1 },
          { kind: "enum", label: "Scale", key: `${prefix}.l2.pitch.scale`, options: ["chromatic", "major", "natural_minor", "dorian", "mixolydian", "major_pentatonic", "minor_pentatonic", "harmonic_minor"] },
          { kind: "enum", label: "Root", key: `${prefix}.l2.pitch.root`, options: ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] },
          { kind: "enum", label: "Out of Range", key: `${prefix}.l2.pitch.outOfRange`, options: ["clamp", "wrap"] }
        ]
      },
      deps.axisGroup("X Axis", `${prefix}.l2.x`, 1),
      deps.axisGroup("Y Axis", `${prefix}.l2.y`, 8)
    ]
  } satisfies MenuNode;
}

export function buildL2SenseGroup<TState>(
  state: PlatformState<TState>,
  deps: SenseMenuDeps<TState>,
  partCount: number,
  instrumentSlotOptions: string[],
  sampleSlots: string[],
  selectedFxConfig: { fxType: any; params: any; targetKey: string },
  partLabel: (state: PlatformState<TState>, idx: number) => string
): MenuNode {
  return {
    kind: "group",
    label: "L2: Sense",
    children: [
      {
        kind: "group",
        label: "Aux Mappings",
        excludeFromParamTree: true,
        children: Array.from({ length: 4 }, (_, i) => {
          const encoderId = `aux${i + 1}` as const;
          return {
            kind: "group",
            label: `Aux ${i + 1}`,
            children: [
              paramTargetGroup("Turn", (s) => ((s.system.auxBindings?.[encoderId]?.turn ?? null) as AuxTurnBinding | null), (binding) => ({ type: "aux_turn_set_target", encoderId, binding })),
              clickActionGroup(
                "Click",
                (s) => ((s.system.auxBindings?.[encoderId]?.press ?? null) as AuxPressBinding | null),
                (press) => ({ type: "aux_click_set_target", encoderId, press }),
                bindableClickActions(state, (binding) => ({ type: "aux_click_set_target", encoderId, press: binding }), selectedFxConfig, sampleSlots, deps.resolveBehavior)
              )
            ]
          } satisfies MenuNode;
        })
      },
      ...Array.from({ length: partCount }, (_, idx) => l2PartGroup(state, deps, idx, instrumentSlotOptions, partLabel))
    ]
  } satisfies MenuNode;
}
