# System FAT software coverage

This is part of system FAT, but it is primarily automated. It establishes that
the main musical and algorithmic functions work as a product contract; the board
steps in the [FAT orchestrator](fat-quick-run.md) and [board end-to-end
procedures](fat-board-end-to-end.md) establish that the exact image and hardware
path carries those functions into the instrument.

Run these from the exact release checkout. The orchestrator creates
`$Evidence/software`; save each result there:

```powershell
cargo test -p platform-core | Tee-Object -FilePath (Join-Path $Evidence "software\01-platform-core.txt")
cargo test -p playback-runtime | Tee-Object -FilePath (Join-Path $Evidence "software\02-playback-runtime.txt")
cargo test -p realtime-engine | Tee-Object -FilePath (Join-Path $Evidence "software\03-realtime-engine.txt")
corepack pnpm run typecheck | Tee-Object -FilePath (Join-Path $Evidence "software\04-typecheck.txt")
```

| Product area | Automated evidence | Establishes | Does not establish |
|---|---|---|---|
| Cellular, field, geometry, motion, growth, and music behaviors | `platform-core` behavior/catalog/configuration tests | Behaviors construct, validate, execute, serialize, and apply parameters | Musical taste or physical control feel |
| Layers, worlds, grid transforms, interpretation, mapping | `platform-core` and `playback-runtime` tests | Coordinate semantics, layer ownership, snapshots, and native execution | Physical grid orientation or LED appearance |
| Pulses, Sparks, sequencer, looper, trigger gates, probability, lane curves | `playback-runtime` native-runner tests | Menu edits, payloads, timing, persistence, overlays, and runtime behavior | OLED readability or live musical usefulness |
| Synth, sampler, MIDI, sample preview, routing, FX, ducking | `realtime-engine`, playback-runtime, and Pi audio tests | Rendering, routing, commands, sample preparation, FX, and failure semantics | Actual DAC/USB/host sound until board steps |
| Patches, presets, samples, preferences, backup/restore | Playback/Pi data tests | Names, migrations, ownership, media limits, atomic restore, and failure safety | Exact-board flash/restore workflow |
| Menus, help, protocol, desktop bridge | Playback tests, enum-help coverage, and TypeScript checks | Native ownership, wire shape, enum coverage, and desktop compilation | Hardware qualification; desktop remains a simulator |
| Image, updater, profile, and recovery contracts | Image/updater/release tests and constructor proofs | Source/image contracts, checksums, profiles, staging, and rollback logic | Physical boot, power, USB electrical behavior, and OLED appearance |

## Integrated manual spot coverage

The default patch sound and one small control edit on each board provide the
integrated spot check for the matrix: native startup, menu/input handoff, patch
loading, one behavior execution, one instrument/audio path, and one physical
output. Do not repeat those checks for every behavior or instrument.

Use remaining manual time only for a distinct representative path, such as:

- sampler versus synth;
- MIDI versus internal audio;
- a non-default behavior;
- a second layer or Pulses/Sparks workflow.

Mark software `FAIL` only when an automated command fails or the integrated spot
check contradicts it. Mark subjective musical quality or unsupported physical
evidence `NOT RUN — operator observation required`, never automated pass. Reserve
`operator_required` for the harness JSON report.
