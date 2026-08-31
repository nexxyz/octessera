# Orange Pi production image and runtime reference

This is the technical reference for the Orange Pi Zero 2W production Armbian
image, service, storage, audio, and updater contracts. Use the ordered
[Orange bring-up procedure](orange-pi-armbian-bringup.md) for a board session;
use [`docs/workflows/image-construction-and-proof.md`](../../docs/workflows/image-construction-and-proof.md)
for construction and proof commands.

## Artifact and image-mode contract

The fixed production path is Armbian Debian 13/Trixie for the exact board ID
`orangepizero2w`. The documented immutable v0.7.5 production artifact is
`octessera-0.7.5-orange-pi-zero-2w.img.xz`, with a matching SHA-256 and
provenance set. Its build metadata contains `OCTESSERA_IMAGE_MODE=production`
and its runtime metadata declares `artifact_kind=production-runtime`,
`runtime_ready=true`, and `orange-pi-zero-2w`.
This is the retained v0.7.5 artifact contract; v0.8.1 constructor evidence is
source-bound and still awaits physical FAT for its exact release artifact.

Production stages the exact hash-bound three-file runtime bundle:
`octessera-pi`, `octessera-runtime.json`, and `SHA256SUMS`. The separate
diagnostic artifact is the canonical `orange-oled-smoke` ELF with its adjacent
schema-2 `orange-oled-smoke.metadata.json` sidecar and lowercase
`binary_sha256`. Keep those names together. `orange-seesaw-smoke` uses the same
sidecar contract and is limited to the proven reset/HW-ID check on `/dev/i2c-2`.

Diagnostic mode is explicit as `OCTESSERA_IMAGE_MODE=diagnostic`; it has no
production runtime bundle and does not contain or enable `octessera.service`.
The shared action's `image_kind=diagnostic` is for bring-up; `production`
requires the hash-bound runtime bundle. Never infer mode from a local binary or
payload; inspect `/etc/octessera/build-metadata.env`.

## Production service contract

`octessera.service` runs the native runtime as the locked
`octessera-runtime` system account. The separate interactive `octessera` account
is for setup and administration. Persistent data belongs to the runtime account:

- `/var/lib/octessera/presets`
- `/var/lib/octessera/samples`

The service supports the OLED, NeoTrellis, NeoKey, four encoders, persistent
store, samples, MIDI, and the selected exact audio outputs. Every non-empty
Jack/USB/HDMI output set is valid. Jack is required only when selected;
recognized disconnected UAC2 and HDMI routes may wait and recover; selected
route faults block readiness; and no route is a fallback. Simultaneous physical
outputs use independent unsynchronized clocks and can drift or echo. The
contract does not provide sample alignment.

Readiness follows selected-route status, initialized control-surface devices, and
the first rendered runtime frame. FIFO priority 70 comes from
`LimitRTPRIO=70`; `CAP_SYS_TTY_CONFIG` is the sole ambient and bounding capability
for native VT leasing. The service does not use `CAP_SYS_NICE` or other realtime
capability elevation. The observed Orange HDMI connector path
is `/sys/class/drm/card0-HDMI-A-1`. A separate live Raspberry observation found
the same card0 status/EDID paths on kernel `6.12.93+rpt-rpi-v8`; Raspberry pins
card0 and does not fall back to card1. These are connector identity observations,
not connected HDMI audio or audible qualification.

Normal SIGINT cleanup joins workers after the shutdown frame, black
Trellis/NeoKey frames, and OLED-off operation.

## HDMI plug-event log mitigation

A loose or intermittent HDMI plug was proven to make the fixed H618 controller
repeat `sun8i-dw-hdmi 6000000.hdmi: EVENT=plugin` until Armbian's 50 MB RAM log
filled. Newly constructed Orange images install
`/etc/rsyslog.d/00-octessera-orange-hdmi-plugin.conf` with root ownership and
mode `0644`. Its exact `$msg` match stops only that rsyslog file-writing copy,
before the default file rules; other HDMI and kernel messages remain visible.
Journald keeps its bounded diagnostic copy.

This prevents duplicate text-log space consumption. It does not repair a loose
connector, cable, or physical HDMI fault. Inspect and reseat the hardware with
the board powered down, then use `rsyslogd -N1 -f /etc/rsyslog.conf` and
`journalctl -k --no-pager` for read-only checks.

## Runtime-only updater contract

Orange Check/Apply/Rollback goes through the root-owned broker and guarded
updater. It accepts only the profile-qualified pair:

