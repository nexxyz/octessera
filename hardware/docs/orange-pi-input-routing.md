# Orange Pi UART0 input routing

The Orange image carries the separate
`octessera-h618-input-routing.dts` source and
`octessera-h618-input-routing.dtbo`. It is not combined with the SPI or AHUB
overlays. Against the exact boot-selected
`sun50i-h618-orangepi-zero2w.dtb`, the checked merge must show UART0 disabled,
the PH0/PH1 `gpio_in` pull-up group selected, and `/chosen/stdout-path` cleared
so it no longer targets UART0. The provisioning path removes only exact
`console=ttyS0` boot-argument tokens from Armbian boot configuration, disables
and masks `serial-getty@ttyS0.service`, and does not edit SSH configuration.

The image builder validates and atomically installs this overlay. For an
already-running board, use the no-reboot wrapper only after reviewing its
printed backup record:

```powershell
.\tools\orange-pi\provision-input-routing.ps1 -Preflight
.\tools\orange-pi\provision-input-routing.ps1 -Apply
```

The apply step records the exact base-DTB and overlay hashes, copies of the
previous Armbian environment, extlinux file, input-routing files, and serial
getty state under `/var/lib/octessera/input-routing-backups/<id>/`. It stages
the changes but does not reboot. Rollback is explicit and also does not reboot:

```powershell
.\tools\orange-pi\provision-input-routing.ps1 -RollbackId <backup-id>
```

After either apply or rollback, reboot only through the separately reviewed
operator procedure, then rerun preflight. Until the apply reboot completes,
AUX2 A/B can be requested but its click line remains unavailable to the running
kernel; after the overlay boots, all four Orange encoder switches are enabled.
