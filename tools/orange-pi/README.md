# Orange Pi SSH bootstrap

This is a one-key bootstrap for an Armbian Orange Pi. It creates only the
dedicated `octessera` deployment account and its SSH key authorization. It does
not edit global `sshd` configuration, passwords, firewall rules, or default
users. No private key or host address belongs in the repository.

## 1. Generate the key on Windows

Run this from the repository root in PowerShell. The key is created only when
`$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519` is absent. The script never
prints the private key. Supplying a host appends a labelled stanza to
`$env:USERPROFILE\.ssh\config`; an existing different stanza is an error, not
an overwrite.

```powershell
.\tools\orange-pi\bootstrap-ssh.ps1 -HostName 192.168.1.50 -UserName octessera
```

To preview without creating or changing anything:

```powershell
.\tools\orange-pi\bootstrap-ssh.ps1 -HostName 192.168.1.50 -UserName octessera -WhatIf
```

Copy the public-key line and the exact next command printed by the script.
Copy `bootstrap-armbian-ssh.sh` to the Orange Pi first; for example, place it
in the current directory as `./bootstrap-armbian-ssh.sh` using the board's
local terminal, a console transfer, or another trusted path.

Record the deployment-key fingerprint on Windows:

```powershell
ssh-keygen -lf "$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519.pub" -E sha256
```

## 2. Run on the Orange Pi terminal

Run this on the Armbian board, not on Windows. Replace the quoted key with the
single line printed in step 1:

```sh
sudo bash ./bootstrap-armbian-ssh.sh 'ssh-ed25519 AAAA... octessera-orange-pi'
```

The script is idempotent. It preserves existing `authorized_keys` entries and
refuses unexpected existing `octessera` homes, groups, or sudoers rules. It
requires exactly one `ssh-ed25519` public-key argument. Passwordless sudo is
not enabled unless explicitly requested:

```sh
sudo bash ./bootstrap-armbian-ssh.sh --allow-deploy-sudo 'ssh-ed25519 AAAA... octessera-orange-pi'
```

That opt-in grants `octessera` passwordless sudo for all commands and writes
`/etc/sudoers.d/octessera-deploy`; the script validates the rule with
`visudo`. Without the flag, the standard sudo policy is unchanged.

## 3. Verify fingerprints, then connect

Before accepting a host key, compare the board's host-key fingerprint with the
fingerprint shown by Windows. On the Orange Pi's local terminal:

```sh
sudo ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub -E sha256
```

On Windows, replace `<ORANGE_PI_HOST>` with the user-supplied IP or hostname:

```powershell
ssh-keyscan -t ed25519 <ORANGE_PI_HOST> | ssh-keygen -lf - -E sha256
```

Only continue when those fingerprints match. Then test the dedicated key
(replace the host with the same user-supplied value):

```powershell
ssh -i "$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519" -o IdentitiesOnly=yes octessera@<ORANGE_PI_HOST> "id -un; hostname; test -r ~/.ssh/authorized_keys"
```

If the local script added its stanza, the shorter equivalent is:

```powershell
ssh octessera-orange-pi "id -un; hostname; test -r ~/.ssh/authorized_keys"
```

Review the host key before answering the first SSH authenticity prompt. Do not
use `StrictHostKeyChecking=accept-new` as a substitute for comparing the
fingerprint.

## Removal and revocation

To revoke this key but keep the deployment account, remove its exact public
key line from `/home/octessera/.ssh/authorized_keys` on the Orange Pi and then
remove the local key files if no longer needed:

```sh
sudoedit /home/octessera/.ssh/authorized_keys
```

```powershell
Remove-Item "$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519", "$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519.pub"
```

To remove the account and its home after revoking the key:

```sh
sudo userdel --remove octessera
sudo groupdel octessera
```

If passwordless deploy sudo was enabled, remove only the drop-in after checking
that it is the rule created for this account:

```sh
sudo rm -- /etc/sudoers.d/octessera-deploy
sudo visudo -c
```