- `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip`
- `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`

The updater validates the board profile, manifest, checksum, and candidate
health, then updates only the managed runtime release and binary link. It does
not replace the Armbian image, kernel, device tree, or other full-image assets.
Full image replacement remains manual, and the standalone manual runtime ZIP is
a manual bundle rather than an OTA asset. Orange never consumes Raspberry assets
or falls back to a manual ZIP or full-image path. Missing or mismatched profile,
asset, manifest, checksum, or health precondition fails closed.

## Power boundary

The runtime performs external MIDI panic and internal audio silence before
sending exactly `reboot\n` or `poweroff\n` to the root-owned
`/run/octessera-device-apply/reboot.sock`. The root-owned device-apply service
validates saved config only for reboot, invokes only the matching fixed
`/usr/bin/systemctl` command, and returns exact `accepted\n` or `rejected\n`
bytes. There is no sudo fallback, command discovery, or live hardware
qualification claim in this source/bring-up contract.

## Setup portal and production inputs

The Orange source tree is `userpatches/overlay`; its exact setup assets,
preconditions, paths, digests, modes, preimages, stale markers, and enabled-unit
differences are bound by
`resources/image-mutations/orange-pi-zero-2w-setup.json`. The Raspberry source
tree is `tools/pi-image/stage4-octessera/files/root`, bound by the matching
Raspberry contract. Setup mutation and runtime-only contracts are separate;
setup is opt-in, while runtime-only is the default.

The setup constructor requires the parent to already contain
`openssh-server`, `network-manager`, `dnsmasq`, `python3-minimal`,
`/usr/local/bin/wifi-connect`, `/usr/bin/python3`, and their service units. It
does not create missing package, account, executable, service, preimage,
ownership, mode, or xattr data. Orange requires
`octessera:octessera` at `/home/octessera` with `/bin/bash` and
`octessera-runtime:octessera-runtime` at `/nonexistent` with `/usr/sbin/nologin`.

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
opt-in actions from `System > Configure WiFi > Open Portal`; the image does not
start a hotspot or SSH automatically.

The production image's SPI1 OLED+SD2 overlay is board-specific:

- Source: `userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts`.
- Installed source: `/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts`.
- Installed DTBO: `/boot/overlay-user/octessera-h618-spi1-oled-sd2.dtbo`.
- Boot enablement: `user_overlays=octessera-h618-spi1-oled-sd2` in `/boot/armbianEnv.txt`.
- Required I2C enablement: `overlays=i2c1-pi` in `/boot/armbianEnv.txt`.

It enables SPI1 data plus CS0 and CS1 on the reviewed H618 pin groups, creates
one `rohm,dh2228fv` OLED device capped at 16 MHz, and creates an
`mmc-spi-slot` SD2 device on CS1 capped at 10 MHz. It does not add GPIO chip
select, card-detect, broken-card-detect, or non-removable properties. Image customization resolves the
boot-selected DTB and records non-secret DTS/DTBO hashes in
`/etc/octessera/build-metadata.env`; DTBO and boot-environment writes are
atomic. Before any OLED transfer, prove the live SPI1 node and pinmux and keep
a recovery path for `/boot/armbianEnv.txt`.

On the fixed PCB, SD2 chip select is header pin 26. H618 PH9 is the local
SPI1 CS1 pin and uses mux `0x4`; the OLED remains SPI1 CS0. Physical
coexistence of the OLED and microSD wiring is intentionally unqualified in
this source/image stage and still needs electrical and live-kernel proof.

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

The image stages the complete 320-file sample library and only seeds the default
preset when `/var/lib/octessera/presets/default.json` is absent. Boot does not
copy or replace sample media. The technical manifest records each file's path,
size, and SHA-256.

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
sudo octessera-armbian-diagnostics
cat /etc/octessera/build-metadata.env
```

## Kernel and audio proof boundary

The `octessera_midi` extension requests only `CONFIG_SND_SEQUENCER=m`,
`CONFIG_SND_RAWMIDI=m`, and `CONFIG_SND_USB_AUDIO=m`, and forces
`# CONFIG_RT_GROUP_SCHED is not set`. Keep cgroup v2,
`kernel.sched_rt_period_us=1000000`, and `kernel.sched_rt_runtime_us=950000`.
The installed module-load file contains only `snd_seq` and `snd_seq_midi`.

After deploying the newly generated image and rebooting once:

