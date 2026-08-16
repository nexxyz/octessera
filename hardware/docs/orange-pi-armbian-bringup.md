# Orange Pi Zero 2W Armbian bring-up

This document is the current Orange Pi Zero 2W bring-up and qualification
procedure for the established Armbian production path. The production artifact
is an explicit `production` image mode; the separate `diagnostic` image mode
remains useful for bus, OLED, and kernel checks. The historical diagnostic
qualification record is not the production service contract and is kept in
[`orange-pi-selection-and-qualification-history.md`](orange-pi-selection-and-qualification-history.md).

Foreground runtime qualification in this procedure uses the production runtime;
the historical `runtime-candidate` procedure is retained only in the linked
selection history.

This is a hardware gate. Do not copy Raspberry Pi constants, overlays, or `rppal` GPIO assumptions into Orange Pi support until these checks pass on the target board and image.

## Target context

- Board: Orange Pi Zero 2W, 2 GB RAM.
- Production image path: Armbian Debian 13/Trixie for Orange Pi Zero 2W.
- Wiring goal: same Octessera PCB and harness as the Raspberry Pi Zero 2 W build.

Record the image URL, image date, kernel version, board name, and all command output during bring-up.

The 0.7.5 production image artifact is
`octessera-0.7.5-orange-pi-zero-2w.img.xz`, with a matching SHA-256 and image
provenance set. Its build metadata must contain
`OCTESSERA_IMAGE_MODE=production`. The image stages the exact, hash-bound
runtime bundle `octessera-pi`, `octessera-runtime.json`, and `SHA256SUMS`; its
runtime metadata declares `artifact_kind=production-runtime` and
`runtime_ready=true` for `orange-pi-zero-2w`.

The diagnostic artifact is the canonical `orange-oled-smoke` ELF. Its adjacent
`orange-oled-smoke.metadata.json` sidecar uses schema 2, contains the exact
identity field set, and binds the copied ELF with a canonical lowercase
`binary_sha256`. Keep those two files together; do not rename either one.
The separate `orange-seesaw-smoke` artifact uses the same sidecar contract and
is limited to the proven Seesaw reset/HW-ID check on `/dev/i2c-2`.

Diagnostic image mode is explicit as `OCTESSERA_IMAGE_MODE=diagnostic`; it has no
production runtime bundle or `octessera.service`. The production runtime
supports the OLED, NeoTrellis, NeoKey, four encoders, persistent store, samples,
and the shared 44.1 kHz audio contract. Jack, USB Audio, and HDMI Audio
are independent desired-next-boot outputs. The selected Jack route is exactly
`hw:CARD=octesseradac,DEV=0`; selected UAC2 and HDMI routes wait for their exact
endpoints and recover when they return. USB Audio and USB MIDI remain
experimental/local bench validation, not public first-release support. The
current Linux Foundation VID/PID values are local-validation-only and are not a
public product identity. MIDI uses the native host adapter when the configured
gadget port is present, but capability and fake-configfs checks do not qualify
the exact image or board.

`octessera.service` runs the native runtime as the locked `octessera-runtime`
system account. The separate interactive `octessera` account is for setup and
administration. Readiness follows selected-route status, initialized
control-surface devices, and the first rendered runtime frame. The service
gets FIFO priority 70 through `LimitRTPRIO=70`; it does not use `CAP_SYS_NICE`,
ambient capabilities, or other realtime capability elevation.

Orange update check, apply, rollback, and OTA remain unsupported.
Use a verified production image artifact for an image update. The historical
foreground `runtime-candidate` procedure is retained in the selection history
only as bring-up history; it is not the production artifact or service path.

The production service uses `/var/lib/octessera/presets` for its persistent
store and `/var/lib/octessera/samples` for samples. Both paths belong to
`octessera-runtime`; the interactive `octessera` account is separate.

The Orange power boundary is intentionally narrow. The runtime performs
external MIDI panic and internal audio silence before sending exactly
`reboot\n` or `poweroff\n` to the root-owned
`/run/octessera-device-apply/reboot.sock`. The root helper validates the saved
config only for reboot, invokes only the matching fixed `/usr/bin/systemctl`
command, and returns exact `accepted\n` or `rejected\n` bytes. There is no
sudo fallback, command discovery, or live hardware qualification claim in this
source/bring-up contract.

