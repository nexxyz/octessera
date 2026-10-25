# Pi provisioning

`tools/pi/provision-pi.ps1` uploads this directory and runs `provision.sh` on
the board. It installs the development Pi OS, boot, network, sudoers,
performance, and splash configuration, and it is safe to run again after
changing the tracked files here or the shared Pi image files.

Provision the device before the first fast deployment:

```powershell
$PiTarget = "pi@<PI_HOST>"
./tools/pi/provision-pi.ps1 -Target $PiTarget -BoardProfile raspberry-pi-zero-2w
```

The selected initramfs is not refreshed by default. Pass `-UpdateInitramfs` for
an explicit OS or boot rebuild; the direct script accepts
`--update-initramfs`. Provisioning removes the retired Raspberry animation hook
and script before an explicit rebuild, so no board carries animation in its
initramfs. The deployment script does not change OS configuration; it uploads
the binary or source, restarts the service, and can tail its logs.
Raspberry Pi provisioning rejects `orange-pi-zero-2w`; Orange Pi uses the
separate Armbian bring-up path.

The shared board transport wrapper (`-Board rpi` or `-Board opi`) uses that
board's dedicated key and strict host checking. It reads an encrypted-key
passphrase only from the process environment and removes its temporary askpass
script when it exits. Set `OCTESSERA_PI_PASSPHRASE` in the current PowerShell
process first:

```powershell
# Set OCTESSERA_PI_PASSPHRASE in this PowerShell process before these commands.
$PiTarget = "pi@<PI_HOST>"
./tools/board-dev/with-board-ssh.ps1 ssh -Board rpi -Target $PiTarget "hostname"
./tools/board-dev/with-board-ssh.ps1 scp -Board rpi -Target $PiTarget ./candidate.bin "${PiTarget}:/tmp/candidate.bin"
./tools/board-dev/with-board-ssh.ps1 ssh-payload -Board rpi -Target $PiTarget ./remote-script.sh
```
