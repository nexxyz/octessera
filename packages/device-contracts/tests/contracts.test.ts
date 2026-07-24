import test from "node:test";
import assert from "node:assert/strict";

import {
  GRID_DOMAIN,
  AUX_ENCODER_COUNT,
  DISPLAY_PALETTE,
  GRID_HEIGHT,
  GRID_WIDTH,
  MIDI_REALTIME_MESSAGE_TYPES,
  OLED_HEIGHT,
  OLED_WIDTH,
  PAN_POSITION_COUNT,
  PLATFORM_CAPS,
  RUNTIME_ERROR_CODES,
  RUNTIME_ERROR_DOMAINS,
  RUNTIME_OPERATIONS,
  RUNTIME_RECOVERIES,
  RUNTIME_STATUS_STATES,
  RUNTIME_TRANSPORT_STATES,
  SHARED_RUNTIME_CONTRACT_FIXTURES,
  type RuntimeAudioCommand,
  type RuntimeHostMessage,
  type RuntimePlatformEffect,
  type RuntimeRunnerMessage,
  type RuntimeStoreResult,
  type RuntimeErrorMetadata,
  type RuntimeSnapshot,
  type OledFrame
} from "../src/index";

type AssertNever<T extends never> = T;

const RUNTIME_AUDIO_COMMAND_FIXTURES = [
  { type: "set_audio_config", revision: 2, config: { sampleRate: 44100, blockFrames: 256 } },
  { type: "set_master_volume", volumePct: 82 },
  { type: "set_instrument_mixer", instrumentSlot: 1, volumePct: 74, panPos: 16 },
  { type: "set_fx_bus_mixer", busIndex: 2, panPos: 12, volumePct: 66 },
  { type: "set_synth_param", instrumentSlot: 3, path: "osc.cutoff", value: 0.7 },
  { type: "set_sample_bank_param", instrumentSlot: 4, path: "slot.0.level", value: 0.8 },
  { type: "set_fx_bus_slot", busIndex: 1, slotIndex: 0, fxType: "delay", params: { mix: 0.35 } },
  { type: "set_global_fx_slot", slotIndex: 1, fxType: "compressor", params: { threshold: -12 } },
  { type: "momentary_fx_start", id: "spark:0", fxType: "freeze", params: { amount: 1 }, target: { type: "fx_bus", index: 0 } },
  { type: "momentary_fx_update", id: "spark:0", params: { amount: 0.5 } },
  { type: "momentary_fx_stop", id: "spark:0" },
  { type: "sample_preview", instrumentSlot: 5, sampleSlot: 2, path: "kits/hat.wav", velocity: 96 }
] as const satisfies readonly RuntimeAudioCommand[];

const RUNTIME_PLATFORM_EFFECT_FIXTURES = [
  { type: "store_list_presets" },
  { type: "store_load_preset", name: "Factory" },
  { type: "store_save_preset", name: "Jam", payload: { bpm: 120 }, mode: "deferred" },
  { type: "store_delete_preset", name: "Old Jam" },
  { type: "store_load_default" },
  { type: "store_save_default", payload: { bpm: 96 }, mode: "immediate" },
  { type: "store_save_backup", payload: { name: "backup" } },
  { type: "store_save_recovery", payload: { dirty: true } },
  { type: "midi_list_outputs_request" },
  { type: "midi_list_inputs_request" },
  { type: "midi_select_output", id: "out-1" },
  { type: "midi_select_input", id: null },
  { type: "midi_panic" },
  { type: "reboot" },
  { type: "shutdown" },
  { type: "hardware_test" },
  { type: "update_check" },
  { type: "update_apply" },
  { type: "rollback" },
  { type: "system_info_request" },
  { type: "sample_list_request", instrumentSlot: 0, sampleSlot: 7, dir: "samples" },
  { type: "audio_command", command: RUNTIME_AUDIO_COMMAND_FIXTURES[0] }
] as const satisfies readonly RuntimePlatformEffect[];

