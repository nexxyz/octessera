import type { DeviceInput, MusicalEvent, RuntimeSnapshot } from "./coreTypes";

export const RUNTIME_STATUS_STATES = ["idle", "running", "paused", "error"] as const;
export type RuntimeStatusState = (typeof RUNTIME_STATUS_STATES)[number];

export const RUNTIME_TRANSPORT_STATES = ["stopped", "playing", "paused"] as const;
export type RuntimeTransportState = (typeof RUNTIME_TRANSPORT_STATES)[number];

export const MIDI_REALTIME_MESSAGE_TYPES = ["clock", "start", "continue", "stop"] as const;
export type MidiRealtimeMessageType = (typeof MIDI_REALTIME_MESSAGE_TYPES)[number];

export const RUNTIME_MOMENTARY_FX_TYPES = ["none", "stutter", "freeze", "filter_sweep", "pitch_shift"] as const;
export type RuntimeMomentaryFxType = (typeof RUNTIME_MOMENTARY_FX_TYPES)[number];

export type RuntimeMomentaryFxTarget =
  | { type: "global" }
  | { type: "fx_bus"; index: number }
  | { type: "instrument"; index: number };

export type RuntimeAudioCommand =
  | { type: "set_master_volume"; volumePct: number }
  | { type: "set_instrument_mixer"; instrumentSlot: number; volumePct?: number; panPos?: number }
  | { type: "set_synth_param"; instrumentSlot: number; path: string; value: number }
  | { type: "set_sample_bank_param"; instrumentSlot: number; path: string; value: number }
  | { type: "set_fx_bus_slot"; busIndex: number; slotIndex: number; fxType: string; params: Record<string, unknown> }
  | { type: "set_global_fx_slot"; slotIndex: number; fxType: string; params: Record<string, unknown> }
  | { type: "momentary_fx_start"; id: string; fxType: RuntimeMomentaryFxType; params: Record<string, unknown>; target: RuntimeMomentaryFxTarget }
  | { type: "momentary_fx_update"; id: string; params: Record<string, unknown> }
  | { type: "momentary_fx_stop"; id: string }
  | { type: "sample_preview"; instrumentSlot: number; sampleSlot: number; path: string; velocity: number };

export type RuntimePlatformEffect =
  | { type: "store_list_presets" }
  | { type: "store_load_preset"; name: string }
  | { type: "store_save_preset"; name: string; payload: Record<string, unknown>; mode?: "immediate" | "deferred" }
  | { type: "store_delete_preset"; name: string }
  | { type: "store_load_default" }
  | { type: "store_save_default"; payload: Record<string, unknown>; mode?: "immediate" | "deferred" }
  | { type: "midi_list_outputs_request" }
  | { type: "midi_list_inputs_request" }
  | { type: "midi_select_output"; id: string | null }
  | { type: "midi_select_input"; id: string | null }
  | { type: "midi_panic" }
  | { type: "shutdown" }
  | { type: "sample_list_request"; instrumentSlot: number; sampleSlot: number; dir: string }
  | { type: "audio_command"; command: RuntimeAudioCommand };

export type RuntimeStoreResult =
  | { type: "list_presets_result"; names: string[] }
  | { type: "load_preset_result"; name: string; payload: Record<string, unknown> | null }
  | { type: "save_preset_result"; name: string; outcome: "created" | "overwritten" }
  | { type: "delete_preset_result"; name: string; ok: boolean }
  | { type: "load_default_result"; payload: Record<string, unknown> | null }
  | { type: "save_default_result"; ok: boolean; isAuto?: boolean }
  | { type: "store_error"; message: string }
  | { type: "midi_list_outputs_result"; outputs: Array<{ id: string; name: string }> }
  | { type: "midi_list_inputs_result"; inputs: Array<{ id: string; name: string }> }
  | { type: "midi_status"; ok: boolean; message?: string; selectedOutId?: string | null; selectedInId?: string | null }
  | { type: "sample_list_result"; instrumentSlot: number; sampleSlot: number; dir: string; entries: Array<{ name: string; path: string; isDir: boolean }> }
  | { type: "sample_list_error"; instrumentSlot: number; sampleSlot: number; dir: string; message: string }
  | { type: "sample_preview_error"; message: string };

export type RuntimeStatus = {
  state: RuntimeStatusState;
  transport: RuntimeTransportState;
  currentPpqnPulse: number;
  pendingResync: boolean;
  syncSource: "internal" | "external";
  message?: string;
};

export type RuntimeDeviceInputMessage = { type: "device_input"; input: DeviceInput };

export type RuntimeTransportPulseStepMessage = {
  type: "transport_pulse_step";
  pulses: number;
  source: "internal" | "external";
  atPpqnPulse?: number;
  requestSnapshot?: boolean;
};

export type RuntimeMidiRealtimeMessage =
  | { type: "midi_realtime"; message: "clock"; pulses: number }
  | { type: "midi_realtime"; message: Exclude<MidiRealtimeMessageType, "clock"> };

export type RuntimeResultMessage = { type: "runtime_result"; result: RuntimeStoreResult };

