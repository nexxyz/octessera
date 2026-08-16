# Orange Pi Zero 2W selection and qualification history

> **Selection complete.** This is the single non-normative record of the Orange Pi selection and qualification history. For the current production procedure, see [`orange-pi-armbian-bringup.md`](orange-pi-armbian-bringup.md).

The “current” and “pending” language below is preserved as the original
decision-stage snapshot; it does not describe the current production procedure.

## Historical distro evaluation

### Orange Pi Zero 2W distro evaluation

Goal: choose the best OS base before adding Orange Pi behavior or broad multi-board support.

Status:

- Desk evaluation: complete.
- On-device validation: pending Orange Pi Zero 2W hardware.
- Current recommendation: start hands-on evaluation with Armbian Debian 13/Trixie, keep the official Orange Pi image as the hardware fallback, and use DietPi as the footprint benchmark.
- Armbian bring-up details, including USB device/gadget mode checks: `hardware/docs/orange-pi-armbian-bringup.md`.

## Constraints

- Target board: Orange Pi Zero 2W, 2 GB RAM.
- Hardware goal: drop-in replacement for Raspberry Pi Zero 2 W.
- PCB constraint: no PCB changes.
- Wiring constraint: same OLED, NeoTrellis/NeoKey, encoders/buttons, and audio DAC wiring as the Raspberry Pi build.
- Current scope: PCB and software compatibility only. Ignore case compatibility until later.
- Repo goal: complete multi-board support for app code, HAL/profile handling, deploy scripts, image builds, release artifacts, and docs.
- Naming: use the canonical board profile IDs `raspberry-pi-zero-2w` and `orange-pi-zero-2w`.

## Candidate images

Evaluate these before adding Orange Pi behavior or broad multi-board refactors:

1. Armbian Debian 13/Trixie for Orange Pi Zero 2W.
2. DietPi Trixie if Orange Pi Zero 2W support is current.
3. Official Orange Pi/vendor image.
4. OpenWrt only if the first three options fail or a very small network-appliance image becomes the priority.

## Current ranking to test

| Rank | Candidate | Why test it | Main risk |
| --- | --- | --- | --- |
| 1 | Armbian Debian 13/Trixie | Best balance of normal Debian userspace, current packages, and reproducible build framework. | Board page is a rolling release, so production stability must be proven. |
| 2 | Official Orange Pi/vendor image | Most likely to expose board peripherals correctly during bring-up. | Older distro/kernel and weaker reproducible release-image story. |
| 3 | DietPi Trixie | Small footprint and appliance-friendly setup. | Support is DietPi-specific; kernel, Wi-Fi, Bluetooth, and audio issues may be out of scope. |
| 4 | OpenWrt | Very small appliance base. | More integration work for Rust app runtime, audio, I2S, and normal Linux service assumptions. |

## Desk evaluation results

| Candidate | Current status | Reproducible image path | Hardware-support expectation | Keep evaluating? |
| --- | --- | --- | --- | --- |
| Armbian Debian 13/Trixie | Board page lists Debian 13/Trixie for Orange Pi Zero 2W, current kernel, rolling release. | Strong. Armbian build framework supports board/release/minimal image parameters. | Good for I2C, SPI, and GPIO; I2S DAC depends on device-tree/overlay maturity. | Yes, primary target. |
| DietPi Trixie | DietPi lists an Orange Pi Zero 2W ARMv8 Trixie image. | Moderate. Good appliance tooling, but less board-focused than Armbian. | Likely OK for basic Linux bring-up; board-specific audio/kernel issues may be outside DietPi support. | Yes, footprint benchmark. |
| Official Orange Pi/vendor image | Vendor docs list Debian/Ubuntu/Android/Orange Pi OS images. | Weak to moderate. Vendor build/customization flow is less clean for release automation. | Best chance of vendor bootloader/device-tree peripheral support. | Yes, hardware fallback. |
| OpenWrt | Upstream support exists, but with caveats such as missing Wi-Fi driver in the referenced support work. | Strong if we accept OpenWrt buildroot-style workflow. | Good for lean appliance basics; weakest fit for Octessera audio/I2S and normal Linux userspace assumptions. | Low priority. |

## Scorecard

Score each candidate from 1 to 5.

