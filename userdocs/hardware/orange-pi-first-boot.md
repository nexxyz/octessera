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

The freshly flashed image boots offline without waiting for a network. NetworkManager
is independent of `octessera.service`: a runtime startup failure does not stop or
mask it. Fresh images keep the profile's SSH units masked until setup selects an
SSH key or password; no hotspot starts by itself. The production constructor
removes `/root/.not_logged_in_yet`, so Armbian's interactive first-login wizard
is suppressed. Its `armbian-firstrun.service` remains enabled with
`OPENSSHD_REGENERATE_HOST_KEYS=true`, and `armbian-resize-filesystem.service`
remains enabled for first-boot filesystem growth. To configure networking and
SSH, choose `System > Configure WiFi > Open Portal`; do not expect a fresh
production image to start an automatic hotspot. A warm reboot is not a guarantee
that Wi-Fi association returns.

Normal first boot does not wait six minutes. Upstream filesystem resize is ordered
before runtime; live Orange evidence saw resize complete from about 19.4 s to
22.7 s, with runtime starting at 24.3 s. Six minutes is only the upstream maximum
timeout, not a planned user-visible duration. The normal animated splash covers
this boot work.

If native ownership has not arrived by the 30-second handoff window, the board
source path shows one static polished `STARTUP DELAYED` /
`PLEASE WAIT` frame and continues polling the handoff state as the sole OLED writer.
This is delayed-start legibility/recovery mitigation, not a claim about resize
state. A timeout alone does not invoke black/off failure cleanup; only a genuine
writer error or termination signal does.

If Orange startup cannot initialize a fixed part, the OLED shows a short fixed
failure instead of leaving you guessing: `GRID NOT FOUND`, `NEOKEY NOT FOUND`,
`CONTROLS NOT FOUND`, `AUDIO NOT FOUND`, or `OLED NOT READY`. These screens tell
you to power off before checking or reseating the connection. The first fatal
screen is bright. If the same failure persists, it redraws once after 60 seconds
at a dimmed but readable level; a changed failure is bright again and starts a new
60-second interval. Generic `STARTUP FAILED` and `OLED NOT READY` cases point to
the service journal; the detailed reason remains there:
`sudo journalctl -u octessera.service --no-pager`.

1. Flash the Octessera Orange Pi Armbian production image to a microSD card.
2. Put the card in the Orange Pi and power it on.
3. Wait for the normal Octessera runtime startup.
4. On the instrument, choose `System > Configure WiFi` and confirm `Open Portal`.
   This deliberate action writes the one setup marker; the enabled path unit
   starts the root setup service.
5. Wait for `Octessera Setup` or `Octessera Setup xxxx`.
6. Join that network from a phone or laptop.
7. Open `http://192.168.42.1/`.
8. Choose the country and a scanned or manual Wi-Fi SSID. Select an open network
   or enter its password.
9. Pick SSH access. Key login and sudo authentication are separate:
   - an SSH key provides key-based login; it does not by itself make `sudo`
     passwordless. Key-only setup is insufficient for attended FAT diagnostics
     unless an approved sudo credential or policy already exists;
   - an SSH password is the normal attended FAT-diagnostic setup path because
     its sudo-capable credential can authenticate `sudo`; or
   - leave SSH off.
10. Set a hostname if you want one.
11. Press `Apply setup`.

The AP remains available for 10 minutes after it is ready. The browser's Applying
screen is provisional: an AP disconnect is expected and is not a success or
failure result. Wait for the OLED terminal result; it is authoritative. Success
requires a usable global `wlan0` IPv4 address. It does not require Internet
access, a default route, DNS, or ICMP. After success, choose `System > Info` to
see the IP. No reboot is required. Success and timeout cards auto-hide; failure
remains dismissible, and another attempt needs a new `Open Portal` action.

When Internet is available, `System > Updates` only checks, applies, or rolls back
the Octessera runtime. It does not update the Armbian OS/image, kernel, device
tree, or other full-image assets; those remain manual image operations.

### Security note

The setup hotspot is for nearby, deliberately opened setup sessions. It does not
start automatically on first boot. Until setup finishes, anyone close enough to
join it can configure the device. Set it up near the device and do not leave it
powered on in setup mode in a public place. SSH keys are safer than passwords.

Octessera does not add a shared SSH password or baked SSH key. Host keys are
generated on the board by Armbian first-run work. When the portal enables SSH,
the setup finalizer ensures the required host keys exist before it starts the
service. The vendor interactive first-login wizard is intentionally suppressed;
the setup portal owns password, key, and no-SSH choices.

### If setup does not appear

- Confirm the normal runtime has started, then choose `System > Configure WiFi`
  and confirm `Open Portal` again.
- If the hotspot disappeared while the browser was applying settings, wait for
  the OLED result. An AP disconnect is expected during the network switch. If
  setup failed or timed out, choose `System > Configure WiFi` and confirm
  `Open Portal` again for one new attempt.
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

The production image itself constructs the already-qualified AHUB0 vendor
dummy-codec route and exact playback card `octessera-dac`; there is no manual or
experimental audio overlay step during first boot.

Independent physical output clocks can drift or echo; this phase does not
provide sample alignment.

## OLED, USB, and final bench checks

The reviewed, board-specific Armbian procedure describes SPI1 OLED+SD2 and I2C,
H618 GPIO mapping, the input-routing overlay, `/dev/spidev1.0`, and GPIO lines
on `300b000.pinctrl`; it does not use Raspberry `rppal`, BCM, or `dwc2` paths.
Confirm those expectations on the exact image/build. Do not copy the overlay to
another board; the electrical checks belong in the [Armbian bring-up
procedure](../../hardware/docs/orange-pi-armbian-bringup.md).

Orange SD2 is header pin 26 with H618 PH9 mux `0x4` for SPI1 CS1, alongside
the OLED on CS0. Physical OLED/microSD coexistence is explicitly unqualified
in this phase and still needs live-kernel and electrical proof.

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

The production image intentionally skips Armbian first-login and does not
support its first-run presets. Use the Octessera setup portal for Wi-Fi,
hostname, and SSH configuration.