## Full setup portal proof status

The software/static setup layer is present for both fixed board paths. The
Raspberry source is `tools/pi-image/stage4-octessera/files/root`; the Orange
source is `userpatches/overlay`. Their exact setup assets and preconditions are
bound by `resources/image-mutations/raspberry-pi-zero-2w-setup.json` and
`resources/image-mutations/orange-pi-zero-2w-setup.json`. Static coverage
includes shell syntax, Python compilation, systemd unit checks, strict request
and receipt handling, HTTP framing and nonce checks, status concurrency, no
secret material, and source-bound mutation contracts. The main local gate is:

```bash
bash tools/armbian-image/validate.sh
```

Targeted setup checks are listed in
[`docs/development-workflows.md`](../../docs/development-workflows.md). These
checks do not prove a board-created AP, network join, captive page, SSH, or
credential application. The exact v0.7.5 trusted-parent exercise and physical
qualification remain deferred; no production parent image is claimed as
exercised here.

## Setup portal checks still required on hardware

Run the complete flow on both the Raspberry Pi Zero 2 W and Orange Pi Zero 2W:

- create and join the setup AP, then load the captive page;
- apply Wi-Fi credentials, hostname, SSH mode, and login settings successfully;
- reconnect over the newly configured network;
- attach to setup when the setup service is already running;
- observe the 30-minute timeout and portal closure;
- verify failure and partial-state messages without powering off during apply;
- inspect AP traffic, HTTP responses, status/receipt files, logs, and artifacts
  for secret leakage.

