# Orange Pi first boot setup

Newly constructed Orange Pi Zero 2W images include a small setup website, but
keep it closed until you deliberately open it. Retained legacy images built from
the v0.7.5 setup parent are explicitly outside this opt-in flow and retain their
legacy first-boot setup behavior. Read [board qualification and status](board-qualification.md)
before treating a successful login or image boot as a ready instrument.

## Published image and current constructor boundary

The boot, OLED handoff, lifecycle, and opt-in setup expectations in this page are
current source-defined constructor contracts. They apply only to a newly
constructed image built from those contracts and identified by release and
qualification evidence. The retained trusted v0.7.5 images are runtime/setup
parents with legacy first-boot behavior. They remain usable published images for
their documented runtime and setup paths, but they do not prove the current
constructor layer or the new opt-in setup behavior. A new full constructor image
still needs build validation and physical qualification.

## Select the correct image

Keep the two image workflows separate:

- **Production release image:** on the [current release page](https://github.com/nexxyz/octessera/releases), select the Orange Pi Zero 2W production image asset. It installs and enables `octessera.service`, which runs the native runtime as the locked `octessera-runtime` system account.
- **Diagnostic workflow image:** a separately produced workflow artifact for
  bus, OLED, and qualification checks. It intentionally has no production
  runtime service and is not the image for normal first boot.

### Verify the selected image

Download the production image and the checksum asset attached to the same
selected release. Use the checksum asset that matches that exact image. A
release may provide a matching image sidecar, such as an `.img.xz.sha256` file,
or a platform/global checksum file; do not mix assets from different releases.

For a standard `hash  filename` checksum asset, run this from the directory
containing the downloaded image on Linux:

```sh
IMAGE='the-downloaded-orange-image.img.xz'
CHECKSUM='the-matching-checksum-asset'
sha256sum -c --ignore-missing "$CHECKSUM"
```

On Windows PowerShell, set the variables to the exact downloaded filenames and
compare the matching checksum line:

```powershell
$image = '.\the-downloaded-orange-image.img.xz'
$checksum = '.\the-matching-checksum-asset'
$imageName = Split-Path -Leaf $image
$line = Get-Content -LiteralPath $checksum | Where-Object { $_ -like "*  $imageName" } | Select-Object -First 1
if (-not $line) { throw "image checksum entry not found" }
$expected = ($line -split '\s+')[0].ToLowerInvariant()
$actual = (Get-FileHash -LiteralPath $image -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "image SHA-256 mismatch" }
```

Do not substitute a diagnostic image, a checksum for another asset, or a local
runtime binary for the checksum-verified production image artifact. Orange also
supports runtime-only Check/Apply/Rollback through its root-owned broker and
guarded updater. That path accepts the profile-qualified
`octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip` with
`SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`; it does not replace the
Armbian image, kernel, device tree, or other full-image assets. Full image
replacement remains manual. The standalone manual runtime ZIP remains a manual
bundle and is not an OTA asset. Profile or asset mismatches fail closed rather
than selecting a Raspberry asset or falling back to the manual ZIP or image.

## Before first boot

The constructor stages the same canonical interactive terminal welcome as the
Raspberry image at `/etc/profile.d/octessera-welcome.sh`. It is silent for
noninteractive commands and redirected output. The empty admin
`/home/octessera/.hushlogin` keeps Armbian login text from stepping on it; this
does not change SSH credentials or the setup portal.

The production image keeps the interactive `octessera` admin/setup user separate
from the locked `octessera-runtime` service account. The service never runs as
the admin user.

Keep the board accessible and read [safety and power](safety-and-power.md). Do
not copy Raspberry pin numbers, overlays, or port roles to Orange. Use the
reviewed, board-specific [Orange Armbian bring-up procedure](../../hardware/docs/orange-pi-armbian-bringup.md)
for the exact board and image checks; it is a procedure, not proof that a
selected build passed its power, USB-role, or live-pinmux gates.

## First boot and setup

If native ownership has not arrived by the 30-second handoff window, the
existing splash owner writes a persistent dimmed `FIRST-RUN` / `HOUSEKEEPING` /
`PLEASE WAIT` status and continues polling the handoff state as the sole OLED
writer. This is a delayed-start legibility/recovery mitigation, not proof that
filesystem expansion is active or complete. A timeout alone does not invoke
black/off failure cleanup; only a genuine writer error or termination signal
does.

1. Flash the Octessera Orange Pi Armbian production image to a microSD card.
2. Put the card in the Orange Pi and power it on.
3. Wait for the normal Octessera runtime startup.
4. On the instrument, choose `System > Configure WiFi` and confirm `Open Portal`.
   This deliberate action emits the setup request; the installed request-path
   watcher then starts the setup service.
5. Wait for `Octessera Setup` or `Octessera Setup xxxx`.
6. Join that network from a phone or laptop.
7. Open `http://192.168.42.1/`.
8. Choose your Wi-Fi network.
9. Pick SSH access:
   - an SSH key is best; it becomes the admin credential and can use `sudo`
     without a password;
   - an SSH password also works, and the same password is used for `sudo`; or
   - leave SSH off.
10. Set a hostname if you want one.
11. Press the final connect button.

The setup hotspot disappears when the Orange Pi joins your Wi-Fi.

### Security note

The setup hotspot is for nearby, deliberately opened setup sessions. It does not
start automatically on first boot. Until setup finishes, anyone close enough to
join it can configure the device. Set it up near the device and do not leave it
powered on in setup mode in a public place. SSH keys are safer than passwords.

Octessera does not add a shared SSH password or baked SSH key. The underlying
Armbian image may still expose its normal first-run console/bootstrap
credentials. If you use that path instead of the setup portal, change the
default password immediately.

The portal creates or updates Octessera's SSH access. It does not scrub
Armbian's own root/bootstrap credentials from the image, though network SSH
remains closed until setup enables it.

### If setup does not appear

- Confirm the normal runtime has started, then choose `System > Configure WiFi`
  and confirm `Open Portal` again.
- If the hotspot disappeared before setup finished, reboot the board. After the
  normal runtime starts, deliberately choose `System > Configure WiFi` and
  confirm `Open Portal` again. A deliberately opened hotspot intentionally
  times out.
- Check that the phone or laptop is not clinging to another Wi-Fi network.
- Try `http://192.168.42.1/` directly.
- If the setup network never appears, use serial/console access and check:

  ```sh
  systemctl status octessera-setup.service
  journalctl -u octessera-setup.service --no-pager
  ```

For setup from the instrument menu later, choose `System > Configure WiFi` and
use [Open or reopen the full setup portal](setup-portal.md). Its apply behavior
is the same for both boards.

## Saved settings and recovery

The production image reads the saved default from
`/var/lib/octessera/presets/default.json`. Save settings and use the confirmed
device apply action. Config apply accepts only the exact `reboot\n` request after
validating the saved file. Ordinary Reboot and Shutdown actions use the same
fixed root-owned socket: Shutdown sends exact `poweroff\n` without persisted
configuration validation, and both actions first silence internal audio and
panic external MIDI. A rejected or indeterminate request is a typed failure,
not permission to try another command path.

The runtime gets three attempts in a 30-second systemd start-limit window: the
initial start plus two retries. If it reaches `start-limit-hit`, recover it
explicitly from the console:

```sh
sudo systemctl reset-failed octessera.service
sudo systemctl start octessera.service
```

## Samples and output paths

The image stages the generated Raspberry/Pi-family default patch without
replacing an existing user config. Image construction stages the complete
320-file sample artifact inventory: 318 WAV rows are sampler-loadable, while two
AIFF rows are retained as attribution-inventory metadata outside the WAV-only
browser/decoder. On first boot,
`octessera-provision-musical-default.service` only seeds the default to
`/var/lib/octessera/presets/default.json` when that file is absent; it never
copies or replaces sample media. Packaged sample bytes are verified by
SHA-256 under:

```text
/var/lib/octessera/samples/
```

User samples remain supported through this board sample path; the construction
manifest is not a reason to replace them during first boot.

The presets directory is a real `octessera-runtime`-owned `0755` directory.
The service refuses symlinks, wrong ownership, wrong modes, and other unsafe
paths instead of trying to repair a user-controlled destination.

The staged samples come from the [Stargate sample pack](https://github.com/stargatedaw/stargate-sample-pack),
whose upstream README describes them as free to use and redistribute. The image
keeps that source attribution in its sample manifest.

The production image supports the OLED, NeoTrellis, NeoKey, four encoders,
persistent store, samples, and selected exact audio routes. Every
non-empty Jack/USB/HDMI output set is valid; Jack is required only when
selected; recognized disconnected selected USB or HDMI routes may wait and
recover; a selected route fault blocks readiness; and no route is a fallback.
The selected Jack route is `hw:CARD=octesseradac,DEV=0`. USB Audio and USB MIDI
are experimental/local bench validation only, not public first-release support.
Defaults remain disabled. The current Linux Foundation VID/PID values are
local-validation-only and are not a public product identity. A kernel capability
check does not qualify the exact image or board; an authorized identity plus
electrical/manual FAT is required.

Independent physical output clocks can drift or echo; this phase does not
provide sample alignment.

## OLED, USB, and final bench checks

The reviewed, board-specific Armbian procedure describes SPI1/CS0 and I2C,
H618 GPIO mapping, the input-routing overlay, `/dev/spidev1.0`, and GPIO lines
on `300b000.pinctrl`; it does not use Raspberry `rppal`, BCM, or `dwc2` paths.
Confirm those expectations on the exact image/build. Do not copy the overlay to
another board; the electrical checks belong in the [Armbian bring-up
procedure](../../hardware/docs/orange-pi-armbian-bringup.md).

The boot handoff, UI sleep, and Linux suspend paths have separate OLED owners.
A blank or unstable OLED, a second writer, an unresolved pinmux, or an unclear
USB role is a qualification result to record, not evidence that the source
contract worked. Do not run Linux suspend, power-loss, repeated unplug, or other
recovery tests unattended.

Before closing the case:

- confirm the board model, selected Orange profile, image mode, kernel, and
  recovery path;
- confirm expected I2C, `/dev/spidev1.0`, GPIO, and UDC devices using the Orange
  notes;
- if Jack is selected, confirm `aplay -l` exposes
  `hw:CARD=octesseradac,DEV=0`;
- treat the Orange USB gadget as experimental/local bench validation, and
  connect a host only after an authorized identity and the exact build's
  port-role, VBUS/CC, and no-backfeed gates are recorded;
- exercise the OLED, all 64 grid cells, four NeoKey switches, four encoders,
  audio, MIDI, and USB while the assembly is open; and
- connect the host-data USB-C path for this exact build only after checking its
  port role, VBUS/CC behavior, and no-backfeed gate.

Do not substitute Raspberry GPIO numbers or overlays. Stop at any unresolved
physical gate and use [troubleshooting](../troubleshooting.md) or [board
qualification](board-qualification.md).

## Advanced setup

Armbian first-run presets still work for fleet or scripted setup. Use them only
if you already know how you will handle Wi-Fi credentials and SSH keys safely.
