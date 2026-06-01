import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import readline from "node:readline";

import { getBehavior } from "@cellsymphony/behavior-api";
import { createInitialState, emergencyBrake, routeInput, tick } from "@cellsymphony/platform-core";
import type { DeviceInput } from "@cellsymphony/device-contracts";
import type { MusicalEvent } from "@cellsymphony/musical-events";

type SidecarEvent =
  | { type: "ports"; inputs: { name: string }[]; outputs: { name: string }[] }
  | { type: "selected"; input: string | null; output: string | null }
  | { type: "recv"; bytes: number[] }
  | { type: "status"; ok: boolean; message: string };

function normalizedArgv(): string[] {
  return process.argv.filter((a) => a !== "--");
}

function arg(name: string, fallback: string | null = null) {
  const argv = normalizedArgv();
  const idx = argv.indexOf(name);
  if (idx === -1) return fallback;
  return argv[idx + 1] ?? fallback;
}

const outNeedle = arg("--out", "loopMIDI")!;
const inNeedle = arg("--in", "loopMIDI")!;
const seconds = Number(arg("--seconds", "3"));
const durationMs = Math.max(250, Math.floor(seconds * 1000));

const here = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(here, "..", "..", "..");
const sidecarManifest = path.join(repoRoot, "tools", "midi-io-sidecar", "Cargo.toml");

const sidecar = spawn("cargo", ["run", "--quiet", "--manifest-path", sidecarManifest], {
  cwd: repoRoot,
  stdio: ["pipe", "pipe", "inherit"]
});

const rl = readline.createInterface({ input: sidecar.stdout });

const received: number[][] = [];
let ready = false;

function sendCmd(cmd: any) {
  sidecar.stdin.write(JSON.stringify(cmd) + "\n");
}

rl.on("line", (line) => {
  const trimmed = line.trim();
  if (!trimmed) return;
  const evt = JSON.parse(trimmed) as SidecarEvent;
  if (evt.type === "recv") received.push(evt.bytes);
  if (!ready && (evt.type === "selected" || evt.type === "ports")) {
    ready = true;
  }
});

async function sleep(ms: number) {
  await new Promise((r) => setTimeout(r, ms));
}

function scheduleNoteOff(queue: { due: number; bytes: Uint8Array }[], due: number, bytes: Uint8Array) {
  queue.push({ due, bytes });
}

function bytes(...b: number[]) {
  return new Uint8Array(b);
}

async function main() {
  sendCmd({ cmd: "select_out", name_contains: outNeedle });
  sendCmd({ cmd: "select_in", name_contains: inNeedle });

  await sleep(200);

  let state = createInitialState(getBehavior("sequencer")!);
  state.system.oledMode = "normal";
  state.runtimeConfig.activeBehavior = "life";

  // Enable MIDI internal + clock out.
  state.runtimeConfig.midi.enabled = true;
  state.runtimeConfig.midi.clockOutEnabled = true;
  state.runtimeConfig.midi.syncMode = "internal";
  state.runtimeConfig.midi.outId = "loopback";

  // Make note generation deterministic/fast.
  state.runtimeConfig.algorithmStepUnit = "1/16";

  // Force stopLatched then play so we emit Start.
  state.system.stopLatched = true;
  let playingPrev = state.transport.playing;
  let stopPrev = state.system.stopLatched;
  let prevPulse = state.transport.ppqnPulse;

  {
    // Seed the grid with a simple pattern so conway generates events.
    const seeds: DeviceInput[] = [
      // blinker
      { type: "grid_press", x: 3, y: 3 },
      { type: "grid_press", x: 4, y: 3 },
      { type: "grid_press", x: 5, y: 3 }
    ];
    for (const s of seeds) {
      const r0 = routeInput(state, s, getBehavior(state.activeBehavior)!);
      state = r0.state;
    }

    const r = routeInput(state, { type: "button_s" } as DeviceInput, getBehavior(state.activeBehavior)!);
    state = r.state;
  }

  const startMs = Date.now();
  const midiQueue: { due: number; bytes: Uint8Array }[] = [];

  const stepMs = 8;
  while (Date.now() - startMs < durationMs) {
    const now = Date.now();
    const r = tick(state, behavior, stepMs / 1000);
    state = r.state;

    // transport bytes
    if (playingPrev !== state.transport.playing) {
      if (!playingPrev && state.transport.playing) {
        // stop->play uses Start, pause->play uses Continue
        sendCmd({ cmd: "send", bytes: Array.from(bytes(stopPrev ? 0xfa : 0xfb)) });
      } else if (playingPrev && !state.transport.playing) {
        sendCmd({ cmd: "send", bytes: Array.from(bytes(0xfc)) });
      }
      playingPrev = state.transport.playing;
      stopPrev = state.system.stopLatched;
    }

    // clock
    if (state.transport.playing) {
      for (let p = prevPulse + 1; p <= state.transport.ppqnPulse; p += 1) {
        sendCmd({ cmd: "send", bytes: Array.from(bytes(0xf8)) });
      }
    }
    prevPulse = state.transport.ppqnPulse;

    // musical events
    for (const e of r.events) {
      if (e.type === "note_on") {
        const ch = Math.max(0, Math.min(15, e.channel | 0));
        const note = Math.max(0, Math.min(127, e.note | 0));
        const vel = Math.max(1, Math.min(127, e.velocity | 0));
        sendCmd({ cmd: "send", bytes: Array.from(bytes(0x90 | ch, note, vel)) });
        const len = Math.max(1, Math.min(10_000, e.durationMs ?? 120));
        scheduleNoteOff(midiQueue, now + len, bytes(0x80 | ch, note, 0));
      }
      if (e.type === "cc") {
        const ch = Math.max(0, Math.min(15, e.channel | 0));
        sendCmd({ cmd: "send", bytes: Array.from(bytes(0xb0 | ch, e.controller | 0, e.value | 0)) });
      }
    }

    // flush note-offs
    for (let i = midiQueue.length - 1; i >= 0; i -= 1) {
      if (midiQueue[i]!.due <= now) {
        const m = midiQueue.splice(i, 1)[0]!;
        sendCmd({ cmd: "send", bytes: Array.from(m.bytes) });
      }
    }

    await sleep(stepMs);
  }

  // Stop
  {
    const r = emergencyBrake(state);
    state = r.state;
    void r;
  }
  sendCmd({ cmd: "send", bytes: [0xfc] });

  await sleep(250);
  sendCmd({ cmd: "quit" });

  // Assert we saw some clock bytes
  const flat = received.flat();
  const sawStart = flat.includes(0xfa);
  const sawClock = flat.includes(0xf8);
  const sawNote = flat.includes(0x90);
  if (!sawStart || !sawClock || !sawNote) {
    console.error({ sawStart, sawClock, sawNote, receivedCount: received.length });
    process.exit(1);
  }
  console.log("midi headless ok", { sawStart, sawClock, sawNote, receivedCount: received.length });
  process.exit(0);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