Once the board is reachable over SSH, run the repo probe from Windows:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 -Target orangepi@192.168.x.x
```

The default probe is read-only, but its qualification-critical target-device
owner proof requires passwordless `sudo -n` or a root SSH session. Add
`-WithSudoChecks` only after SSH/recovery is stable. The probe and wrapper never
bind a gadget; use the separate composer below for an explicitly authorized USB
test.

`-WithSudoChecks` requires passwordless `sudo -n` or a root SSH session. If the board asks for a sudo password, run the default probe first, then either configure temporary passwordless sudo for bring-up or run the probe as root. The explicit-UDC composer is a separate command and is never invoked by this wrapper.

## Safety gates before connecting the Octessera PCB

Start bare-board. Do not connect the Octessera PCB or harness until these checks pass:

- Compare the Orange Pi Zero 2W schematic/header pinout against the Raspberry Pi Zero 2 W wiring used by Octessera.
- Confirm 5 V, 3.3 V, and GND pins land where the PCB expects them.
- Confirm all connected GPIOs are 3.3 V logic and tolerate the existing pullups/pulldowns.
- Confirm the physical pins used for I2C, SPI, I2S, encoder/button lines, OLED reset/DC/CS, and interrupt lines can expose those functions on Armbian.
- Confirm power input and USB host/device wiring cannot back-power the board or brown out during gadget binding.
- Confirm a recovery path before editing boot overlays: UART serial console, known-good SSH path, or a reflashing workflow that does not depend on the gadget port.

If any pin or power check fails, stop. The no-PCB-change assumption is not valid for that board/image combination.

## Preliminary header desk comparison

Primary references:

- Official Orange Pi Zero 2W product page: <http://www.orangepi.org/html/hardWare/computerAndMicrocontrollers/details/Orange-Pi-Zero-2W.html>
- Official Orange Pi Zero 2W H618 user manual v1.1: <https://orangepi.net/wp-content/uploads/2023/10/OrangePi_Zero2w_H618_User-Manual_v1.1.pdf>
- Clear third-party pinout table: <https://git.munts.com/muntsos/doc/OrangePiZero2WPinout.pdf>

Use this as a desk check only. Trust physical pin numbers first, then verify against the board revision, schematic, Armbian device tree, and live pinmux state.

- Power rail: the third-party pinout shows matching 5 V, 3.3 V, and ground positions. Confirm with a multimeter before connecting the PCB.
- I2C: physical pins 3 and 5 map to I2C1 SDA/SCL in the third-party pinout. Confirm Armbian exposes that bus on those pins.
- OLED SPI: physical pins 19, 21, 23, and 24 map to the reviewed SPI1 data/CS0 path; physical pin 26 is SPI1 CS1 and remains unused. Confirm `/dev/spidev1.0` and pinmux before OLED testing.
- OLED D/C and reset: physical pins 16 and 36 appear GPIO-capable, but not display-specific. Confirm gpiochip lines and polarity.
- DAC I2S: physical pins 12, 35, and 40 are not proven as Pi-style I2S/PCM pins in the official docs found so far. This is blocked until schematic, DTS, and Armbian overlay checks prove I2S there.
- Encoder and button GPIOs use the existing Octessera netlist routes and the H618 40-pin mapping below. The gpiocdev offsets are relative to `300b000.pinctrl`; do not use BCM numbering.
- NeoTrellis interrupt: physical pin 10 is UART0 RX in the third-party pinout. The approved input-routing overlay disables UART0 and releases both PH0/PH1; stop if the live DTB cannot prove that path.
- SW3 switch: physical pin 8 is UART0 TX in the third-party pinout. Its GPIO line is explicitly unavailable until the checked input-routing overlay has booted.

### Direct encoder mapping

The PCB netlist supplies the A/B/switch physical header pins. H618 port offsets
use the established `port base + pin` mapping (`PC12 = 76`, `PI14 = 270`).

| Encoder | A physical / H618 / offset | B physical / H618 / offset | Switch physical / H618 / offset | Candidate status |
|---|---|---|---|---|
| SW1 main | 29 / PI0 / 256 | 31 / PI15 / 271 | 32 / PI11 / 267 | implemented; hardware qualification pending |
| SW2 aux1 | 33 / PI12 / 268 | 22 / PI6 / 262 | 11 / PH2 / 226 | implemented; hardware qualification pending |
| SW3 aux2 | 13 / PH3 / 227 | 7 / PI13 / 269 | 8 / PH0 / 224 | A/B implemented; switch unavailable until UART0-disabled routing is qualified |
| SW4 aux3 | 37 / PI16 / 272 | 18 / PH4 / 228 | 15 / PI5 / 261 | implemented; hardware qualification pending |

Implemented lines are requested with gpiocdev v2 pull-ups and both-edge
quadrature/switch detection; live qualification remains pending. AUX2 A/B are
requested even while UART0 is active;
its switch request is omitted in that profile and included after input-routing
boot. The switch request retains the existing 45 ms debounce contract. The
Orange event boundary reverses all four literal board A/B directions; the
Raspberry path remains rppal-based.

- USB gadget/data: official Orange Pi docs describe two USB-C USB2.0 ports and say both can power the board. They do not prove a Pi-style dedicated OTG/data port. This is blocked until port role, VBUS/CC/ID, and UDC behavior are proven on hardware.
Current desk result: power, I2C, SPI, and the static encoder mapping are
plausible; the native encoder implementation remains unqualified on live
hardware. I2S and USB gadget mode remain the highest-risk hardware gates.

## Armbian differences from Raspberry Pi OS
Armbian does not use the Raspberry Pi firmware overlay path.

| Area | Raspberry Pi path | Armbian / Orange Pi path |
| --- | --- | --- |
| Boot overlay config | `/boot/config.txt` | `/boot/armbianEnv.txt` |
| Overlay loader | Raspberry Pi firmware | U-Boot |
| Kernel overlays | Raspberry Pi `dtoverlay=` names | SoC/board-specific overlays under `/boot/dtb/.../overlay/` |
| User overlays | Usually not needed for current Pi image | `/boot/overlay-user/`, enabled with `user_overlays=` |
| USB device controller | Pi `dwc2` path | Board/kernel-specific UDC; must be detected on hardware |
| GPIO userspace | `rppal` / BCM numbering | Prefer libgpiod gpiochip/line mapping |
Practical rule: Raspberry Pi overlay names and BCM GPIO numbers are not portable contracts.

## Custom SPI1/CS0 overlay
The Octessera Armbian image carries one board-specific user overlay for the Orange Pi Zero 2W:

- Source: `userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts`.
- Installed source: `/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts`.
- Installed DTBO: `/boot/overlay-user/octessera-h618-spi1-cs0.dtbo`.
- Boot enablement: `user_overlays=octessera-h618-spi1-cs0` in `/boot/armbianEnv.txt`.
- Required I2C enablement: `overlays=i2c1-pi` in `/boot/armbianEnv.txt`; existing overlay settings are preserved and the token is added if absent.
The overlay declares one address cell and zero size cells, enables `&spi1` with `&spi1_pins` and `&spi1_cs0_pin`, then creates one CS0 `rohm,dh2228fv` device capped at 16 MHz. The reviewed H618 pin groups are PH6/PH7/PH8 with function `spi1` and PH5 with function `spi1`. It does not touch SPI0, CS1, GPIO lines, services, or authorization. It is not the stock `spidev1_0` overlay. The OLED HAL defaults to 16 MHz and retains the validated 1/2/4/8/12/16 MHz override ladder.
Image customization refuses any board other than the exact Armbian board ID `orangepizero2w`. It resolves the boot-selected `fdtfile`/current DTB path from the image boot configuration and refuses missing or ambiguous H618 candidates; it does not infer the target from `uname -r`. It compiles with symbols, decompiles and merges with fatal diagnostics, and asserts unchanged SPI0/I2C nodes plus the expected SPI1 pinctrl/CS0 result. DTBO and `armbianEnv.txt` writes are atomic. Non-secret source and DTBO SHA-256 values are recorded in `/etc/octessera/build-metadata.env` as `OCTESSERA_SPI1_CS0_DTS_SHA256`, `OCTESSERA_SPI1_CS0_DTBO_SHA256`, `OCTESSERA_INPUT_ROUTING_DTS_SHA256`, and `OCTESSERA_INPUT_ROUTING_DTBO_SHA256`.

The boot-environment parser rejects duplicate assignments, duplicate tokens, commented or inline-commented assignments, and malformed token lists. Full qualification image builds also require a reviewed immutable 40-character Armbian commit SHA; validation-only runs may use the workflow's default ref.

Do not enable this overlay on another board or kernel without a new device-tree review. Before any OLED transfer, prove the live SPI1 node and pinmux mapping on the target and keep a recovery path for `/boot/armbianEnv.txt`.

The separate UART0 input-routing overlay and its no-reboot provisioning,
backup, rollback, and preflight procedure are documented in
`hardware/docs/orange-pi-input-routing.md`.

## GitHub-built Armbian image

The shared `build-armbian-image` action accepts an explicit `image_kind`:
`diagnostic` builds the bring-up image with setup helpers, board-specific USB
audio/MIDI, musical assets, OLED qualification services, and bus diagnostics;
`production` requires the hash-bound Orange runtime bundle and installs the
native runtime service. The generic `Armbian Image` workflow uses diagnostic
mode; the 0.7.5 release image uses `image_kind=production`. Do not infer image
mode from a payload or from the presence of a local binary; inspect
`OCTESSERA_IMAGE_MODE` in `/etc/octessera/build-metadata.env`.

Start with validation only:

```bash
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=xz \
  -f 'extensions=preset-firstrun octessera_midi octessera_image_sanitize' \
  -f run_build=false \
  -f artifact_mode=public-generic \
  -f 'public_inputs={"public_preset_configuration_url":"https://example.invalid/preset.conf"}'
