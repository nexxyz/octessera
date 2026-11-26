# Orange Pi production image and runtime reference

This is the technical reference for the Orange Pi Zero 2W production Armbian
image, service, storage, audio, and updater contracts. Use the ordered
[Orange bring-up procedure](orange-pi-armbian-bringup.md) for a board session;
use [`docs/workflows/image-construction.md`](../../docs/workflows/image-construction.md)
for image construction commands.

## Artifact contract

The fixed production path is Armbian Debian 13/Trixie for the exact board ID
`orangepizero2w`. A production image uses a version-qualified
`octessera-<version>-orange-pi-zero-2w.img.xz` name with a matching SHA-256
file. Its build metadata in `/etc/octessera/build-metadata.env` records
`OCTESSERA_BOARD_PROFILE_ID=orange-pi-zero-2w` and the runtime hashes, and its
runtime metadata declares `artifact_kind=production-runtime`,
`runtime_ready=true`, and `orange-pi-zero-2w`.

Production stages the exact hash-bound three-file runtime bundle:
`octessera-pi`, `octessera-runtime.json`, and `SHA256SUMS`. There is no
diagnostic image. The bring-up smoke tool is the canonical `orange-oled-smoke`
ELF with its adjacent schema-2 `orange-oled-smoke.metadata.json` sidecar and
lowercase `binary_sha256`; it is cross-built separately and copied to a
production-image board. Keep those names together. `orange-seesaw-smoke` uses
the same sidecar contract and is limited to the reset/HW-ID check on
`/dev/i2c-2`.

## Production service contract

`octessera.service` runs the native runtime as the locked
`octessera-runtime` system account. The separate interactive `octessera` account
is for setup and administration. Persistent data belongs to the runtime account:

- `/var/lib/octessera/presets`
- `/var/lib/octessera/samples`

The service supports the OLED, NeoTrellis, NeoKey, four encoders, persistent
store, samples, MIDI, and the selected exact audio outputs. Jack Audio is the
always-on primary output. USB Audio and HDMI Audio are optional PCM mirrors of
the Jack mix; they do not replace it, and an optional route fault does not stop
Jack. HDMI audio is separate from HDMI video. Simultaneous physical outputs use
independent unsynchronized clocks and can drift or echo; the contract does not
provide sample synchronization.

Readiness follows selected-route state, initialized control-surface devices, and
the first rendered runtime frame. FIFO priority 70 comes from
`LimitRTPRIO=70`; `CAP_SYS_TTY_CONFIG` is the sole ambient and bounding capability
for native VT leasing. The service does not use `CAP_SYS_NICE` or other realtime
capability elevation. The Orange HDMI connector path is
`/sys/class/drm/card0-HDMI-A-1`; HDMI audio is selected independently from HDMI
video.

Normal SIGINT cleanup joins workers after the shutdown frame, black
Trellis/NeoKey frames, and OLED-off operation.

## HDMI plug-event log mitigation

A loose or intermittent HDMI plug can make the fixed H618 controller repeat
`sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin` until Armbian's 50 MB RAM log fills.
Orange images install
`/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf` with root ownership and
mode `0644`. Its exact `$msg` match stops only that rsyslog file-writing copy,
before the default file rules; other HDMI and kernel messages remain visible.
Journald keeps its bounded diagnostic copy.

This prevents duplicate text-log space consumption. Inspect and reseat the
hardware with the board powered down, then use `rsyslogd -N1 -f /etc/rsyslog.conf` and
`journalctl -k --no-pager` for read-only checks.

## Boot configuration writes