```sh
uname -r
modinfo snd_seq
modinfo snd_seq_midi
modinfo snd_usb_audio
test -c /dev/snd/seq
aconnect -l
```

The matching kernel package and Armbian image must be newly generated; do not
qualify MIDI from a runtime rebuild or `modprobe` on an old image. The live gate
must reject an enabled or modular RT group scheduler and verify the global
throttle and cgroup mode. A FIFO callback result is a live image/process check,
not a replacement for those kernel assertions.

The exact live kernel assertions are:

```sh
kernel_config=/boot/config-$(uname -r)
grep -qxF '# CONFIG_RT_GROUP_SCHED is not set' "$kernel_config"
! grep -qE '^CONFIG_RT_GROUP_SCHED=' "$kernel_config"
test "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs
test "$(sysctl -n kernel.sched_rt_period_us)" = 1000000
test "$(sysctl -n kernel.sched_rt_runtime_us)" = 950000
```

For a foreground callback check, leave the default priority unset and inspect
the running process from a second shell:

```sh
unset OCTESSERA_AUDIO_THREAD_PRIORITY
/tmp/octessera-pi 2>&1 | tee /tmp/octessera-fifo.log
pid=$(pgrep -xo octessera-pi)
ps -L -p "$pid" -o pid,tid,cls,rtprio,comm
ps -L -p "$pid" -o cls=,rtprio= | awk '$1 == "FF" && $2 == 70 { found = 1 } END { exit found ? 0 : 1 }'
! grep -q 'audio callback RT promotion not qualified' /tmp/octessera-fifo.log
```

Stop if `CONFIG_RT_GROUP_SCHED` is enabled, either throttle value changes,
cgroup v2 is absent, FIFO 70 is not reached, or the runtime reports an
unqualified DAC/UAC2 callback promotion.

## USB identity boundary

The Orange combined configfs service accepts only the verified UDC
`musb-hdrc.4.auto`; absence, a different controller, a bound controller, or an
existing configfs gadget fails closed. It creates only UAC2 and MIDI functions,
binds the UDC last, and exposes no mass storage during normal operation.
SD2 transfer is a separate fixed root-owned storage-control action using the
same UDC and lifecycle lock. It unmounts the label-safe `OCTESSERA_SD` card
before binding a writable/removable mass-storage LUN and restores the normal
UAC2/MIDI gadget after host eject and stop. Source and fake-configfs contracts
are present. Teardown unbinds first, removes configuration links and functions,
then removes the gadget tree.

The installed service can be inspected without binding a new gadget:

```sh
systemctl status octessera-orange-usb-gadget.service
cat /sys/class/udc/musb-hdrc.4.auto/function
ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
```

Its product strings are `Octessera Audio + MIDI` for `combined`,
`Octessera MIDI` for `midi`, and `Octessera Line In` for `uac2`. Setup and
teardown share one exclusive lifecycle lock; a concurrent operation fails before
changing the gadget.

The standalone composer uses the same contract for fake-configfs qualification:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

MIDI and combined modes require the patched qualified kernel's writable
`interface_string`. The composer writes exactly 14 bytes of `Octessera MIDI`
without a trailing LF, verifies byte-for-byte readback, and only then creates
the MIDI link and binds the UDC. Missing, write, readback, and bind failures
roll back the partial gadget. The Linux Foundation VID/PID values are for local
validation only, not a public USB identity; defaults remain disabled. The legacy
Windows MEDIA `FriendlyName` may remain `MIDI function` and is diagnostic-only,
not an acceptance field.

Qualification status: The non-final installed board's live DT reports
`usb@5100000/dr_mode=peripheral` and its USB0/controller 0 path uses the fixed
UDC `musb-hdrc.4.auto`. It passed high-speed combined UAC2+MIDI, 44.1 kHz
stereo board-to-Windows capture with a board-generated 1 kHz tone on both
channels, exact bidirectional MIDI traffic, and exact ConfigFS
`interface_string`, actual MIDI interface descriptor, and Windows
`DEVPKEY_Device_BusReportedDeviceDesc` identity `Octessera MIDI`. Windows names
the UAC2 endpoint `Octessera Audio`, not the combined composite product
`Octessera Audio + MIDI`. Repeat on the exact final v0.8.2 constructor image and
complete physical connector naming, VBUS/CC/no-backfeed electrical, physical
reconnect and host suspend/resume, SD2 mass-storage start/eject/stop recovery,
and authorized public VID/PID gates before claiming public USB support or
closing qualification.
