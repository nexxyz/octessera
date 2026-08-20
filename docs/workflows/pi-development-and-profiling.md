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

# Current high-headroom Pi settings.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits -SynthSlotWorkers 2 -AudioBlockFrames 256
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
capabilities. Inspect p99/p99.9/p99.99 and outlier counts, not only p95.

After live probes, inspect recent logs:

```powershell
./tools/pi/with-pi-ssh.ps1 ssh pi@192.168.0.218 "journalctl -u octessera.service --since '10 minutes ago' --no-pager | grep -E 'audio callback RT promotion not qualified|audio stream error|underrun|POLLERR' || true"
```

Offer a live probe when the report is subjective or audio-path-specific; do not
run long live probes for unrelated changes.
