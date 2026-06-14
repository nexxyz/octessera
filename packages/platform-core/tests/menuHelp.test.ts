import test from "node:test";
import assert from "node:assert/strict";
import { resolveMenuHelpEntry } from "../src/menuHelp";
import { menuHelpTargetFromNode } from "../src/menuHelpTargets";

test("menu help resolves specific submenu groups", () => {
  const cases = [
    { path: "Menu > L3: Voice > Instruments", key: "", kind: "group", label: "Instruments", id: "inst_group" },
    { path: "Menu > L3: Voice > FX Buses", key: "", kind: "group", label: "FX Buses", id: "fx_buses_group" },
    { path: "Menu > L3: Voice > Global FX", key: "", kind: "group", label: "Global FX", id: "global_fx_group" },
    { path: "Menu > L2: Sense > Aux Mappings", key: "", kind: "group", label: "Aux Mappings", id: "sense_aux_mappings_group" },
    { path: "Menu > L4: Dance > Mode Grid", key: "", kind: "group", label: "Mode Grid", id: "touch_gate_mode_grid" },
    { path: "Menu > L2: Sense > P1: life", key: "", kind: "group", label: "P1: life", id: "sense_part_group" }
  ] as const;

  for (const target of cases) {
    const entry = resolveMenuHelpEntry(target);
    assert.ok(entry, `expected help entry for ${target.path}`);
    assert.equal(entry!.id, target.id);
  }
});

test("menu help resolves dynamic instrument and bus paths", () => {
  const busEntry = resolveMenuHelpEntry(
    menuHelpTargetFromNode("Menu > L3: Voice > FX Buses", { kind: "group", label: "B1: fx", children: [] })
  );
  assert.ok(busEntry);
  assert.equal(busEntry!.id, "fx_bus_wild");

  const instrumentEntry = resolveMenuHelpEntry(
    menuHelpTargetFromNode("Menu > L3: Voice > Instruments > I1: synth", { kind: "group", label: "Mixer", children: [] })
  );
  assert.ok(instrumentEntry);
  assert.equal(instrumentEntry!.id, "inst_mixer_group");
});

test("menu help prefers exact key over wildcard matches", () => {
  const target = {
    path: "Menu > System > MIDI > Sync & Clock > Respond Start/Stop",
    key: "key:midi.respondToStartStop",
    kind: "bool",
    label: "Respond Start/Stop"
  };
  const entry = resolveMenuHelpEntry(target);
  assert.ok(entry);
  assert.equal(entry!.id, "midi_respond_start_stop");
});

test("menu help resolves dynamic MIDI output by wildcard key", () => {
  const target = {
    path: "Menu > System > MIDI > MIDI Out > Out Port",
    key: "action:midi_select_output:out-1",
    kind: "action",
    label: "Out Port"
  };
  const entry = resolveMenuHelpEntry(target);
  assert.ok(entry);
  assert.equal(entry!.id, "midi_out_dynamic");
});
