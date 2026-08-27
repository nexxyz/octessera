import test from "node:test";
import assert from "node:assert/strict";

import {
  createOledFrameRevision,
  isPositiveOledFrameRevision,
  isRuntimeSnapshotMessage,
  MIDI_REALTIME_MESSAGE_TYPES,
  RUNTIME_STATUS_STATES,
  RUNTIME_TRANSPORT_STATES,
  SHARED_RUNTIME_CONTRACT_FIXTURES,
  type RuntimeAudioCommand,
  type RuntimeHostMessage,
  type RuntimePlatformEffect,
  type RuntimeRunnerMessage,
  type RuntimeStoreResult,
} from "../src/index";
import {
  RUNTIME_AUDIO_COMMAND_FIXTURES,
  RUNTIME_PLATFORM_EFFECT_FIXTURES,
  RUNTIME_SETUP_PORTAL_STATUS_FIXTURES,
  RUNTIME_STORE_RESULT_FIXTURES,
  RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES,
} from "./runtimeProtocolFixtures";

type AssertNever<T extends never> = T;

type AudioCommandFixtureTypes =
  (typeof RUNTIME_AUDIO_COMMAND_FIXTURES)[number]["type"];
type PlatformEffectFixtureTypes =
  (typeof RUNTIME_PLATFORM_EFFECT_FIXTURES)[number]["type"];
type StoreResultFixtureTypes =
  (typeof RUNTIME_STORE_RESULT_FIXTURES)[number]["type"];
type HostMessageFixtureTypes =
  (typeof SHARED_RUNTIME_CONTRACT_FIXTURES)[number]["hostMessages"][number]["type"];
type RunnerMessageFixtureTypes =
  (typeof SHARED_RUNTIME_CONTRACT_FIXTURES)[number]["runnerMessages"][number]["type"];
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

const assertRoundTripsThroughJson = <T extends { type: string }>(
  fixtures: readonly T[],
  expectedTypes: readonly string[],
) => {
  const serialized = JSON.parse(JSON.stringify(fixtures)) as Array<{
    type?: unknown;
  }>;
  assert.deepEqual(
    serialized.map((fixture) => fixture.type).sort(),
    [...expectedTypes].sort(),
  );
};

test("OLED frame revisions are positive and snapshot wire messages reject missing or non-positive revisions", () => {
  assert.equal(isPositiveOledFrameRevision(1), true);
  assert.equal(isPositiveOledFrameRevision(0), false);
  assert.equal(isPositiveOledFrameRevision(-1), false);
  assert.equal(isPositiveOledFrameRevision(1.5), false);
  assert.throws(() => createOledFrameRevision(0), RangeError);

  const snapshot = SHARED_RUNTIME_CONTRACT_FIXTURES[0]!.runnerMessages.find(
    (message) => message.type === "snapshot",
  );
  assert.ok(snapshot && isRuntimeSnapshotMessage(snapshot));
  assert.equal(
    isRuntimeSnapshotMessage({
      ...snapshot,
      snapshot: { ...snapshot.snapshot, oledFrameRevision: 0 },
    }),
    false,
  );
  assert.equal(
    isRuntimeSnapshotMessage({
      ...snapshot,
      snapshot: { ...snapshot.snapshot, oledFrameRevision: undefined },
    }),
    false,
  );
});

test("runtime contract fixtures cover each host and runner message class", () => {
  assert.equal(EXHAUSTIVE_RUNTIME_PROTOCOL_FIXTURE_CHECK, undefined);
  assert.equal(
    MIDI_REALTIME_MESSAGE_TYPES.join(","),
    "clock,start,continue,stop",
  );
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
      if (message.type === "transport_pulse_step")
        assert.ok(message.pulses > 0);
      if (message.type === "midi_realtime_clock") assert.ok(message.pulses > 0);
    }

    for (const message of fixture.runnerMessages) {
      runnerTypes.add(message.type);
      if (message.type === "runtime_status")
        assert.ok(message.status.currentPpqnPulse >= 0);
    }
  }

  assert.deepEqual([...hostTypes].sort(), [
    "device_input",
    "midi_realtime_clock",
    "midi_realtime_continue",
    "midi_realtime_start",
    "midi_realtime_stop",
    "runtime_result",
    "transport_pulse_step",
    "transport_stop",
  ]);
  assert.deepEqual([...runnerTypes].sort(), [
    "audio_commands",
    "midi_events",
    "musical_events",
    "oled_frame",
    "platform_effects",
    "runtime_status",
    "snapshot",
  ]);
  for (const fixture of SHARED_RUNTIME_CONTRACT_FIXTURES) {
    const oledIndex = fixture.runnerMessages.findIndex(
      (message) => message.type === "oled_frame",
    );
    const snapshotIndex = fixture.runnerMessages.findIndex(
      (message) => message.type === "snapshot",
    );
    const statusIndex = fixture.runnerMessages.findIndex(
      (message) => message.type === "runtime_status",
    );
    if (oledIndex >= 0) {
      assert.ok(snapshotIndex >= 0);
      assert.ok(statusIndex >= 0);
      assert.ok(oledIndex < snapshotIndex);
      assert.ok(snapshotIndex < statusIndex);
    }
  }
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
    "sample_preview",
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
    "setup_portal_open",
    "user_data_transfer_open",
    "user_data_transfer_close",
    "sample_list_request",
    "audio_command",
  ]);
  assertRoundTripsThroughJson(
    RUNTIME_SETUP_PORTAL_STATUS_FIXTURES,
    new Array(10).fill("setup_portal_status"),
  );
  assertRoundTripsThroughJson(
    RUNTIME_USER_DATA_TRANSFER_STATUS_FIXTURES,
    new Array(3).fill("user_data_transfer_status"),
  );
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
    "system_info_error",
    "setup_portal_status",
    "identified",
    "user_data_transfer_status",
  ]);
});