```

Run a no-secret full build by changing `run_build=true` and setting `armbian_build_ref` to a reviewed full 40-character Armbian commit SHA; the mutable default ref is validation-only. Public generic artifacts must not contain Wi-Fi credentials, user passwords, user-specific SSH keys, or private first-run URLs added by Octessera inputs or overlays. If you need first-boot personalization, use the private artifact mode with the protected `armbian-image-personalized` GitHub environment and repository/environment secrets; do not pass secrets as workflow inputs.

The public first-run and diagnostic payload values use one bounded JSON object,
for example `{"public_preset_configuration_url":"https://example.invalid/preset.conf","payload_url":"https://example.invalid/payload.tar","payload_sha256":"<64-hex-sha256>"}`.
The preset URL must be non-secret HTTPS Armbian `PRESET_CONFIGURATION` data;
keep `preset-firstrun` in the extensions list when using it. Private preset URLs
belong in the protected `ARMBIAN_PRESET_CONFIGURATION_URL` secret.

Optional diagnostic payload tarballs must use HTTPS and a matching SHA256.
Diagnostic payloads are staged by default and must not contain the production
runtime. Production images instead receive the exact three-file runtime bundle
and validate its `production-runtime` metadata and hash before installation. A
local `runtime-candidate` sidecar is historical qualification metadata; it does
not replace the production bundle.

### ALSA sequencer kernel fix

The Orange image includes the small `octessera_midi` Armbian extension. It uses
the documented `custom_kernel_config` hook to request only
`CONFIG_SND_SEQUENCER=m`, `CONFIG_SND_RAWMIDI=m`, and
`CONFIG_SND_USB_AUDIO=m`, and to force
`# CONFIG_RT_GROUP_SCHED is not set`. The last setting is the fixed Orange
kernel remedy for the confirmed live scheduler denial: this board runs cgroup
v2 with `CONFIG_RT_GROUP_SCHED=y`, and `pthread_setschedparam(SCHED_FIFO)`
continued to return `EPERM` even with a sufficient `RLIMIT_RTPRIO`. The Orange
qualification launch uses `LimitRTPRIO=70`; no `CAP_SYS_NICE`, ambient
capability, or other realtime capability is added, and only CPAL callback
threads may request FIFO 70. The runtime verifies the effective callback
policy and priority before treating a sink as qualified.

