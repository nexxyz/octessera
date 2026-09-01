# Pi development and profiling

This page covers host-only Pi builds and board-specific profiling. It does not
replace the ordered Orange bring-up procedure or the deployment runbook.

Canonical board IDs are `raspberry-pi-zero-2w` and `orange-pi-zero-2w`; their
images, pinouts, port roles, and deployment adapters are not interchangeable.
See [`../board-profiles.md`](../board-profiles.md) for feature owners and the
deprecated compatibility aliases.

## Builds without hardware

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w

# Deprecated compatibility alias; accepted for existing Cargo commands.
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features pi-zero
```

The Orange cross-builder is WSL Docker-only. It never contacts or deploys to a
board and writes checked artifacts under `target/orange-pi-cross/`:

```powershell
./tools/orange-pi/build-orange-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-orange-cross.ps1 -Binary octessera-pi -Profile release
./tools/orange-pi/test-build-orange-cross.ps1
```

Cargo and rustup caches use persistent named Docker volumes; `-DryRun` prints
the command without starting Docker. The helper accepts the two diagnostic
smoke binaries and a local Orange `octessera-pi` development binary. It does
not produce a production image or its `production-runtime` bundle, and no
artifact is run against the board by this helper.

## Orange live audio benchmark tooling

The fixed-target Orange capability runner is host tooling, not another SSH
transport. Single-cell mode requires a reviewed artifact and metadata sidecar,
explicit interruption consent, and one approved scenario/configuration. It
checks readiness identity, exact DAC ALSA `buffer_size`/`period_size`, release
identity, schema-2 callback geometry, thermal/memory safety, and restoration.

Preview the deterministic 29-cell order without transport or board access:

```powershell
./tools/orange-pi/run-orange-live-audio-matrix.ps1 -PrintOnly
```

The order is A (11 cells), the selected A 120-second repeat, B (11 cells), then
C0, C2, and C3 (two cells each). Callback batches are variable positive counts
no larger than the requested ALSA buffer; render ratios use each actual callback
size, while spacing lateness uses the fixed ALSA period. Active execution
requires `-AllowMatrixServiceInterruption` and per-cell consent. This is
host-only Phase 2 validation; do not cross-build, deploy, or run it as a normal
contributor check.

## Pi UI and audio profiling

Pi UI/render profiling is quiet by default. Enable summaries with either form:

```bash
OCTESSERA_PI_UI_PROFILE=1 octessera-pi
octessera-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns,
snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency,
and DSP budget questions. PC/runtime-only probes are plausibility checks, not
hardware audio timing proof. `tools/pi/run-pi-timing-probes.ps1` is
Raspberry-only; never point it at Orange.

```powershell
# Safe default: runtime-only, does not stop the service or open live audio.
./tools/pi/run-pi-timing-probes.ps1 -Mode RuntimeOnly -Durations 15s -Scenarios idle,pulses-stress

# Optional live-audio probe.
./tools/pi/run-pi-timing-probes.ps1 -Mode Live -Durations 10m -Scenarios idle

# Optional audio-source drain latency probe.
./tools/pi/run-pi-timing-probes.ps1 -Mode AudioDrain -Durations 10m

# Focused FX budget profile.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits

# Explicit high-headroom frame comparison settings.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits -AudioRenderQuantumFrames 256
```

The wrapper stops `octessera.service` for live/audio/DSP modes and restarts it
afterward. Runtime-only leaves it running. Use `-PrintOnly` to inspect the
remote command first.

Orange command generation and offline comparisons use:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode PassiveBaseline -PrintOnly
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64 -AllowServiceInterruption -PrintOnly
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp256 -AllowServiceInterruption -PrintOnly
```

DSP modes require explicit acknowledgement even with `-PrintOnly`; non-print
runs stop the production service:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64 -AllowServiceInterruption
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp256 -AllowServiceInterruption
```

The Orange offline DSP profile finds computational knees; it is not live-xrun
proof. The current CPAL/ALSA path cannot count internally recovered `EPIPE`
events, so a clean offline report is not zero-xrun evidence and cannot change
capabilities. Inspect p99/p99.9 and outlier counts, not only p95.

After live probes, inspect recent logs:

```powershell
./tools/pi/with-pi-ssh.ps1 ssh pi@192.168.0.218 "journalctl -u octessera.service --since '10 minutes ago' --no-pager | grep -E 'audio callback RT promotion not qualified|audio stream error|underrun|POLLERR' || true"
```