`/boot/armbianEnv.txt` is written only at image build. Armbian's
`armbian-hardware-optimize.service` normally runs `add_usb_storage_quirks` on
every boot, which rewrites the file with appends and `sed -i` passes and no
fsync; a power cut in that window could truncate it. `userpatches/customize-image.sh`
writes the BSP's default `usbstoragequirks=0x2537:0x1066:u,0x2537:0x1068:u`
line (Armbian's build does not), then replaces only that one call in
`/usr/lib/armbian/armbian-hardware-optimization` with a no-op. The build fails
if the BSP script no longer has exactly that call and function, and the
built-image smoke check verifies both the line and the disabled call. The rest
of the service still runs. A BSP package upgrade on a board restores the call
until the next Octessera image.

USB host mode is for keyboards and MIDI controllers. The kernel enumerates USB
drives, but Octessera keeps samples and saves on the microSD card, extra samples
on SD2, and mounts nothing else. SD2 is the single FAT32/exFAT partition (or a whole card
formatted that way), whatever its label, on the card behind the SPI SD host from the SPI1 overlay
(`/sys/devices/platform/soc/5011000.spi/spi_master/spi1/spi1.1/mmc_host/`;
the Raspberry uses `3f204000.spi/.../spi0.1`). An empty slot or a card without
such a partition means no SD2, a card with several is refused, and USB drives
are ignored. SD2 is mounted read-only, because nothing on either board writes to
it during play (recordings, saves, presets and backups live on SD1, and restore
never crosses into the mount). It is remounted read-write only for the moment
the helper needs to create a missing `octessera/samples` folder; SD2
transfers write through the USB gadget with the card unmounted.
A pull outside a transfer therefore leaves a clean volume. Before mounting, the
shared SD helper reads the volume's dirty
flag (exFAT VolumeFlags, FAT boot-sector state) and, if it is set, runs
`fsck.exfat -p` or `fsck.vfat -a` with a 60 s timeout (killed 5 s later if it
ignores the stop), logging the result. Only a check that leaves the flag clear
counts as a repair, since exit code 1 also covers read errors. A repair writes
`/run/octessera-sd-card.repaired` (device, disk sequence and a timestamp); the
app opens the `SD card repaired` popup once per new value while the app runs
(after the startup splash if it arrives during it). The marker is written only
once the repaired card has mounted; a repaired card that then fails to mount
gets the needs-repair marker instead. Pulling the card clears both. A
failed or timed-out check leaves SD2 unmounted and writes
`/run/octessera-sd-card.needs-repair`, so the sample browser says the card
needs repair; the marker goes when a card mounts or the slot is empty.
Pulling the card fires the slot's udev `remove` rule, which starts the SD card
service; finding no card, it lazily unmounts the stale SD2 mount and clears the
marker, unless an SD2 transfer owns the card. After an
SD2 transfer, a card the computer left dirty is not checked inside the
transfer's 12 s budget: the transfer starts the SD card service with
`--no-block`, which owns the check. A transfer stopped after the card was pulled restores the
normal USB gadget and leaves SD2 absent. Both images install `dosfstools` and `exfatprogs`.

## Updater contract

Orange Check/Apply/Rollback goes through the root-owned update socket to
`octessera-update`. It takes only this board's pair:

- `octessera-<version>-orange-pi-zero-2w-device-aarch64.zip`
- `SHA256SUMS-orange-pi-zero-2w-device.txt`

Apply verifies the checksum, unpacks the release to
`/opt/octessera/releases/<version>`, installs the Octessera-owned root files the
ZIP carries (units, drop-ins, helper scripts, setup portal; the replaced ones
stay with the old release), and switches `current` with a restart. If the
runtime is not up and ready within 60 seconds, it switches both back. It never
replaces sudoers, the initramfs, the Armbian image, kernel, or device tree;
those need a reflash.

## Power boundary

The runtime performs external MIDI panic and internal audio silence before
sending exactly `reboot\n` or `poweroff\n` to the root-owned
`/run/octessera-device-apply/reboot.sock`. The root-owned device-apply service
validates saved config only for reboot, invokes only the matching fixed
`/usr/bin/systemctl` command, and returns exact `accepted\n` or `rejected\n`
bytes. The same socket accepts exactly `usb-gadget\n`, which validates saved
config, refuses an active SD2 transfer, and restarts only
`octessera-orange-usb-gadget.service`. There is no sudo fallback or command
discovery.

## Setup portal and production inputs

The Orange source tree is `userpatches/overlay`. The
Raspberry source tree is `tools/pi-image/stage4-octessera/files/root`. Setup
changes ship through a full constructor image.

The portal owns first-boot setup: anyone who joins the setup hotspot before
completion can configure it, while the production constructor removes
`/root/.not_logged_in_yet` so Armbian's interactive vendor wizard does not
compete with it. `armbian-firstrun.service` remains enabled with
`OPENSSHD_REGENERATE_HOST_KEYS=true`, and `armbian-resize-filesystem.service`
remains enabled for first-boot filesystem growth. Octessera adds no shared SSH
password or baked SSH key. Fresh images mask the profile's SSH units until the
portal finalizes SSH; host keys are generated on-device only.

The portal's SSH choices are explicit. Key mode installs the selected key and
keeps password authentication off. Password mode sets the selected
`octessera` password and enables password authentication. None removes the key,
locks the account, and leaves SSH masked.

The production image boots offline without waiting for a network. `NetworkManager`
remains installed and available, while `dnsmasq.service`,
`systemd-networkd-wait-online.service`, and `NetworkManager-wait-online.service`
are disabled. The setup service is disabled, and only
`octessera-setup-request.path` is enabled. Networking and SSH are deliberate
opt-in actions from `System > Connections > Wi-Fi > Setup Portal > Open Portal`; the image does not
start a hotspot or SSH automatically.

The production image's SPI1 OLED+SD2 overlay is board-specific:

- Source: `userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts`.
- Installed source: `/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts`.
- Installed DTBO: `/boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo`.
- Boot enablement: `user_overlays=octessera-h618-spi1-oled-sd2` in `/boot/armbianEnv.txt`.
- Required I2C enablement: `overlays=i2c1-pi` in `/boot/armbianEnv.txt`.

It enables SPI1 data plus CS0 and CS1 on the reviewed H618 pin groups, creates
one `rohm,dh2228fv` OLED device capped at 16 MHz, and creates an
`mmc-spi-slot` SD2 device on CS1 capped at 10 MHz. Its card detect is the
OLED module's CD line on header pin 38 (H618 PI4, pull-up, high when a card is
present), set through the binding's legacy `gpios` property. Without it the
kernel polls the empty slot over SPI1 many times a second, which floods the
journal on the debug-enabled Armbian kernel. It does not add GPIO chip select,
broken-card-detect, or non-removable properties. Image customization resolves the
boot-selected DTB; DTBO and boot-environment writes are atomic. Before any OLED transfer, verify the live SPI1 node and pinmux and keep
a recovery path for `/boot/armbianEnv.txt`.

