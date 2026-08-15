# Pi provisioning

`../provision-pi.ps1` installs the development Pi OS, boot, network, sudoers,
performance, and splash configuration. It is safe to run again after changing
the tracked files in this directory or the shared Pi image files.

Provision the device before the first fast deployment:

```powershell
./tools/pi/provision-pi.ps1 -Target pi@192.168.0.218 -BoardProfile raspberry-pi-zero-2w
```

The selected initramfs is not refreshed by default. Pass `-UpdateInitramfs` for
an explicit OS or boot rebuild; the direct script accepts
`--update-initramfs`. Provisioning removes the retired Raspberry animation hook
and script before an explicit rebuild, so no board carries animation in its
initramfs. The deployment script does not change OS configuration; it uploads
the binary or source, restarts the service, and can tail its logs.
Raspberry Pi provisioning rejects `orange-pi-zero-2w`; Orange Pi uses the
separate Armbian bring-up path.

The shared transport wrapper reads an encrypted-key passphrase only from the
process environment and removes its temporary askpass helper when it exits.
Set `OCTESSERA_PI_PASSPHRASE` in the current PowerShell process first:

```powershell
# Set OCTESSERA_PI_PASSPHRASE in this PowerShell process before these commands.
./tools/pi/with-pi-ssh.ps1 ssh -Target pi@192.168.0.218 "hostname"
./tools/pi/with-pi-ssh.ps1 scp -Target pi@192.168.0.218 ./candidate.bin pi@192.168.0.218:/tmp/candidate.bin
./tools/pi/with-pi-ssh.ps1 ssh-payload -Target pi@192.168.0.218 ./remote-script.sh
```
