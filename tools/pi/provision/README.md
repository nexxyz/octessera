# Pi provisioning

`../provision-pi.ps1` installs the development Pi OS, boot, network, sudoers,
performance, and splash configuration. It is safe to run again after changing
the tracked files in this directory or the shared Pi image files.

Provision the device before the first fast deployment:

```powershell
./tools/pi/provision-pi.ps1 -Target pi@192.168.0.211 -BoardProfile raspberry-pi-zero-2w
```

Pass `-UpdateInitramfs` when the early boot splash or its boot configuration
needs to be refreshed. The deployment script does not change OS configuration;
it uploads the binary or source, restarts the service, and can tail its logs.
Raspberry Pi provisioning rejects `orange-pi-zero-2w`; Orange Pi uses the
separate Armbian bring-up path.