const RUNTIME_STORE_RESULT_FIXTURES = [
  { type: "list_presets_result", names: ["Factory", "Live Set"] },
  { type: "load_preset_result", name: "Factory", payload: { layers: [] } },
  { type: "save_preset_result", name: "Live Set", outcome: "overwritten" },
  { type: "delete_preset_result", name: "Scratch", ok: true },
  { type: "load_default_result", payload: null },
  { type: "save_default_result", ok: true, isAuto: true },
  { type: "save_backup_result", ok: true },
  { type: "save_recovery_result", ok: false },
  { type: "store_error", message: "disk full" },
  { type: "runtime_failure", error: { domain: "audio", code: "audio_thread_failed", operation: "audio_thread", message: "thread stopped" } },
  { type: "identified", result: { type: "list_presets_result", names: ["Factory"] }, requestId: "platform-1", revision: 3 },
  { type: "operation_succeeded", operation: "store_load_default", requestId: "load-default-1", revision: 4 },
  { type: "midi_list_outputs_result", outputs: [{ id: "out-1", name: "Octessera MIDI" }] },
  { type: "midi_list_inputs_result", inputs: [{ id: "in-1", name: "Clock In" }] },
  { type: "midi_status", ok: false, message: "not connected", selectedOutId: null, selectedInId: "in-1" },
  { type: "sample_list_result", instrumentSlot: 2, sampleSlot: 3, dir: "samples", entries: [{ name: "Kicks", path: "samples/Kicks", isDir: true }] },
  { type: "sample_list_error", instrumentSlot: 2, sampleSlot: 3, dir: "samples", message: "permission denied" },
  { type: "sample_preview_error", message: "unsupported format" },
  { type: "device_update_status", ok: false, message: "opaque helper output" },
  {
    type: "system_info_result",
    info: {
      os: "linux",
      osVersion: "6.6",
      octesseraVersion: "0.7.0",
      primaryIp: "192.168.1.5",
      primaryMac: "aa:bb:cc:dd:ee:ff",
      hostname: "octessera",
      boardProfile: "raspberry-pi-zero-2w"
    }
  },
  { type: "system_info_error", error: { code: "unavailable", message: "not connected" } }
] as const satisfies readonly RuntimeStoreResult[];

const CANDIDATE_HEALTH_MARKER_FIXTURE = {
  schema_version: 1,
  pid: 4242,
  systemd_invocation_id: "inv-1",
  package_version: "0.7.0",
  board_profile: "raspberry-pi-zero-2w",
  ready_at_unix_ms: 1_700_000_000_123
} as const;

type AudioCommandFixtureTypes = (typeof RUNTIME_AUDIO_COMMAND_FIXTURES)[number]["type"];
type PlatformEffectFixtureTypes = (typeof RUNTIME_PLATFORM_EFFECT_FIXTURES)[number]["type"];
type StoreResultFixtureTypes = (typeof RUNTIME_STORE_RESULT_FIXTURES)[number]["type"];
type HostMessageFixtureTypes = (typeof SHARED_RUNTIME_CONTRACT_FIXTURES)[number]["hostMessages"][number]["type"];
type RunnerMessageFixtureTypes = (typeof SHARED_RUNTIME_CONTRACT_FIXTURES)[number]["runnerMessages"][number]["type"];
const EXHAUSTIVE_RUNTIME_PROTOCOL_FIXTURE_CHECK: AssertNever<
  | Exclude<RuntimeAudioCommand["type"], AudioCommandFixtureTypes>
  | Exclude<AudioCommandFixtureTypes, RuntimeAudioCommand["type"]>
  | Exclude<RuntimePlatformEffect["type"], PlatformEffectFixtureTypes>
  | Exclude<PlatformEffectFixtureTypes, RuntimePlatformEffect["type"]>
  | Exclude<RuntimeStoreResult["type"], StoreResultFixtureTypes>
  | Exclude<StoreResultFixtureTypes, RuntimeStoreResult["type"]>
  | Exclude<RuntimeHostMessage["type"], HostMessageFixtureTypes>
  | Exclude<HostMessageFixtureTypes, RuntimeHostMessage["type"]>
  | Exclude<RuntimeRunnerMessage["type"], RunnerMessageFixtureTypes>
  | Exclude<RunnerMessageFixtureTypes, RuntimeRunnerMessage["type"]>
> = undefined as never;

const assertRoundTripsThroughJson = <T extends { type: string }>(fixtures: readonly T[], expectedTypes: readonly string[]) => {
  const serialized = JSON.parse(JSON.stringify(fixtures)) as Array<{ type?: unknown }>;
  assert.deepEqual(
    serialized.map((fixture) => fixture.type).sort(),
    [...expectedTypes].sort()
  );
};

test("display palette matches the canonical instrument colors", () => {
  assert.deepEqual(DISPLAY_PALETTE.green, { label: "Green", hex: "#63D23F", rgb: [99, 210, 63], rgb565: 0x6687 });
  assert.deepEqual(DISPLAY_PALETTE.red, { label: "Red", hex: "#DD82CD", rgb: [221, 130, 205], rgb565: 0xdc19 });
  assert.deepEqual(DISPLAY_PALETTE.blue, { label: "Blue", hex: "#35CFF2", rgb: [53, 207, 242], rgb565: 0x367e });
  assert.deepEqual(DISPLAY_PALETTE.yellow, { label: "Yellow", hex: "#FFD447", rgb: [255, 212, 71], rgb565: 0xfea8 });
  assert.deepEqual(DISPLAY_PALETTE.gray, { label: "Gray", hex: "#C9CED6", rgb: [201, 206, 214], rgb565: 0xce7a });
  assert.deepEqual(DISPLAY_PALETTE.white, { label: "White", hex: "#FFFFFF", rgb: [255, 255, 255], rgb565: 0xffff });
  assert.deepEqual(DISPLAY_PALETTE.black, { label: "Black", hex: "#000000", rgb: [0, 0, 0], rgb565: 0x0000 });
});

