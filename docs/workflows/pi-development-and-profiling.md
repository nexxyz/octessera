# Pi development and profiling

This page covers host-only Pi builds and board-specific profiling. It does not
replace the ordered Orange bring-up procedure or the deployment runbook.

Canonical board IDs are `raspberry-pi-zero-2w` and `orange-pi-zero-2w`; their
images, pinouts, port roles, and deployment adapters are not interchangeable.
See [`../board-profiles.md`](../board-profiles.md) for feature owners and
supported board IDs.

## Builds without hardware

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w
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

## Pi UI and audio profiling

Pi UI/render profiling is quiet by default. Enable summaries with either form:

```bash
OCTESSERA_PI_UI_PROFILE=1 octessera-pi
octessera-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns,
snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency,
and DSP budget questions. PC/runtime-only probes do not measure hardware audio
timing. `tools/pi/run-pi-timing-probes.ps1` is
Raspberry-only; never point it at Orange.

```powershell
# Shared native logical wake probe.
cargo run -p playback-runtime --bin playback_timing_probe -- --durations 6s --scenarios idle --wake-intervals-ms 2,4,6,8,10,12

# Runtime-only probe; leaves service and live audio untouched.
./tools/pi/run-pi-timing-probes.ps1 -Mode RuntimeOnly -Durations 15s -Scenarios idle,pulses-stress -WakeIntervalsMs 2,4,6,8,10,12 -Snapshots

# Live-audio probe.
./tools/pi/run-pi-timing-probes.ps1 -Mode Live -AllowServiceInterruption -Durations 10m -Scenarios idle -WakeIntervalsMs 2,4,6,8,10,12

# Audio-source drain latency probe.
./tools/pi/run-pi-timing-probes.ps1 -Mode AudioDrain -AllowServiceInterruption -Durations 10m

# FX budget profile.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits -AllowServiceInterruption

# Alternate render quantum.
./tools/pi/run-pi-timing-probes.ps1 -Mode DspFxLimits -AllowServiceInterruption -AudioRenderQuantumFrames 256
```

The shared binary uses a deterministic logical matrix and the Pi RuntimeOnly
mode leaves the service and live audio untouched. Pi Live opens live audio.
Both Pi modes pass the actual elapsed interval to the shared runtime. These
metrics diagnose trigger cadence and event batching; use `AudioDrain` separately
for queue/control-drain measurements.

The wrapper stops `octessera.service` for live/audio/DSP modes and restarts it
afterward; those modes require `-AllowServiceInterruption`. Runtime-only leaves
it running. Use `-PrintOnly` to inspect the remote command first.

After live probes, inspect recent logs:

```powershell
./tools/pi/with-pi-ssh.ps1 ssh pi@192.168.0.218 "journalctl -u octessera.service --since '10 minutes ago' --no-pager | grep -E 'audio callback RT promotion not qualified|audio stream error|underrun|POLLERR' || true"
```

Use `-PrintOnly` before any live or service-changing probe. Runtime-only mode
leaves the service running; live, audio-drain, and DSP modes stop the service
and restart it afterward. These probes measure timing and audio-path behavior;
they do not replace board bring-up or image checks.