Remove the labelled `OCTESSERA ORANGE PI` stanza from the Windows SSH config
manually. Do not commit private keys, public keys, fingerprints tied to a
specific board, hostnames, IP addresses, or generated SSH config to Git.

## Local WSL Docker cross-build

Build Orange Pi artifacts on Windows without contacting or deploying to a
board. The builder starts an ephemeral Debian tool container, installs the
aarch64 GNU linker/sysroot there, and keeps Cargo and rustup data in named
Docker volumes. Outputs and their checked metadata stay under
`target/orange-pi-cross/`. The supported local outputs are the canonical
`orange-oled-smoke`, `orange-seesaw-smoke`, and `octessera-pi` development
binaries beside matching `.metadata.json` sidecars. Each sidecar is schema 2
and binds the copied ELF with its lowercase SHA-256. This helper does not build
the 0.7.5 production image or its hash-bound `production-runtime` bundle, and
it never deploys an artifact. The production image and service support the
shared 44.1 kHz runtime, the OLED, NeoTrellis, NeoKey, four encoders, store,
samples, MIDI, and the required internal DAC at
`hw:CARD=octesseradac,DEV=0`. USB-only audio is unsupported; UAC2 is an
optional companion and `audioOut=usb` is rejected.

```powershell
./tools/orange-pi/build-orange-cross.ps1 -Binary orange-oled-smoke -Profile release
```

Use `-DryRun` to inspect the WSL Docker command without starting a container.
The two smoke binaries are diagnostic-only. The local `octessera-pi` output is
for development and qualification; building any output does not run it against
a board. The production release artifact is
`octessera-0.7.5-orange-pi-zero-2w.img.xz`, built with explicit production image
mode. Orange update check, apply, rollback, and OTA remain unsupported.
The offline builder test uses a temporary binary and adjacent sidecar, checks a
tampered sidecar, and confirms failed verification removes both artifacts.
The offline host checks are:

```powershell
./tools/orange-pi/test-build-orange-cross.ps1
```

When staging on a board, copy both files to their canonical names under `/tmp`:

```powershell
$Target = "orangepi@<address>"
$Artifact = "target/orange-pi-cross/orange-oled-smoke"
$Metadata = "$Artifact.metadata.json"
$RemoteArtifact = "/tmp/orange-oled-smoke"
$RemoteMetadata = "/tmp/orange-oled-smoke.metadata.json"
$SshOptions = @("-o", "BatchMode=yes", "-o", "ConnectTimeout=5")
& scp @SshOptions $Artifact "${Target}:$RemoteArtifact"
if ($LASTEXITCODE -ne 0) { throw "artifact upload failed" }
& scp @SshOptions $Metadata "${Target}:$RemoteMetadata"
if ($LASTEXITCODE -ne 0) { throw "metadata sidecar upload failed" }
& ssh @SshOptions $Target "chmod 0755 '$RemoteArtifact' && '$RemoteArtifact' --print-build-metadata"
if ($LASTEXITCODE -ne 0) { throw "staged metadata check failed" }
$LocalSha = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$RemoteShaOutput = @(& ssh @SshOptions $Target "sha256sum -- '$RemoteArtifact'")
if ($LASTEXITCODE -ne 0) { throw "remote SHA-256 command failed" }
if ($RemoteShaOutput.Count -ne 1) { throw "remote SHA-256 output was not exactly one record" }
$RemoteShaRecord = ([string]$RemoteShaOutput[0]).Trim()
$ShaPattern = "^(?<Hash>[0-9a-f]{64})\s+(?<Path>$([regex]::Escape($RemoteArtifact)))$"
$ShaMatch = [regex]::Match($RemoteShaRecord, $ShaPattern)
if (-not $ShaMatch.Success) { throw "remote SHA-256 output had an invalid format" }
if ($ShaMatch.Groups["Hash"].Value -ne $LocalSha) { throw "remote SHA-256 mismatch" }
```