test("grid constants are 8x8", () => {
  assert.equal(GRID_WIDTH, 8);
  assert.equal(GRID_HEIGHT, 8);
});

test("platform capabilities match the hardware profile", () => {
  assert.deepEqual(PLATFORM_CAPS, {
    gridWidth: 8,
    gridHeight: 8,
    layerCount: 8,
    instrumentCount: 8,
    sampleSlotCount: 8,
    audioSampleRate: 44100,
    audioBlockFrames: 256,
    synthSlotWorkers: 2,
    maxSynthVoices: 16,
    maxSampleVoices: 64,
    maxSynthVoicesPerSlot: 8,
    maxSampleVoicesPerSlot: 8,
    busFxWarningSlotCount: 12,
    busCount: 4,
    globalFxSlotCount: 2,
    auxEncoderCount: 3,
    sparksFxMaxConcurrent: 2,
    scanSectionCounts: [1, 2, 4, 8],
    panPositionCount: 33,
    oledWidth: 128,
    oledHeight: 128
  });
  assert.equal(AUX_ENCODER_COUNT, PLATFORM_CAPS.auxEncoderCount);
  assert.equal(PAN_POSITION_COUNT, PLATFORM_CAPS.panPositionCount);
  assert.equal(OLED_WIDTH, PLATFORM_CAPS.oledWidth);
  assert.equal(OLED_HEIGHT, PLATFORM_CAPS.oledHeight);
});

test("OLED framebuffer uses device contract rgb565be dimensions", () => {
  const frame: OledFrame = { width: OLED_WIDTH, height: OLED_HEIGHT, format: "rgb565be", pixels: new Uint8Array(OLED_WIDTH * OLED_HEIGHT * 2) };
  assert.equal(frame.pixels.length, OLED_WIDTH * OLED_HEIGHT * 2);
});

test("grid domain clamps/floors and preserves immutability", () => {
  const a = GRID_DOMAIN.toLogicalCell({ x: -1.4, y: 999.2 });
  assert.equal(a.x, 0);
  assert.equal(a.y, 0);

  const cells = new Array(GRID_WIDTH * GRID_HEIGHT).fill(false);
  const set = GRID_DOMAIN.set(cells, { x: 2, y: 3 }, true);
  assert.equal(cells[GRID_DOMAIN.indexOf({ x: 2, y: 3 })], false);
  assert.equal(set[GRID_DOMAIN.indexOf({ x: 2, y: 3 })], true);

  const toggled = GRID_DOMAIN.toggle(set, { x: 2, y: 3 });
  assert.equal(toggled[GRID_DOMAIN.indexOf({ x: 2, y: 3 })], false);
});

test("grid domain index conversion is consistent", () => {
  const idx = GRID_DOMAIN.toLogicalIndex({ x: 1, y: 2 });
  const cell = GRID_DOMAIN.cellOf(idx);
  assert.equal(cell.x, 1);
  assert.equal(cell.y, 5);
  const back = GRID_DOMAIN.toDisplayCell(cell);
  assert.equal(back.x, 1);
  assert.equal(back.y, 2);
});

test("grid display conversion preserves lower-left logical origin", () => {
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 0, y: 0 }), { x: 0, y: 7 });
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 7, y: 0 }), { x: 7, y: 7 });
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 0, y: 7 }), { x: 0, y: 0 });
  assert.deepEqual(GRID_DOMAIN.toLogicalCell({ x: 7, y: 7 }), { x: 7, y: 0 });
});

