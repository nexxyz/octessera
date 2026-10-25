# Orange Pi tools

Workstation tools for the Orange Pi Zero 2W: SSH bootstrap, the SSH/SCP
wrapper, cross-builds, development deployment, performance studies, and the
USB gadget composer. Every example takes the board target explicitly; nothing
here guesses which board you meant.

## SSH bootstrap

The bootstrap sets up one key for an Armbian Orange Pi. It creates only the
dedicated `octessera` deployment account and its SSH key authorization, and
leaves global `sshd` configuration, passwords, firewall rules, and default
users alone.

### 1. Generate the key on Windows

Run this from the repository root in PowerShell. The key is created only when
`$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519` is absent. The script never
prints the private key. Supplying a host appends a labelled stanza to
`$env:USERPROFILE\.ssh\config`; an existing different stanza is an error, not
an overwrite.

```powershell
.\tools\orange-pi\bootstrap-ssh.ps1 -HostName "<ORANGE_HOST>" -UserName octessera
```

To preview without creating or changing anything:

```powershell
.\tools\orange-pi\bootstrap-ssh.ps1 -HostName "<ORANGE_HOST>" -UserName octessera -WhatIf
```

Copy the public-key line and the exact next command printed by the script.
Copy `bootstrap-armbian-ssh.sh` to the Orange Pi first; for example, place it
in the current directory as `./bootstrap-armbian-ssh.sh` using the board's
local terminal, a console transfer, or another trusted path.

Record the deployment-key fingerprint on Windows:

```powershell
ssh-keygen -lf "$env:USERPROFILE\.ssh\octessera_orange_pi_ed25519.pub" -E sha256
```

### 2. Run on the Orange Pi terminal

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

### 3. Verify fingerprints, then connect

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

### Removal and revocation

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

## Target and transport

Set the target for the board you are operating. Use
`tools/board-dev/with-board-ssh.ps1 -Board opi` for SSH and SCP; it supplies
the dedicated Orange key, `known_hosts`, strict host-key checking, and the
passphrase from `OCTESSERA_PI_PASSPHRASE`.

```powershell
$OrangeTarget = "octessera@<ORANGE_HOST>"
./tools/board-dev/with-board-ssh.ps1 ssh -Board opi -Target $OrangeTarget "id -un; hostname"
```

For input-routing changes, use the [Orange input-routing reference](../../hardware/docs/orange-pi-input-routing.md)
and its `provision-input-routing.ps1` wrapper. The [deployment workflow](../../docs/workflows/deployment.md)
covers Raspberry deployment, Orange development deployment, and Orange input routing.

## Cross-build and stage

The WSL Docker builder writes AArch64 binaries under `target/orange-pi-cross/`,
plus schema-2 metadata sidecars for the two smoke tools:

```powershell
./tools/orange-pi/build-opi-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-opi-cross.ps1 -Binary orange-seesaw-smoke -Profile release
./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release
./tools/orange-pi/test-build-opi-cross.ps1
```

Use `-DryRun` to print the Docker command. The default profile is `pi-dev`,
so pass `-Profile release` for anything you plan to deploy. The canonical outputs are
`orange-oled-smoke` and `orange-seesaw-smoke`, each with its adjacent
`.metadata.json` file, and `octessera-pi`, whose identity on the board is its
`sha256sum`. The production image and runtime bundle use the
contracts in the [Orange production reference](../../hardware/docs/orange-pi-production-reference.md).

To stage a smoke tool on the fixed board, use the canonical wrapper and
preserve the binary and sidecar names:

```powershell
$OrangeTarget = "octessera@<ORANGE_HOST>"
./tools/board-dev/with-board-ssh.ps1 scp -Board opi `
  -Target $OrangeTarget `
  target/orange-pi-cross/orange-oled-smoke `
  target/orange-pi-cross/orange-oled-smoke.metadata.json `
  "${OrangeTarget}:/tmp/"
```

### Persistent Orange development runtime (explicit opt-in)

This is **not** the release updater. Deploy an Orange `octessera-pi` build
and return to the installed release with:

```powershell
$OrangeTarget = "octessera@<ORANGE_HOST>"
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -LocalBinary target/orange-pi-cross/octessera-pi
# Later, to return to the installed release:
./tools/orange-pi/deploy-opi-fast.ps1 -Target $OrangeTarget -Restore
./tools/orange-pi/test-deploy-opi-fast.ps1
```

Redeploying replaces the running development build. The `octessera` account
needs passwordless sudo (`bootstrap-armbian-ssh.sh --allow-deploy-sudo`). What
the deploy changes on the board is described in the
[deployment workflow](../../docs/workflows/deployment.md#how-a-development-deploy-works-both-boards).
An active service is not proof of clean audio, so keep listening and control
checks in the loop.

## Performance tools

The shared-path timing smoke, the Aux/autosave study and the load ramp all run
through `tools/pi/board-timing-smoke.sh opi`, described in
[`pi-development-and-profiling.md`](../../docs/workflows/pi-development-and-profiling.md#shared-path-timing-smoke).
For the DSP profile, build the profiling binary with
`./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release -Profiling`
and run it by hand with `--profile-dsp` while the service is stopped.

## Orange Pi USB gadget composer

`orange-pi-usb-gadget.sh` is the Armbian ConfigFS composer. It reads
`/var/lib/octessera/presets/system.json`; `audioOutputs.usb` selects UAC2 and
`usb.midiOutEnabled` selects MIDI. The fixed UDC is `musb-hdrc.4.auto`.

```sh
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh setup \
  --config /var/lib/octessera/presets/system.json
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh teardown
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

The mode follows those two settings: no gadget, `midi`, `uac2`, or
`combined`. Shipped defaults have both off, so a fresh image starts with no
gadget until you enable USB Audio or USB MIDI. Setup and teardown share
`/run/lock/octessera-orange-usb-gadget.lock`. `--lock-file`, `--configfs-root`,
and `--udc-root` are available for isolated fake-ConfigFS tests.

The product strings are `Octessera MIDI`, `Octessera Line In`, and
`Octessera Audio + MIDI` for `midi`, `uac2`, and `combined`. MIDI and combined
require a writable ConfigFS `interface_string`. The MIDI interface descriptor
and Windows `DEVPKEY_Device_BusReportedDeviceDesc` are `Octessera MIDI`; the
composer writes and verifies the 14-byte value before binding the UDC.
