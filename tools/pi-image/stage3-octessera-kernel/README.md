# Raspberry custom-kernel stage

Enable this stage before `stage4-octessera`. The image build must provide:

- `OCTESSERA_KERNEL_PACKAGE`: the already validated exact arm64 `.deb`;
- `OCTESSERA_KERNEL_CHECKSUMS`: its one-entry `SHA256SUMS` file;
- `OCTESSERA_KERNEL_PROVENANCE`: the package provenance JSON; it is copied and
  validated before `dpkg` is allowed to install the package.

The stage rechecks the package filename, Debian control fields, checksum, ARM64
firmware header, DTB, overlays, and required modules. It retains stock boot
files under `octessera/recovery-stock` before installing the package. Stage 4B
generates the custom initramfs after the service stage has installed its hooks.