The extension sets `opts_val["RT_GROUP_SCHED"]="n"`, rather than relying on
`opts_n`: Armbian's core Docker extension can append `RT_GROUP_SCHED` to
`opts_y` later in the same configuration pass, while `opts_val` is the final
value override.

This change does not alter cgroup v2, global runtime sysctls, or the global RT
throttle. Keep `kernel.sched_rt_period_us=1000000` and
`kernel.sched_rt_runtime_us=950000`; the one-second period and 950 ms runtime
budget remain the safety boundary for realtime work. The installed module-load
file contains only `snd_seq` and `snd_seq_midi`: the sequencer device and its
raw-MIDI bridge. It does not use the obsolete `CONFIG_SND_SEQ` name, OSS
sequencer support, generic device discovery, or capability broadening.

The build also forces the `octessera_image_sanitize` extension. Its exact
Armbian hook, `pre_umount_final_image__9999_octessera_image_sanitize`, runs
against `MOUNT` immediately before the final image unmount. It removes only
`authorized_keys` under `/root/.ssh`, immediate `/home/*/.ssh` account homes,
`/etc/ssh`, and `/etc/dropbear`, then fails closed if any of those paths remain.
It never reads, hashes, or logs key contents. The early customizer cleanup and
strict built-image inspector remain in place as independent checks.

Include the extension in every image build, alongside any first-run extension:

```sh
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=sha,img,xz \
  -f 'extensions=preset-firstrun octessera_midi octessera_image_sanitize' \
  -f armbian_build_ref=<reviewed-40-character-armbian-commit> \
  -f run_build=true
```

The build gate is a newly generated matching
`output/debs/linux-image-current-sunxi64_<version>_arm64.deb` and the matching
`output/images/Armbian_*_orangepizero2w_current_*.img.xz`, not the old installed
kernel. Deploy the resulting Armbian image to the test SD card, boot it, and
reboot once after the new kernel is installed. Do not qualify the MIDI path
from a runtime rebuild or from `modprobe` on the old image.

After SSH returns from the reboot, verify the running kernel and both the
sequencer device and ALSA MIDI clients:

```sh
uname -r
modinfo snd_seq
modinfo snd_seq_midi
modinfo snd_usb_audio
test -c /dev/snd/seq
aconnect -l
```

Record the exact kernel package version, image filename, `uname -r`, and the
command output. `/dev/snd/seq` must exist, and `aconnect -l` must run against
the new image before any foreground runtime MIDI qualification.

The fixed-kernel gate must also reject an enabled or modular RT group
scheduler and must preserve the global throttle and cgroup mode:

```sh
kernel_config=/boot/config-$(uname -r)
grep -qxF '# CONFIG_RT_GROUP_SCHED is not set' "$kernel_config"
! grep -qE '^CONFIG_RT_GROUP_SCHED=' "$kernel_config"
test "$(stat -fc %T /sys/fs/cgroup)" = cgroup2fs
test "$(sysctl -n kernel.sched_rt_period_us)" = 1000000
test "$(sysctl -n kernel.sched_rt_runtime_us)" = 950000
```