test("runtime contract fixtures cover each host and runner message class", () => {
  assert.equal(EXHAUSTIVE_RUNTIME_PROTOCOL_FIXTURE_CHECK, undefined);
  assert.equal(MIDI_REALTIME_MESSAGE_TYPES.join(","), "clock,start,continue,stop");
  assert.equal(RUNTIME_STATUS_STATES.join(","), "idle,running,paused,error");
  assert.equal(RUNTIME_TRANSPORT_STATES.join(","), "stopped,playing,paused");

  const hostTypes = new Set<string>();
  const runnerTypes = new Set<string>();

  for (const fixture of SHARED_RUNTIME_CONTRACT_FIXTURES) {
    assert.ok(fixture.id.length > 0);
    assert.ok(fixture.description.length > 0);
    assert.ok(fixture.hostMessages.length > 0);
    assert.ok(fixture.runnerMessages.length > 0);

    for (const message of fixture.hostMessages) {
      hostTypes.add(message.type);
      if (message.type === "transport_pulse_step") assert.ok(message.pulses > 0);
      if (message.type === "midi_realtime_clock") assert.ok(message.pulses > 0);
    }

    for (const message of fixture.runnerMessages) {
      runnerTypes.add(message.type);
      if (message.type === "runtime_status") assert.ok(message.status.currentPpqnPulse >= 0);
    }
  }

  assert.deepEqual([...hostTypes].sort(), ["device_input", "midi_realtime_clock", "midi_realtime_continue", "midi_realtime_start", "midi_realtime_stop", "runtime_result", "transport_pulse_step", "transport_stop"]);
  assert.deepEqual([...runnerTypes].sort(), ["audio_commands", "midi_events", "musical_events", "platform_effects", "runtime_status", "snapshot", "ui_pulse"]);
});

test("runtime protocol union fixtures serialize every drift-prone discriminant", () => {
  assertRoundTripsThroughJson(RUNTIME_AUDIO_COMMAND_FIXTURES, [
    "set_audio_config",
    "set_master_volume",
    "set_instrument_mixer",
    "set_fx_bus_mixer",
    "set_synth_param",
    "set_sample_bank_param",
    "set_fx_bus_slot",
    "set_global_fx_slot",
    "momentary_fx_start",
    "momentary_fx_update",
    "momentary_fx_stop",
    "sample_preview"
  ]);
  assertRoundTripsThroughJson(RUNTIME_PLATFORM_EFFECT_FIXTURES, [
    "store_list_presets",
    "store_load_preset",
    "store_save_preset",
    "store_delete_preset",
    "store_load_default",
    "store_save_default",
    "store_save_backup",
    "store_save_recovery",
    "midi_list_outputs_request",
    "midi_list_inputs_request",
    "midi_select_output",
    "midi_select_input",
    "midi_panic",
    "reboot",
    "shutdown",
    "hardware_test",
    "update_check",
    "update_apply",
    "rollback",
    "system_info_request",
    "sample_list_request",
    "audio_command"
  ]);
  assertRoundTripsThroughJson(RUNTIME_STORE_RESULT_FIXTURES, [
    "list_presets_result",
    "load_preset_result",
    "save_preset_result",
    "delete_preset_result",
    "load_default_result",
    "save_default_result",
    "save_backup_result",
    "save_recovery_result",
    "store_error",
    "runtime_failure",
    "identified",
    "operation_succeeded",
    "midi_list_outputs_result",
    "midi_list_inputs_result",
    "midi_status",
    "sample_list_result",
    "sample_list_error",
    "sample_preview_error",
    "device_update_status",
    "system_info_result",
    "system_info_error"
  ]);
});

test("candidate health marker fixture matches the guard identity contract", () => {
  assert.deepEqual(JSON.parse(JSON.stringify(CANDIDATE_HEALTH_MARKER_FIXTURE)), CANDIDATE_HEALTH_MARKER_FIXTURE);
  assert.equal(CANDIDATE_HEALTH_MARKER_FIXTURE.schema_version, 1);
  assert.ok(CANDIDATE_HEALTH_MARKER_FIXTURE.pid > 0);
  assert.ok(CANDIDATE_HEALTH_MARKER_FIXTURE.systemd_invocation_id.length > 0);
  assert.equal(CANDIDATE_HEALTH_MARKER_FIXTURE.board_profile, "raspberry-pi-zero-2w");
});

test("runtime error metadata serializes with stable typed identity and recovery", () => {
  const error = {
    domain: "storage",
    code: "operation_failed",
    operation: "store_load_default",
    recovery: "retain_last_good",
    message: "disk full"
  } as const satisfies RuntimeErrorMetadata;
  const snapshot = { runtimeError: error } as Pick<RuntimeSnapshot, "runtimeError">;

  assert.deepEqual(JSON.parse(JSON.stringify(snapshot)), snapshot);
  assert.deepEqual(RUNTIME_ERROR_DOMAINS, ["runtime", "storage", "midi", "sample", "audio", "serialization"]);
  assert.deepEqual(RUNTIME_ERROR_CODES, [
    "operation_failed",
    "unavailable",
    "invalid_payload",
    "not_found",
    "unsupported",
    "serialization_failed",
    "audio_thread_failed"
  ]);
  assert.ok(RUNTIME_OPERATIONS.includes(error.operation));
  assert.ok(RUNTIME_OPERATIONS.includes("device_update"));
  assert.ok(RUNTIME_RECOVERIES.includes(error.recovery));
});
