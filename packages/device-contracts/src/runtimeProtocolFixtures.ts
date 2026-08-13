import { createOledFrameRevision } from "./runtimeProtocol";
import type { RuntimeContractFixture } from "./runtimeProtocol";

const EMPTY_OLED_FRAME_BASE64 = "A".repeat(43691) + "=";

export const SHARED_RUNTIME_CONTRACT_FIXTURES: RuntimeContractFixture[] = [
  {
    id: "device-grid-press-refreshes-snapshot",
    description:
      "A host forwards hardware-like grid input and receives an updated snapshot without any host-owned scheduling semantics.",
    hostMessages: [
      { type: "device_input", input: { type: "grid_press", x: 2, y: 5 } },
    ],
    runnerMessages: [
      {
        type: "oled_frame",
        revision: createOledFrameRevision(1),
        width: 128,
        height: 128,
        format: "rgb565be",
        pixelsBase64: EMPTY_OLED_FRAME_BASE64,
      },
      {
        type: "snapshot",
        snapshot: {
          display: {
            page: "life",
            title: "Build",
            lines: ["grid press"],
            editing: false,
          },
          leds: {
            width: 8,
            height: 8,
            rgb: Array.from({ length: 64 * 3 }, () => 0),
            active: Array.from({ length: 64 }, () => false),
          },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          activeBehavior: "life",
          gridInteraction: "paint",
          neoKeyLeds: {
            back: [221, 130, 205],
            space: [221, 130, 205],
            shift: [67, 68, 71],
            fn: [67, 68, 71],
          },
          eventDotOn: false,
          transportIcon: "stop",
          transportFlash: "none",
          oledFrameRevision: createOledFrameRevision(1),
        },
      },
      {
        type: "runtime_status",
        status: {
          state: "idle",
          transport: "stopped",
          currentPpqnPulse: 0,
          pendingResync: false,
          syncSource: "internal",
        },
      },
    ],
  },
  {
    id: "internal-pulse-step-emits-events",
    description:
      "The Rust runtime advances the core by explicit PPQN pulses and receives resolved musical events plus status.",
    hostMessages: [
      {
        type: "transport_pulse_step",
        pulses: 6,
        source: "internal",
        atPpqnPulse: 96,
      },
    ],
    runnerMessages: [
      {
        type: "musical_events",
        events: [
          {
            type: "note_on",
            channel: 0,
            note: 60,
            velocity: 96,
            durationMs: 120,
          },
        ],
      },
      {
        type: "midi_events",
        events: [
          {
            type: "note_on",
            channel: 1,
            note: 64,
            velocity: 90,
            durationMs: 120,
          },
        ],
      },
      {
        type: "platform_effects",
        effects: [
          {
            type: "audio_command",
            command: {
              type: "sample_preview",
              instrumentSlot: 0,
              sampleSlot: 1,
              path: "samples/kick.wav",
              velocity: 110,
            },
          },
        ],
      },
      {
        type: "audio_commands",
        commands: [
          {
            type: "momentary_fx_start",
            id: "fx:2:5",
            fxType: "stutter",
            params: { depth: 0.6 },
            target: { type: "global" },
          },
        ],
      },
      {
        type: "runtime_status",
        status: {
          state: "running",
          transport: "playing",
          currentPpqnPulse: 102,
          pendingResync: false,
          syncSource: "internal",
        },
      },
    ],
  },
  {
    id: "external-midi-realtime-controls-transport",
    description:
      "External MIDI realtime messages stay explicit at the contract boundary instead of being inferred from desktop timers.",
    hostMessages: [
      { type: "midi_realtime_start" },
      { type: "midi_realtime_clock", pulses: 24 },
      { type: "midi_realtime_continue" },
      { type: "midi_realtime_stop" },
      { type: "transport_stop" },
    ],
    runnerMessages: [
      {
        type: "runtime_status",
        status: {
          state: "running",
          transport: "playing",
          currentPpqnPulse: 24,
          pendingResync: false,
          syncSource: "external",
        },
      },
      {
        type: "platform_effects",
        effects: [{ type: "midi_panic" }],
      },
      {
        type: "runtime_status",
        status: {
          state: "paused",
          transport: "stopped",
          currentPpqnPulse: 24,
          pendingResync: false,
          syncSource: "external",
        },
      },
    ],
  },
  {
    id: "host-results-round-trip-platform-effects",
    description:
      "The host returns effect outcomes back into the runner so platform-core can update snapshots without owning storage or device I/O.",
    hostMessages: [
      {
        type: "runtime_result",
        result: { type: "list_presets_result", names: ["Factory", "Live Set"] },
      },
    ],
    runnerMessages: [
      {
        type: "oled_frame",
        revision: createOledFrameRevision(1),
        width: 128,
        height: 128,
        format: "rgb565be",
        pixelsBase64: EMPTY_OLED_FRAME_BASE64,
      },
      {
        type: "snapshot",
        snapshot: {
          display: {
            page: "system",
            title: "System",
            lines: ["presets updated"],
            editing: false,
          },
          leds: {
            width: 8,
            height: 8,
            rgb: Array.from({ length: 64 * 3 }, () => 0),
            active: Array.from({ length: 64 }, () => false),
          },
          transport: { playing: false, bpm: 120, tick: 0, ppqnPulse: 0 },
          activeBehavior: "life",
          gridInteraction: "paint",
          neoKeyLeds: {
            back: [221, 130, 205],
            space: [221, 130, 205],
            shift: [67, 68, 71],
            fn: [67, 68, 71],
          },
          eventDotOn: false,
          transportIcon: "stop",
          transportFlash: "none",
          oledFrameRevision: createOledFrameRevision(1),
        },
      },
      {
        type: "runtime_status",
        status: {
          state: "idle",
          transport: "stopped",
          currentPpqnPulse: 0,
          pendingResync: false,
          syncSource: "internal",
        },
      },
    ],
  },
  {
    id: "runtime-snapshot-and-host-actions",
    description:
      "Native snapshots carry transient presentation while platform effects stay explicit host-owned actions.",
    hostMessages: [
      { type: "device_input", input: { type: "button_s", pressed: true } },
    ],
    runnerMessages: [
      {
        type: "snapshot",
        snapshot: {
          display: { page: "life", title: "Play", lines: [], editing: false },
          leds: {
            width: 8,
            height: 8,
            rgb: Array.from({ length: 64 * 3 }, () => 0),
            active: Array.from({ length: 64 }, () => false),
          },
          transport: { playing: true, bpm: 120, tick: 0, ppqnPulse: 24 },
          activeBehavior: "life",
          gridInteraction: "paint",
          neoKeyLeds: {
            back: [221, 130, 205],
            space: [255, 212, 71],
            shift: [201, 206, 214],
            fn: [201, 206, 214],
          },
          eventDotOn: true,
          transportIcon: "play",
          transportFlash: "beat",
          oledFrameRevision: createOledFrameRevision(1),
        },
      },
      {
        type: "platform_effects",
        effects: [
          { type: "hardware_test" },
          { type: "update_check" },
          { type: "update_apply" },
          { type: "rollback" },
        ],
      },
    ],
  },
];