Run `Get-FileHash` locally and validate exactly one, well-formed remote
`sha256sum` record before comparing it. Fail closed on an SSH failure, extra or
missing output, malformed output, or a mismatch; metadata validation alone is
not a transport check. The default passive qualification probe also needs
passwordless `sudo -n` (or a root SSH session) to prove that no process owns the
target devices.

## Orange capability study runner

Phase 1 uses the existing offline DSP scenarios as the low-risk Orange
measurement affordance. The Raspberry timing launcher is deliberately not an
Orange entry point. The fixed-board runner uses only the local
`with-orange-ssh.ps1` wrapper, stages a release `octessera-pi` and its exact
schema-2 sidecar under a unique `/tmp` path bound to the binary SHA-256, and
retrieves one bounded evidence bundle before removing the remote staging path.

Preview every generated command without connecting:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode PassiveBaseline -PrintOnly
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64 -ProfileMode soak -PrintOnly
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp256 -ProfileMode soak -PrintOnly
```

After reviewing the plan, non-print DSP runs are active measurements and must
acknowledge service interruption:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp64 -ProfileMode soak -AllowServiceInterruption
./tools/orange-pi/run-orange-capability-study.ps1 -Mode Dsp256 -ProfileMode soak -AllowServiceInterruption
```

The DSP modes pass `--profile-dsp` explicitly and set the internal block size
to 64 or 256 frames. They do not set the environment-only trigger. They collect
low-rate CPU/load, memory, thermal, frequency, and service-state evidence in a
single remote shell; they do not stream SSH or journal logs continuously.

The existing `opi-bringup-validator.sh` is a pristine qualification probe. It
checks that no production service or target-device owner is running, so it will
correctly flag a deliberately running `octessera.service`. The passive study
does not make that claim: it reports the initial service state and leaves it
alone.

The bounded live-candidate plan is reserved for Phase 2 and requires an
explicit interruption acknowledgement:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 -Mode LiveCandidate -AllowServiceInterruption -PrintOnly
```

Its transient unit keeps the production runtime account, `Nice=-10`,
`LimitRTPRIO=70`, `/var/lib/octessera/presets`, `/var/lib/octessera/samples`,
and the compiled DAC route `hw:CARD=octesseradac,DEV=0`. Active runs require
that initial service state to be active and enabled. A remote trap captures,
restores, waits for active, and records the final state without changing
enablement. It makes no power, flash, cable, suspend, GPIO, OLED, or USB
changes. The production sandbox properties also include
`ProtectKernelTunables=yes`, `ProtectKernelModules=yes`,
`ProtectControlGroups=yes`, `RestrictNamespaces=yes`, and
`LockPersonality=yes`. Its existing deliberate sandbox difference is
`PrivateTmp=no`; the candidate health marker itself lives at a unique
`/run/octessera/candidate-health-...json` path under the transient
`RuntimeDirectory`, not in `/tmp`. The production unit itself is not changed.

### Phase 2 live audio benchmark

`LiveAudioBenchmark` runs one approved scenario through the reviewed native
benchmark CLI. It requires the exact scenario, output buffer, engine block,
worker count, measure duration, artifact, metadata sidecar, and
service-interruption consent.
The runner waits for readiness and fixed DAC ALSA geometry before publishing the
identity-bound release file that lets the native process continue.

Readiness, progress, and release evidence use schema 2. Terminal results use
schema 3, retain optional exact worker delta and policy-error evidence, and are
independently recomputed by the host. Requested output buffer, negotiated ALSA
period, and internal engine block are separate fields. CPAL callback batches are
variable positive counts no larger than the requested buffer; render/audio-
duration ratios use each callback's actual frame count, and callback-spacing
lateness uses the fixed ALSA period. Schema-1 readiness/progress/release and
schema-2 terminal results are rejected. Callback batch size changes are
recorded as evidence, not treated as a period mismatch; zero or oversized
batches and invalid-frame counts remain terminal failures. Each retained result
also exposes the aggregate render-duration ratio from total render nanoseconds,
rendered frames, and sample rate; missing or zero aggregate evidence fails
closed.

Preview one cell without transport:

```powershell
./tools/orange-pi/run-orange-capability-study.ps1 `
  -Mode LiveAudioBenchmark `
  -Scenario synth_cross_slot_96_steal `
  -OutputFrames 256 `
  -EngineBlockFrames 256 `
  -Workers 2 `
  -MeasureSeconds 30 `
  -Artifact target/orange-pi-cross/octessera-pi `
  -Metadata target/orange-pi-cross/octessera-pi.metadata.json `
  -AllowServiceInterruption -PrintOnly
```