| Criterion | Armbian | DietPi | Vendor image | OpenWrt |
| --- | --- | --- | --- | --- |
| First boot and SSH setup | 4 | 4 | 3 | 2 |
| Package install and update reliability | 5 | 4 | 2 | 3 |
| I2C device exposure | 4 | 3 | 5 | 3 |
| SPI device exposure | 4 | 3 | 5 | 3 |
| GPIO interrupt support | 4 | 3 | 5 | 3 |
| I2S DAC/audio device support | 3 | 3 | 4 | 2 |
| Audio latency and buffer behavior | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Service/systemd setup | 5 | 4 | 3 | 2 |
| Idle CPU and RAM footprint | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Boot time | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Thermal behavior and governor support | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Reproducible image-build path | 5 | 4 | 2 | 4 |
| Release stability confidence | 3 | 3 | 4 | 3 |

Preliminary hypothesis score, to replace with hardware data:

| Candidate | Score | Role |
| --- | ---: | --- |
| Armbian Debian 13/Trixie | 4/5 | Primary candidate. |
| DietPi Trixie | 4/5 | Footprint and ease-of-use benchmark. |
| Official Orange Pi/vendor image | 3/5 | Hardware-support fallback. |
| OpenWrt | 3/5 | Low-priority lean-appliance alternative. |

## Bring-up checks per image

Run these on each serious candidate before selecting the base image.

The desk evaluation cannot complete these checks. They require the Orange Pi Zero 2W and the no-PCB-change wiring harness.

### Basic OS

- Flash image and boot without desktop packages.
- Confirm SSH access and stable network reconnect after reboot.
- Record kernel version, distro version, image date, and bootloader/source image URL.
- Install build/runtime dependencies needed by Octessera.
- Confirm systemd service installation works without distro-specific hacks.

### Hardware paths

- Confirm I2C bus path and scan seesaw devices.
- Confirm SPI device path and run a minimal OLED transfer test.
- Confirm GPIO chip and line mapping for all buttons/encoders/interrupts.
- Confirm GPIO interrupts work without polling-only fallbacks.
- Confirm I2S DAC is available and selected as the intended audio output.
- Run a short audio playback test and record underruns/dropouts.
- Confirm USB device/gadget mode exposes a UDC through `/sys/class/udc`.
- Confirm the Orange Pi configfs composer binds MIDI, UAC2 audio, and combined modes on the OTG/data port.
- Confirm Armbian uses `/boot/armbianEnv.txt` overlays for required buses; do not use Raspberry Pi `config.txt`/`dtoverlay` assumptions.
- Confirm shutdown/reboot behavior and any required privilege policy.

### Performance and stability

- Measure idle CPU/RAM after boot and after Octessera service start.
- Measure boot-to-service-ready time.
- Run the existing runtime/audio benchmark scenarios.
- Run a long audio/service soak test.
- Record thermals, throttling, and governor behavior.

## Decision rules

- Prefer Armbian if it exposes I2C, SPI, GPIO interrupts, and I2S audio cleanly and remains stable under soak testing.
- Prefer Armbian only if USB gadget/device mode is available without board-specific kernel patches we cannot carry.
- Prefer the vendor image only if Armbian has peripheral problems that cost more than the weaker image/release workflow.
- Prefer DietPi only if it clearly reduces setup and runtime footprint without making hardware support harder.
- Do not add Orange Pi behavior or broad board-profile refactors until one lead distro is selected and the no-PCB-change hardware path looks viable. Raspberry-only no-behavior-change foundation refactors are allowed.

## Expected repo work after selection

1. Extract current Raspberry Pi assumptions into `raspberry-pi-zero-2w` profile without behavior changes.
2. Add `orange-pi-zero-2w` profile and the required GPIO backend only after hardware validation.
3. Parameterize bus paths, GPIO mapping, audio device selection, diagnostics, service account/home paths, deploy target, image sanitation, and release artifact names.
4. Keep `platform-core`, `playback-runtime`, and `realtime-engine` shared.
5. Keep Raspberry Pi and Orange Pi image pipelines parallel.

## Diagnostic qualification contract (historical bring-up)