export type RuntimeHostMessage = RuntimeDeviceInputMessage | RuntimeTransportPulseStepMessage | RuntimeMidiRealtimeMessage | RuntimeResultMessage;

export type RuntimeSnapshotMessage = { type: "snapshot"; snapshot: RuntimeSnapshot };
export type RuntimePlatformEffectsMessage = { type: "platform_effects"; effects: RuntimePlatformEffect[] };
export type RuntimeMusicalEventsMessage = { type: "musical_events"; events: MusicalEvent[] };
export type RuntimeAudioCommandsMessage = { type: "audio_commands"; commands: RuntimeAudioCommand[] };
export type RuntimeUiPulse =
  | { type: "transport_flash"; flash: "measure" | "beat"; durationMs: number }
  | { type: "trigger_pulse"; durationMs: number };
export type RuntimeUiPulseMessage = { type: "ui_pulse"; pulse: RuntimeUiPulse };
export type RuntimeStatusMessage = { type: "runtime_status"; status: RuntimeStatus };

export type RuntimeRunnerMessage =
  | RuntimeSnapshotMessage
  | RuntimePlatformEffectsMessage
  | RuntimeMusicalEventsMessage
  | RuntimeAudioCommandsMessage
  | RuntimeUiPulseMessage
  | RuntimeStatusMessage;

export type RuntimeContractFixture = {
  id: string;
  description: string;
  hostMessages: RuntimeHostMessage[];
  runnerMessages: RuntimeRunnerMessage[];
};

export const SHARED_RUNTIME_CONTRACT_FIXTURES: RuntimeContractFixture[] = [
  {
    id: "device-grid-press-refreshes-snapshot",
    description: "A host forwards hardware-like grid input and receives an updated snapshot without any host-owned scheduling semantics.",
    hostMessages: [{ type: "device_input", input: { type: "grid_press", x: 2, y: 5 } }],
    runnerMessages: [
      {
        type: "snapshot",
        snapshot: {
          display: { page: "life", title: "L1: Life", lines: ["grid press"], editing: false },
          leds: { width: 8, height: 8, rgb: Array.from({ length: 64 * 3 }, () => 0) },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          activeBehavior: "life",
          gridInteraction: "paint"
        }
      },
      {
        type: "runtime_status",
        status: { state: "idle", transport: "stopped", currentPpqnPulse: 0, pendingResync: false, syncSource: "internal" }
      }
    ]
  },
  {
    id: "internal-pulse-step-emits-events",
    description: "The Rust runtime advances the core by explicit PPQN pulses and receives resolved musical events plus status.",
    hostMessages: [{ type: "transport_pulse_step", pulses: 6, source: "internal", atPpqnPulse: 96 }],
    runnerMessages: [
      {
        type: "musical_events",
        events: [{ type: "note_on", channel: 0, note: 60, velocity: 96, durationMs: 120 }]
      },
      {
        type: "platform_effects",
        effects: [{ type: "audio_command", command: { type: "sample_preview", instrumentSlot: 0, sampleSlot: 1, path: "samples/kick.wav", velocity: 110 } }]
      },
      {
        type: "audio_commands",
        commands: [{ type: "momentary_fx_start", id: "fx:2:5", fxType: "stutter", params: { depth: 0.6 }, target: { type: "global" } }]
      },
      {
        type: "runtime_status",
        status: { state: "running", transport: "playing", currentPpqnPulse: 102, pendingResync: false, syncSource: "internal" }
      }
    ]
  },
  {
    id: "external-midi-realtime-controls-transport",
    description: "External MIDI realtime messages stay explicit at the contract boundary instead of being inferred from desktop timers.",
    hostMessages: [
      { type: "midi_realtime", message: "start" },
      { type: "midi_realtime", message: "clock", pulses: 24 },
      { type: "midi_realtime", message: "stop" }
    ],
    runnerMessages: [
      {
        type: "runtime_status",
        status: { state: "running", transport: "playing", currentPpqnPulse: 24, pendingResync: false, syncSource: "external" }
      },
      {
        type: "platform_effects",
        effects: [{ type: "midi_panic" }]
      },
      {
        type: "runtime_status",
        status: { state: "paused", transport: "stopped", currentPpqnPulse: 24, pendingResync: false, syncSource: "external" }
      }
    ]
  },
  {
    id: "host-results-round-trip-platform-effects",
    description: "The host returns effect outcomes back into the runner so platform-core can update snapshots without owning storage or device I/O.",
    hostMessages: [
      { type: "runtime_result", result: { type: "list_presets_result", names: ["Factory", "Live Set"] } }
    ],
    runnerMessages: [
      {
        type: "snapshot",
        snapshot: {
          display: { page: "system", title: "System", lines: ["presets updated"], editing: false },
          leds: { width: 8, height: 8, rgb: Array.from({ length: 64 * 3 }, () => 0) },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          activeBehavior: "life",
          gridInteraction: "paint"
        }
      },
      {
        type: "runtime_status",
        status: { state: "idle", transport: "stopped", currentPpqnPulse: 0, pendingResync: false, syncSource: "internal" }
      }
    ]
  }
];