The individual runner approves output/engine/workers tuples 256/64/2,
256/256/0, 256/256/2, 512/128/2, and 1024/256/0|2|3. ALSA periods remain
fixed by output at 64/128/256 for 256/512/1024 output frames. The fixed matrix
is A (256/64, workers 2, all 11 scenarios), B (512/128, workers 2, all 11),
and C0/C2/C3 (1024/256, workers 0/2/3, synth and mixed steal scenarios). A
120-second run is accepted only with the explicit
`-AllowLongRepeat` switch; the matrix runner selects the worst passing A cell
before requesting that repeat.

Preview the complete deterministic matrix order:

```powershell
./tools/orange-pi/run-orange-live-audio-matrix.ps1 -PrintOnly
```

Active matrix execution requires both `-AllowMatrixServiceInterruption` on the
matrix runner and the per-cell interruption consent it supplies. The host keeps
the fixed DAC identity, samples thermal and memory safety outside the callback,
retrieves evidence on failure, and verifies production restoration after every
cell. Phase 2 is host-only tooling; it does not change hardware configuration,
capabilities, defaults, or user documentation claims.

Offline DSP rows locate computational knees; they are not live-xrun proof. The
current CPAL/ALSA path also cannot count internally recovered `EPIPE` events.
Therefore these tools must not claim zero xruns or change platform capabilities.

Run the host-only command-generation test without a board:

```powershell
./tools/orange-pi/test-run-orange-capability-study.ps1
```

## Orange Pi USB gadget composer

`orange-pi-usb-gadget.sh` is the separate Armbian/configfs path for the Orange
Pi. It does not reuse the Raspberry Pi gadget script, `dwc2`, BCM numbering, or
mass storage. The image service loads the board modules and owns the combined
UAC2/MIDI lifecycle.

The UDC is fail-closed and fixed to the verified `musb-hdrc.4.auto`; the
composer never picks the first controller:

```sh
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh setup --mode combined
```

The supported modes are `midi`, `uac2`, and `combined`. Binding is the final
setup operation, and teardown unbinds before removing function links and
directories:

```sh
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh teardown
```

Setup and teardown take the same exclusive lifecycle lock at
`/run/lock/octessera-orange-usb-gadget.lock`; a concurrent invocation fails
without changing the gadget. `--lock-file` is available for isolated
fake-ConfigFS tests.

Setup refuses any existing configfs gadget and any UDC already in use. The
`--configfs-root` and `--udc-root` options are for isolated fake-configfs tests
and controlled offline validation; they are not automatic discovery paths. The
supported modes are `midi`, `uac2`, and `combined`; the installed image always
uses `combined`.

The USB product string is `Octessera MIDI` for `midi`, `Octessera Line In` for
`uac2`, and `Octessera Audio + MIDI` for `combined`. The manufacturer,
configuration, serial, VID, and PID remain the Orange Pi values used by the
composer. MIDI and combined modes require the patched, qualified image kernel
to expose a writable `interface_string`. The composer writes exactly 14 bytes
of `Octessera MIDI` without a trailing LF, verifies the byte-for-byte readback,
and only then creates the MIDI configuration link and binds the UDC. `id` is
still set for ALSA identity, but never substitutes for `interface_string`. A
generic Windows `MIDI function` label indicates an unpatched or unqualified
image and is not accepted for release validation.

Run the offline tests from a Linux shell with:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```