This section describes the separate diagnostic image and smoke-utility path. Run
it in order on one identified board when qualifying hardware. It intentionally
does not install or enable the production service. The production path is
documented in the [current Armbian bring-up procedure](orange-pi-armbian-bringup.md)
and under [Runtime service status](orange-pi-armbian-bringup.md#runtime-service-status).
Record the board revision, PCB/harness revision, image/kernel/DT identity,
artifact SHA-256, and timestamps for every gate.

### Passive gate

Before any transfer, GPIO request, audio playback, USB bind, or runtime launch:

- Confirm the board is exactly `orangepizero2w`, the recovery path works, and no
  Octessera service or other process owns the connected hardware.
- Reconfirm the live DT/pinmux mappings for I2C, SPI1/CS0, OLED D/C/reset, I2S,
  USB role, and UDC. Record device nodes, GPIO ownership, `aplay -l`, and
  `/sys/class/udc`; do not infer a mapping from a Raspberry Pi number.
- Confirm the candidate is an Orange Pi artifact with matching metadata. Stage
  it only under `/tmp`; do not install it, replace a release, or start a
  service.
- Confirm the exact USB-C OTG/data port from the schematic and board. With the
  board power arrangement documented, measure VBUS and CC/role state with the
  host disconnected and connected. Pass only when the expected host/device
  direction, peripheral role, and no-backfeed/no-brownout behavior are proven.

### Reboot and staging gate

`/tmp` is cleared by reboot. After every controlled reboot, do not run a probe
or active test until SSH has returned and the artifact has been staged again:

```powershell
$Target = "orangepi@<address>"
$Artifact = "<local-path-to-orange-oled-smoke>"
$Metadata = "$Artifact.metadata.json"
$RemoteArtifact = "/tmp/orange-oled-smoke"
$RemoteMetadata = "/tmp/orange-oled-smoke.metadata.json"
$SshOptions = @("-o", "BatchMode=yes", "-o", "ConnectTimeout=5")
$Deadline = (Get-Date).AddMinutes(5)
$Reachable = $false
while ((Get-Date) -lt $Deadline) {
  & ssh @SshOptions $Target "true"
  if ($LASTEXITCODE -eq 0) { $Reachable = $true; break }
  Start-Sleep -Seconds 2
}
if (-not $Reachable) { throw "post-reboot SSH poll timed out; stop" }
& scp @SshOptions $Artifact "${Target}:$RemoteArtifact"
if ($LASTEXITCODE -ne 0) { throw "artifact redeploy failed; stop" }
& scp @SshOptions $Metadata "${Target}:$RemoteMetadata"
if ($LASTEXITCODE -ne 0) { throw "metadata sidecar redeploy failed; stop" }
& ssh @SshOptions $Target "chmod 0755 '$RemoteArtifact' && '$RemoteArtifact' --print-build-metadata"
if ($LASTEXITCODE -ne 0) { throw "staged artifact metadata check failed; stop" }
$LocalSha = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$RemoteShaOutput = @(& ssh @SshOptions $Target "sha256sum -- '$RemoteArtifact'")
if ($LASTEXITCODE -ne 0) { throw "remote SHA-256 command failed; stop" }
if ($RemoteShaOutput.Count -ne 1) { throw "remote SHA-256 output was not exactly one record; stop" }
$RemoteShaRecord = ([string]$RemoteShaOutput[0]).Trim()
$ShaPattern = "^(?<Hash>[0-9a-f]{64})\s+(?<Path>$([regex]::Escape($RemoteArtifact)))$"
$ShaMatch = [regex]::Match($RemoteShaRecord, $ShaPattern)
if (-not $ShaMatch.Success) { throw "remote SHA-256 output had an invalid format; stop" }
$RemoteSha = $ShaMatch.Groups["Hash"].Value
if ($RemoteSha -ne $LocalSha) { throw "remote binary SHA-256 differs from the recorded local SHA-256; stop" }
```

The metadata validation and the independent remote SHA-256 comparison are both
required before launching anything. Repeat this poll-and-redeploy sequence
after every reboot, including one caused by an overlay change. The utility's
metadata mode reads only the adjacent exact-name sidecar and hashes its running
`/proc/self/exe`; it performs no hardware initialization.

Stage the Seesaw diagnostic under its canonical names and repeat the same
independent SHA-256 check. The metadata command is intentionally unprivileged:

```powershell
$SeesawArtifact = "<local-path-to-orange-seesaw-smoke>"
$SeesawMetadata = "$SeesawArtifact.metadata.json"
$RemoteSeesawArtifact = "/tmp/orange-seesaw-smoke"
$RemoteSeesawMetadata = "/tmp/orange-seesaw-smoke.metadata.json"
& scp @SshOptions $SeesawArtifact "${Target}:$RemoteSeesawArtifact"
if ($LASTEXITCODE -ne 0) { throw "Seesaw artifact upload failed; stop" }
& scp @SshOptions $SeesawMetadata "${Target}:$RemoteSeesawMetadata"
if ($LASTEXITCODE -ne 0) { throw "Seesaw metadata upload failed; stop" }
& ssh @SshOptions $Target "chmod 0755 '$RemoteSeesawArtifact' && '$RemoteSeesawArtifact' --print-build-metadata"
if ($LASTEXITCODE -ne 0) { throw "Seesaw metadata check failed; stop" }
$SeesawLocalSha = (Get-FileHash -LiteralPath $SeesawArtifact -Algorithm SHA256).Hash.ToLowerInvariant()
$SeesawRemoteSha = @(& ssh @SshOptions $Target "sha256sum -- '$RemoteSeesawArtifact'")
if ($LASTEXITCODE -ne 0 -or $SeesawRemoteSha.Count -ne 1) { throw "Seesaw remote SHA-256 check failed; stop" }
if (([string]$SeesawRemoteSha[0]).Trim() -notmatch "^$SeesawLocalSha\s+$([regex]::Escape($RemoteSeesawArtifact))$") {
  throw "Seesaw binary SHA-256 differs from the recorded local SHA-256; stop"
}
```

### Active gate and order

Proceed only when the passive gate, staging gate, and USB electrical gate pass:

The OLED operation has a cooperative 3-second budget and a cooperative
1-second cleanup budget. Normal shutdown performs black and display-off
together; error and interruption cleanup uses one deadline, prioritizing
display-off before the fallback black frame. Synchronous SPI/GPIO calls may
outlast these checks, so neither budget is a wall-clock promise.

1. **Seesaw:** run `sudo -n /tmp/orange-seesaw-smoke --confirm-active-test` only after
   the passive and staging gates pass. It resets the four NeoTrellis addresses
   and NeoKey address on `/dev/i2c-2`, then reads their valid hardware IDs; it
   does not configure keypad events, write LEDs, poll keys, request GPIO, access
   OLED/SPI/audio, start runtime, or install a service.
2. **OLED:** run the diagnostic-only utility from `/tmp`. One invocation owns
   the cooperative pattern-to-black-to-display-off sequence, with operation
   and cleanup budgets, cleanup on errors, and handled interruption. Blocking
   SPI/GPIO syscalls are synchronous and may outlast those cooperative checks;
   record that limitation rather than treating the budgets as a wall-clock
   promise. Do not split it into separate commands:

   ```sh
   sudo -n /tmp/orange-oled-smoke --confirm-active-test
   ```

3. **I2S/DAC:** enumerate ALSA, select the exact CPAL endpoint
   `hw:CARD=octesseradac,DEV=0` at the shared 44.1 kHz runtime rate, and run
   one short playback plus an underrun check. A sound from HDMI or an
   implicit/default ALSA device is not an I2S pass.
4. **HDMI:** after the I2S result is recorded, enumerate HDMI separately and
   confirm it has not been selected as an audio fallback. Do not use HDMI to
   qualify the DAC wiring.
5. **USB gadget:** only after I2S and HDMI checks, recheck VBUS/CC/role and the
   exact UDC, bind one composer mode, verify host enumeration, then unbind and
   verify clean teardown. Use the Orange Pi composer; do not bind a pre-existing
   gadget or guess a UDC.
6. **Historical `runtime-candidate` step:** this old qualification step is retained
   for reproducibility only. It may use the profile-matched diagnostic utility
   or the old `runtime-candidate` from `/tmp`; keep deployment, release, and
   production service paths untouched. It is not the 0.7.5 production runtime.

### Stop conditions

Stop the session, preserve logs and measurements, and do not retry or reorder a
gate if any of these occurs: the SSH poll times out; the board identity, boot
DT, pinmux, artifact metadata, or SHA-256 differs; `/tmp` staging or metadata
validation fails; the cooperative OLED operation or cleanup budget is
exhausted, or black/display-off cannot be confirmed; any bus hangs, GPIO
ownership mismatch, kernel fault, brownout, thermal rise, unexpected reboot, or hardware owner appears; the I2S card is absent, an
audio test falls back to HDMI, or playback underruns; VBUS/CC/OTG direction is
unproven, backfeed or power loss appears, UDC is absent/pre-bound, host
enumeration fails, or gadget teardown cannot unbind cleanly. Do not continue
to runtime qualification after a failed gate.

## Historical repo follow-up (pre-0.7.5)

These notes record the implementation work that preceded the production image;
they are not outstanding release tasks:

1. Add real `orange-pi-zero-2w` board profile values.
2. Add a non-`rppal` GPIO backend based on gpiochip lines.
3. Split gadget setup by board/image layer so Raspberry Pi keeps `dwc2` and Orange Pi uses the detected UDC path.
4. Parameterize service user, store paths, samples paths, deploy target, preflight checks, and image sanitation.
5. Add Orange Pi image automation as a parallel Armbian path, not a pi-gen variant.