Offer a live probe when the report is subjective or audio-path-specific; do not
run long live probes for unrelated changes.

## Cross-board performance baseline

The baseline is deliberately two-layer evidence, not a normalized score. The
native profile layer compares the same 44.1 kHz scenarios with a two-second
warmup, 4096 measured observations, and three fresh processes per cell. It
contains the common reference, Orange-effective-default, and block
cohorts in [`tools/performance/cross-board-baseline.json`](../../tools/performance/cross-board-baseline.json).

The board-live layer retains each board's own proof. Orange reports strict ALSA
callback geometry and one-second thermal/load/memory sampling. Raspberry runs
fresh `Live` and `AudioDrain` probes at output 128, 256, and 512 for 30 seconds
each, with a 128-frame internal render quantum; its Orange-only callback
fields are unavailable and remain `null`. Raspberry also retains one-second
native thermal and throttling samples; every measurement requires valid startup
and runtime samples and no active undervoltage. Temperature and current
frequency-cap, throttled, and soft-limit bits are measured variables, not
admission limits; thermal and throttling effects belong in baseline
interpretation, while Raspberry firmware owns safe thermal management.
Missing or malformed system evidence is fatal. The p99.9
population is the measured observations for one native profile repetition or
the measured callbacks for one Orange live repetition. Do not combine board
populations or turn them into a single score.

Schema-4 profile rows require numeric, non-negative admission-drop evidence; a
qualified current scenario must reconcile its expected start/end counters and
report zero drops unless that scenario explicitly declares otherwise.

Orange's provisional capability geometry is output 256 → internal 128.
Raspberry's Phase 1 geometry is output 256 → internal 128. These are evidence
labels for the current branch defaults.

Print the exact deterministic plan without transport:

```powershell
./tools/orange-pi/run-orange-performance-baseline.ps1 -PrintOnly
./tools/pi/run-pi-performance-baseline.ps1 -PrintOnly
```

Run the bounded Orange canary (passive identity, one offline cell, and one
live default cell) before the full Orange study:

```powershell
./tools/orange-pi/run-orange-performance-baseline.ps1 -CanaryOnly -AllowServiceInterruption -Artifact target/orange-pi-cross/octessera-pi -Metadata target/orange-pi-cross/octessera-pi.metadata.json
```

Run the full Orange plan only with the exact release artifact and sidecar:

```powershell
./tools/orange-pi/run-orange-performance-baseline.ps1 -Phase Full -AllowServiceInterruption -Artifact target/orange-pi-cross/octessera-pi -Metadata target/orange-pi-cross/octessera-pi.metadata.json
```

The Raspberry adapter has the same print/canary/full shape and uses the fixed
Pi SSH transport, a local artifact candidate, and board metadata. Each live cell
runs three fresh processes in round-robin order for both `Live` and `AudioDrain`;
stdout, stderr, JSON summaries, and service-restoration evidence are retained:

```powershell
./tools/pi/run-pi-performance-baseline.ps1 -PrintOnly
./tools/pi/run-pi-performance-baseline.ps1 -CanaryOnly -AllowServiceInterruption -Artifact target/pi-cross/octessera-pi -Binary /usr/local/bin/octessera-pi -Metadata target/pi-cross/octessera-pi.metadata.json
./tools/pi/run-pi-performance-baseline.ps1 -Phase Full -AllowServiceInterruption -Artifact target/pi-cross/octessera-pi -Binary /usr/local/bin/octessera-pi -Metadata target/pi-cross/octessera-pi.metadata.json
```

Every full native cohort cell runs in repetition order, with a fresh runner
process for each cell; it does not run all repeats of one cell back-to-back.
Measured over-budget cells are retained and the next cell continues. Identity,
geometry, infrastructure, process, invalid evidence, service-restoration, and
electrical failures remain fatal. Both board adapters retain one-second thermal
and throttling telemetry without a project temperature ceiling. Safe thermal
management is part of measured behavior; platform firmware and the kernel own
thermal protection. Active runs also require a clean worktree, full repository
`HEAD`, and a cross-build metadata `source_commit` equal to that `HEAD`; the
local and remote artifact SHA-256 values must match. Neither adapter changes
governors or shipped defaults. The current ALSA/CPAL path cannot observe
recovered `EPIPE` events, so these tools must not claim zero ALSA xruns.
