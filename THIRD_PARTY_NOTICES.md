# Third-party notices

This is a reviewed notice for known third-party material used or referenced by
Octessera. It is not an exhaustive dependency inventory. Generated Cargo and
pnpm dependency inventories are maintained separately at
`licenses/cargo/inventory.json` and `licenses/pnpm/inventory.json` by the
dependency-inventory lane.

## Vendored CPAL

The vendored CPAL 0.15.3 tree is under Apache License 2.0. Its license and the
Octessera-specific provenance and modification record are kept beside the tree:
[`third_party/cpal-0.15.3/LICENSE`](third_party/cpal-0.15.3/LICENSE) and
[`third_party/cpal-0.15.3/PROVENANCE.md`](third_party/cpal-0.15.3/PROVENANCE.md).
The provenance file identifies the exact modified CPAL files; it is part of
the notice for that vendored tree.

## Operating-system and image sources

Board images and build paths use or reference Linux and the following projects
descriptively: [Armbian](https://github.com/armbian/build),
[Debian](https://www.debian.org/),
[Raspberry Pi OS](https://www.raspberrypi.com/software/operating-systems/),
and [Raspberry Pi's pi-gen](https://github.com/RPi-Distro/pi-gen). Their own
copyright, license, source, and attribution files remain authoritative. Pinned
upstream source references are retained in the project records. Applicable
source duties require review before any future public image release; this notice
does not claim legal compliance.

The image setup payload also uses [balenaOS wifi-connect](https://github.com/balena-os/wifi-connect).
The image-side notice is installed as
`/usr/local/share/doc/octessera/wifi-connect.NOTICE`; it is not replaced by this
repository notice.

## Samples

The 320 repository media files are byte-matched and recorded individually in
[`samples/ATTRIBUTIONS.tsv`](samples/ATTRIBUTIONS.tsv). The exact upstream
[LICENSE](samples/upstream/LICENSE) and [README designation](samples/upstream/README.txt)
snapshots are retained beside the inventory. The upstream project designates the
pack under CC0 1.0 and describes it as attribution-free;
Octessera records that designation but does not independently warrant
third-party rights.

## Hardware

Hardware attribution, product references, CAD provenance, and unresolved exact
file terms are documented in [`hardware/ATTRIBUTIONS.md`](hardware/ATTRIBUTIONS.md).
