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
Docker volumes. Outputs and their checked diagnostic-only metadata stay under
`target/orange-pi-cross/`. This helper cannot build, deploy, or label an Orange
runtime-ready `octessera-pi` artifact.

```powershell
./tools/orange-pi/build-orange-cross.ps1 -Binary orange-oled-smoke -Profile release
```

Use `-DryRun` to inspect the WSL Docker command without starting a container.
The only supported binary is the diagnostic-only `orange-oled-smoke`; building
it does not run it against a board.
The offline host checks are:

```powershell
./tools/orange-pi/test-build-orange-cross.ps1
```

## Orange Pi USB gadget composer

`orange-pi-usb-gadget.sh` is the separate Armbian/configfs path for the Orange
Pi. It does not reuse the Raspberry Pi gadget script, load modules, mount
configfs, enable a service, or create mass storage. The caller must prepare
configfs and load the kernel's MIDI/UAC2 function modules as appropriate.

The UDC is always explicit; the composer never picks the first controller:

```sh
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh setup \
  --udc <exact-udc-name> --mode midi
```

The supported modes are `midi`, `uac2`, and `combined`. Binding is the final
setup operation, and teardown unbinds before removing function links and
directories:

```sh
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh teardown \
  --udc <exact-udc-name>
```

Setup refuses any existing configfs gadget and any UDC already in use. The
`--configfs-root` and `--udc-root` options are for isolated fake-configfs tests
and controlled offline validation; they are not automatic discovery paths.

Run the offline tests from a Linux shell with:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```