Do not treat `CONFIG_RT_GROUP_SCHED=y` as a pass; the live test must fail
closed if that exact line is present. After the new kernel is running, start
the foreground runtime with its default audio-thread priority and verify
the actual callback thread reaches FIFO scheduling:

```sh
unset OCTESSERA_AUDIO_THREAD_PRIORITY
/tmp/octessera-pi 2>&1 | tee /tmp/octessera-fifo.log
```

In a second shell, while audio is active, require at least one Octessera
thread with `FF`/`SCHED_FIFO` and the expected default realtime priority 70,
and require no scheduler-denial log:

```sh
pid=$(pgrep -xo octessera-pi)
ps -L -p "$pid" -o pid,tid,cls,rtprio,comm
ps -L -p "$pid" -o cls=,rtprio= | awk '$1 == "FF" && $2 == 70 { found = 1 } END { exit found ? 0 : 1 }'
! grep -q 'audio callback RT promotion not qualified' /tmp/octessera-fifo.log
```

This FIFO check is a live gate, not a substitute for the kernel-config
assertion: it proves the running image, process limits, and callback path all
agree. Stop immediately if the config is `=y`, either RT throttle value is
changed, cgroup v2 is not mounted, the FIFO assertion fails, or the runtime
reports an unqualified `DAC`/`UAC2` callback promotion.

### First-boot setup portal

The generic image installs `wifi-connect` plus Octessera setup helpers. If the board has no configured network and setup is not complete, `octessera-setup.service` starts a local hotspot named `Octessera Setup` or `Octessera Setup xxxx`.

Separately, the image contains the inactive `octessera-wifi-foundation.service`
and its root-owned Wi-Fi-only helper. It is not enabled, does not replace the
Orange first-boot portal or sidecar, and is fixed to `wlan0` with gateway
`192.168.42.1` for bounded image validation only.

The image stages the generated Pi-family default and the complete 320-file sample
inventory during image construction. Stage those assets before an Armbian build:

```sh
bash tools/armbian-image/stage-musical-assets.sh
bash tools/armbian-image/test-musical-assets.sh
```

The first-boot provisioning service only seeds the default to
`/var/lib/octessera/presets/default.json` when that file is absent. The complete
sample inventory is already installed during image construction; boot does not
copy or replace sample media. The manifest records source URL, byte count, and
SHA-256. The source repository is the Stargate sample pack; its upstream README
describes the pack as free to use and redistribute, and the image retains that
attribution.

The captive portal at `http://192.168.42.1/` configures:

- Wi-Fi network and country code;
- SSH mode: off, public key, or password;
- optional hostname.

In SSH key mode, the installed key is the admin credential and the `octessera` user receives passwordless `sudo`. In password mode, the `octessera` password is used for both SSH login and `sudo`.

Security model: this is local first-boot trust. Until setup completes, anyone nearby who joins the setup hotspot can configure the device. Octessera does not add its own shared SSH password or baked SSH key, and it does not scrub Armbian's own root/bootstrap credentials from the image. The underlying Armbian image may still include its normal first-run bootstrap credentials or root setup path; if you use that path, change the default password immediately. Octessera still owns network SSH exposure: `ssh.service` and `ssh.socket` are masked until Octessera setup finalizes SSH. SSH host keys are generated on-device only when SSH is enabled.

Useful checks after boot:

```sh
systemctl status octessera-setup.service
journalctl -u octessera-setup.service --no-pager
systemctl is-enabled ssh.service || true
ls /etc/ssh/ssh_host_* 2>/dev/null || true
```

After flashing, run:

```sh
sudo octessera-armbian-diagnostics
cat /etc/octessera/build-metadata.env
```

The workflow intentionally does not copy Raspberry Pi `config.txt`, `dwc2`, BCM GPIO numbering, SD export, or fixed user-home assumptions. Its USB gadget and OLED services use the reviewed Orange UDC, H618 SPI, and H618 GPIO paths instead.

## Basic Armbian facts to capture

Run these before changing overlays:

```sh
cat /etc/os-release
uname -a
cat /proc/device-tree/model 2>/dev/null || true
cat /boot/armbianEnv.txt
ls -R /boot/dtb/*/overlay /boot/dtb/overlay 2>/dev/null || true
ls /sys/class/udc 2>/dev/null || true
ls /dev/i2c-* /dev/spidev* 2>/dev/null || true
gpioinfo 2>/dev/null || true
aplay -l 2>/dev/null || true
USB_CONFIG_RE='CONFIGFS_FS|USB_LIBCOMPOSITE|USB_CONFIGFS|USB_F_UAC2|USB_F_MIDI'
zcat /proc/config.gz 2>/dev/null | grep -E "$USB_CONFIG_RE" || true
grep -E "$USB_CONFIG_RE" /boot/config-$(uname -r) 2>/dev/null || true
```

Install `gpiod` if `gpioinfo` is missing.

## USB device/gadget validation

The current Raspberry Pi image starts the gadget with `octessera-usb-gadget` in the pi-gen stage. That script uses Linux configfs, which is portable in principle, but the Raspberry Pi image setup is not portable as-is.

### Raspberry Pi assumptions to avoid

- Loading `dwc2` as the USB device controller driver.
- Enabling gadget mode with Raspberry Pi `dtoverlay=dwc2` style config.
- Assuming the OTG port is wired and configured for peripheral mode.
- Assuming the service user and storage paths are `/home/pi/...`.
- Assuming the UAC2 ALSA card name matches the Raspberry Pi gadget path.

### Orange Pi checks

Gadget support requires a kernel UDC and configfs:

```sh
sudo modprobe libcomposite
sudo mount -t configfs none /sys/kernel/config 2>/dev/null || true
ls /sys/class/udc
USB_CONFIG_RE='CONFIGFS_FS|USB_LIBCOMPOSITE|USB_CONFIGFS|USB_F_UAC2|USB_F_MIDI|USB_F_MASS_STORAGE'
zgrep -E "$USB_CONFIG_RE" /proc/config.gz 2>/dev/null || true
grep -E "$USB_CONFIG_RE" /boot/config-$(uname -r) 2>/dev/null || true
ls /lib/modules/$(uname -r)/kernel/drivers/usb/gadget/function 2>/dev/null || true
```

Pass criteria:

- `/sys/class/udc` contains at least one controller after boot and overlay setup.
- `libcomposite` loads.
- UAC2 and MIDI configfs functions exist or can be loaded.
- Binding a minimal gadget does not disconnect power or network access unexpectedly.
- A host computer sees the expected device functions on the OTG/data port.

Treat an empty `/sys/class/udc` as a failed Orange Pi gadget validation. The Raspberry Pi script currently logs and skips when no UDC exists. That behavior is acceptable for a running Pi image, but not for Orange Pi bring-up.

If `/sys/class/udc` is empty, inspect Armbian overlays and the USB controller device tree. The likely fix is board-specific overlay or DTB work, not a change in Octessera runtime code.

Before binding any gadget, record the USB power topology:

- Which physical port is the OTG/data port.
- Whether the host powers the Orange Pi or the Orange Pi has separate power.
- How VBUS, CC, and ID/role detection are handled on the target board.
- Whether unplug/replug and host sleep/resume keep the board powered safely.

### Orange Pi gadget composer

The Orange image owns a board-specific combined configfs service. It does not
load Raspberry Pi `dwc2` or select the first controller. The only accepted UDC
is the verified `musb-hdrc.4.auto`; absence, a different controller, a bound
controller, or an existing configfs gadget fails closed. The service loads
`musb_hdrc`, `libcomposite`, `usb_f_uac2`, and `usb_f_midi`, creates UAC2 and
MIDI functions and their configuration links, and binds the UDC last:

```sh
systemctl status octessera-orange-usb-gadget.service
cat /sys/class/udc/musb-hdrc.4.auto/function
ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
```

The same teardown path unbinds first, removes configuration links, removes
function directories, and only then removes the gadget tree. The service does
not expose mass storage. The standalone composer at
`tools/orange-pi/orange-pi-usb-gadget.sh` uses the same exact UDC contract for
offline/fake-configfs qualification:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

Setup and teardown share one exclusive lifecycle lock. A concurrent operation
fails before changing the gadget; teardown still unbinds before any removal.

