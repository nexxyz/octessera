# Raspberry custom-kernel stage

Enable this stage before `stage4-octessera`. The image build must provide
`OCTESSERA_KERNEL_PACKAGE`, the arm64 `.deb` that the kernel builder already
checked. The stage retains stock boot files under `octessera/recovery-stock`
before installing the package, and it installs
`/usr/local/sbin/octessera-finalize-rpi-kernel` without running it.

The initramfs is generated later: `stage4-octessera/03-boot-config` runs
`octessera-finalize-rpi-kernel` after `02-setup-service` has installed the
boot-splash hooks. Running it any earlier would build an initramfs without
them.