On the fixed PCB, SD2 chip select is header pin 26. H618 PH9 is the local
SPI1 CS1 pin and uses mux `0x4`; the OLED remains SPI1 CS0. Verify the physical
coexistence of the OLED and microSD wiring with the live kernel and electrical
setup before use.

The production DAC is owned by the Octessera AHUB audio overlay and is enabled
only by the mandatory `octessera_audio` Armbian extension. The boot composition
is the selected H618 DTB, stock `sun50i-h616-i2c1-pi.dtbo`, SPI1, input-routing,
then `octessera-ahub0-pcm5102.dtbo`. The audio overlay uses APB0/DMA3/TDM0,
PI1/PI2 `i2s0`, and PI3 `i2s0_dout0` to expose the playback-only
`octessera-dac` card. The exact ALSA card identity is `octesseradac`, with the
playback route `hw:CARD=octesseradac,DEV=0`; the fixed image does not depend on a
`CONFIG_SND_SOC_PCM5102A` driver or a PCM5102A codec node/link. The overlay
uses the vendor dummy-codec topology and does not claim MCLK.

SD2 requires `CONFIG_MMC=y` and `CONFIG_MMC_BLOCK=y`. `CONFIG_MMC_SPI` may be
built in or modular: a modular build installs `mmc_spi` through
`/etc/modules-load.d/octessera-orange-sd-card.conf`; a built-in build does not
install a module-load entry or an `mmc_spi` module.

The image stages the complete 320-file sample library. The musical-default
provisioner seeds `system.json` and `default.patch.json` when both are missing,
and validates an existing pair without replacing it. It refuses a legacy mixed
`default.json` or an incomplete pair. Boot does not copy or replace sample
media. The build preserves the mixed `pi-default.json` as inert image metadata;
it is not an active preset. The technical manifest records each sample file's
path, size, and SHA-256.

Stage and inspect those assets before an Armbian build:

```sh
bash tools/armbian-image/stage-musical-assets.sh
bash tools/armbian-image/test-musical-assets.sh
```

After boot, useful setup checks are:

```sh
systemctl status octessera-setup.service
journalctl -u octessera-setup.service --no-pager
systemctl is-enabled ssh.service || true
ls /etc/ssh/ssh_host_* 2>/dev/null || true
cat /etc/octessera/build-metadata.env
```

## Kernel and audio checks

The `octessera_midi` extension requests only `CONFIG_SND_SEQUENCER=m`,
`CONFIG_SND_RAWMIDI=m`, and `CONFIG_SND_USB_AUDIO=m`, and forces
`# CONFIG_RT_GROUP_SCHED is not set`. Keep cgroup v2,
`kernel.sched_rt_period_us=1000000`, and `kernel.sched_rt_runtime_us=950000`.
The installed module-load file contains only `snd_seq` and `snd_seq_midi`.

On the installed image:

```sh
uname -r
modinfo snd_seq
modinfo snd_seq_midi
modinfo snd_usb_audio
test -c /dev/snd/seq
aconnect -l
```

Run these checks against the matching kernel package and Armbian image. The live
checks reject an enabled or modular RT group scheduler and verify the global
throttle and cgroup mode. A FIFO callback result does not replace those kernel
assertions.

The exact live kernel assertions are:

```sh
kernel_config=/boot/config-$(uname -r)
grep -qxF '# CONFIG_RT_GROUP_SCHED is not set' "$kernel_config"
! grep -qE '^CONFIG_RT_GROUP_SCHED=' "$kernel_config"
test "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs
test "$(sysctl -n kernel.sched_rt_period_us)" = 1000000
test "$(sysctl -n kernel.sched_rt_runtime_us)" = 950000
```

For a foreground callback, leave the default priority unset and inspect
the running process from a second shell:

```sh
unset OCTESSERA_AUDIO_THREAD_PRIORITY
/tmp/octessera-pi 2>&1 | tee /tmp/octessera-fifo.log
pid=$(pgrep -xo octessera-pi)
ps -L -p "$pid" -o pid,tid,cls,rtprio,comm
ps -L -p "$pid" -o cls=,rtprio= | awk '$1 == "FF" && $2 == 70 { found = 1 } END { exit found ? 0 : 1 }'
! grep -q 'audio callback RT promotion not qualified' /tmp/octessera-fifo.log
```

## USB identity boundary

The Orange combined configfs service accepts only the verified UDC
`musb-hdrc.4.auto`; absence, a different controller, a bound controller, or an
existing configfs gadget fails closed. It creates only UAC2 and MIDI functions,
binds the UDC last, and exposes no mass storage during normal operation.
SD2 transfer is a separate fixed root-owned storage-control action using the
same UDC and lifecycle lock. It unmounts the slot-bound SD2 card
before binding a writable/removable mass-storage LUN and restores the normal
UAC2/MIDI gadget after host eject and stop. The fake-configfs composer follows
the same lifecycle. Teardown unbinds first, removes configuration links and functions,
then removes the gadget tree.

The installed service can be inspected without binding a new gadget:

```sh
systemctl status octessera-orange-usb-gadget.service
cat /sys/class/udc/musb-hdrc.4.auto/function
ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
```

Every host-visible name is `Octessera (Orange <suffix>)` in all three modes:
the product string, configuration string, UAC2 `function_name`, and MIDI
`interface_string`. `<suffix>` is the same last four lowercase hex digits of
the `wlan0` MAC that the `Octessera Setup <suffix>` hotspot uses, so the
computer and the OLED name the same board. Without a valid `wlan0` MAC the
suffix falls back to the last four hex digits of `/etc/machine-id`, then to
`0000`. The board-local f_midi `id`, which becomes the ALSA card id, stays
`Octessera MIDI`, and the serial number stays `octessera-orange-pi`. Setup and
teardown share one exclusive lifecycle lock; a concurrent operation fails before
changing the gadget.

The standalone composer uses the same contract for fake-configfs tests:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

MIDI and combined modes require the patched production kernel's writable
`interface_string`. The composer writes the board name without a trailing LF, verifies byte-for-byte readback, and only then creates
the MIDI link and binds the UDC. Missing, write, readback, and bind failures
roll back the partial gadget. The Linux Foundation VID/PID values are
local-validation values and defaults remain disabled. The legacy Windows MEDIA
`FriendlyName` may remain `MIDI function`.