The gadget product string is `Octessera Audio + MIDI` for `combined`,
`Octessera MIDI` for `midi`, and `Octessera Line In` for `uac2`. The composer
keeps the existing manufacturer, configuration, serial, VID/PID, function
composition, and UAC2 names. MIDI and combined modes require the patched,
qualified image kernel to expose a writable `interface_string`. The composer
writes exactly 14 bytes of `Octessera MIDI` without a trailing LF, verifies the
byte-for-byte readback, and only then creates the MIDI configuration link and
binds the UDC. `id` remains set for ALSA identity but never substitutes for
`interface_string`; missing, write, readback, and bind failures roll back the
partial gadget. A generic Windows `MIDI function` label indicates an unpatched
or unqualified image and is not accepted for release validation.

Live host validation is intentionally separate from image construction. The
composer implements the exact UAC2/MIDI gadget composition, but its fake-configfs
test is not host audio evidence. Run the host checks below only during an
authorized live qualification; this document makes no host tone or capture claim.

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

Host-side checks for an authorized live qualification:

- Capture `lsusb -v` for each gadget configuration.
- Confirm DAW-visible MIDI naming and basic MIDI send/receive.
- Confirm the exact UAC2 audio output, sample rate, and reconnect behavior.
- Confirm unplug/replug and host suspend/resume behavior.
- Confirm no storage function is exposed by the Orange Pi gadget configuration.

The Linux Foundation VID/PID values used by the composer are only for local
validation. Do not treat them as release USB IDs or invent a replacement. Public
USB support requires an authorized identity plus electrical and manual FAT for
the exact image and assembled board. Defaults remain disabled.

## Peripheral validation

### I2C

- Enable the required Armbian I2C overlay in `/boot/armbianEnv.txt` if the bus is absent.
- Record the bus path that sees NeoTrellis and NeoKey devices.
- Confirm expected seesaw addresses before adding an Orange Pi profile.
- Confirm the detected bus is muxed to the exact physical pins used by the Octessera harness.

### SPI and OLED

- The reviewed image installs `octessera-h618-spi1-cs0.dtbo` and enables it with `user_overlays=octessera-h618-spi1-cs0`. Do not substitute the stock `spidev1_0` overlay.
- Record the SPI bus/device path.
- Run a minimal OLED transfer test before starting the app.
- Confirm MOSI, SCLK, CS, DC, and reset are on the expected physical pins.

### GPIO and interrupts

- Use `gpioinfo` and edge-event tests to map physical pins to gpiochip lines.
- Do not translate Raspberry Pi BCM pin numbers by position.
- Confirm encoder, button, NeoKey, and NeoTrellis interrupt lines with edge events.
- Record active-low/active-high behavior and pullup/pulldown requirements.

### I2S DAC and audio

- Treat the I2S DAC as unproven until `aplay -l` exposes the expected card.
- Record required overlays and ALSA card names.
- Run a short playback test and an underrun/dropout check before foreground runtime qualification.
- Confirm bit clock, word select, and data pins match the existing DAC wiring.

## Runtime service status

The production image installs and enables `octessera.service`. It runs
the native runtime as the locked `octessera-runtime` system account; the
interactive `octessera` account remains separate for setup and administration.
The service supports the OLED, NeoTrellis, NeoKey, four encoders, persistent
store, samples, MIDI, and the selected exact audio outputs. Every non-empty
output set is valid; Jack is required only when selected, recognized disconnected
UAC2 and HDMI routes may wait and recover, selected route faults block readiness,
and no route is a fallback. Simultaneous physical outputs use independent
unsynchronized clocks and can drift or echo; this service contract does not
provide sample alignment. Runtime readiness follows selected-route status, initialized
control-surface devices, and the first rendered snapshot. The observed Orange
HDMI connector path is `/sys/class/drm/card0-HDMI-A-1`. Separately, a live
Raspberry Pi Zero 2 W observation on kernel `6.12.93+rpt-rpi-v8` found the exact
`/sys/class/drm/card0-HDMI-A-1/{status,edid}` paths; Raspberry code pins card0
and does not fall back to card1. These are connector identity observations, not
claims of connected HDMI audio or audible qualification.
FIFO priority 70 comes from
`LimitRTPRIO=70`; the service does not use `CAP_SYS_NICE` or ambient
capabilities.

Orange update check, apply, rollback, and OTA remain unsupported. Use the
verified production image artifact for an image update. The diagnostic image
mode and the historical smoke utilities remain separate from this service
path. Normal SIGINT cleanup joins workers after the shutdown frame, black
Trellis/NeoKey frames, and OLED-off operation.
