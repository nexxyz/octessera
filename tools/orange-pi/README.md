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
