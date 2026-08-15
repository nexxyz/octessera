# Orange Pi first boot setup

The Orange Pi image starts a small setup website if it does not already know a Wi-Fi network.

Before treating an image or a successful login as a ready instrument, read the
[board qualification and status page](board-qualification.md). It separates
source/build proof from the physical checks that still need a person, a named
board, and the assembled hardware.

The constructor stages the same canonical interactive terminal welcome as the
Raspberry image at `/etc/profile.d/octessera-welcome.sh`. It is silent for
noninteractive commands and redirected output. The empty admin
`/home/octessera/.hushlogin` keeps Armbian's login text from stepping on it;
this does not change SSH credentials or the setup portal.

Keep the two image workflows separate:

- **Production release image:** on the release page, choose the asset whose name
  ends in `-orange-pi-zero-2w.img.xz`. It installs and enables
  `octessera.service`. The service runs the native runtime as the locked
  `octessera-runtime` system account.
- **Diagnostic workflow image:** a separately produced workflow artifact for bus,
  OLED, and qualification checks. It intentionally has no production runtime
  service and is not the image to use for normal first boot.

Before flashing a production image, download the root `SHA256SUMS.txt` asset and
verify the line for that exact image. For example, on Linux:

```sh
grep '  octessera-<version>-orange-pi-zero-2w.img.xz$' SHA256SUMS.txt | sha256sum -c -
```

On Windows PowerShell, compare the hash named in that line with the image
directly:

```powershell
$expected = (Select-String -Path .\SHA256SUMS.txt -Pattern '  octessera-<version>-orange-pi-zero-2w.img.xz$').Line.Split()[0].ToLowerInvariant()
$actual = (Get-FileHash .\octessera-<version>-orange-pi-zero-2w.img.xz -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "image SHA-256 mismatch" }
```

Do not substitute a diagnostic workflow image, a checksum for another asset, or
a local runtime binary for the verified production image.

The production image also has a separate interactive `octessera` admin/setup
user. The service never runs as that user. Production supports the OLED,
NeoTrellis, NeoKey, four encoders, persistent store, samples, MIDI, and the
selected exact audio routes. Every non-empty Jack/USB/HDMI output set is valid.
Jack is required only when selected; recognized disconnected selected USB or
HDMI routes may wait and recover; a selected route fault blocks readiness, and
no route is a fallback. Readiness follows three checks: every selected required
route is healthy or in its recognized waiting state, the control surface
initializes, and the first runtime frame renders. A selected Jack route uses
`hw:CARD=octesseradac,DEV=0`; selected USB UAC2 and HDMI routes may wait for
their exact endpoints and recover when they return. The native menu persists
Jack Audio, USB Audio, and HDMI Audio independently. Simultaneous physical
outputs use independent unsynchronized clocks and can drift or echo; this phase
does not provide sample alignment. The observed Orange HDMI connector path is
`/sys/class/drm/card0-HDMI-A-1`. Raspberry has a separate fixed connector
contract: the live Pi Zero 2 W observation on kernel `6.12.93+rpt-rpi-v8` uses
`/sys/class/drm/card0-HDMI-A-1/{status,edid}`. Neither observation claims
connected HDMI audio or audible qualification.

On boot, the Orange image reads the saved default from
`/var/lib/octessera/presets/default.json`. USB Audio exposes the fixed stereo
UAC2 gadget, and USB MIDI exposes the fixed MIDI gadget; either, both, or
neither may be enabled. HDMI and Jack do not change gadget composition. Save
the setting and use the confirmed device apply action; the image accepts only
the exact `reboot\n` request for config apply after validating the saved file.
The instrument's ordinary Reboot and Shutdown actions use the same fixed
root-owned socket: Shutdown sends exact `poweroff\n` without persisted-config
validation, while both actions first silence internal audio and panic external
MIDI. A rejected or indeterminate request is a typed failure, not permission
to try another command path.

The Orange runtime gets three attempts in a 30-second systemd start-limit
window: the initial start plus two retries. If it reaches `start-limit-hit`,
recover it explicitly from the console:

```sh
sudo systemctl reset-failed octessera.service
sudo systemctl start octessera.service
```

The boot OLED handoff has a 30-second monotonic deadline starting immediately
after handoff start, before OLED initialization or adoption. On timeout, a
signal, or an unexpected failure after ownership, it attempts an exact 32768-byte
black RGB565 frame and display-off `0xAE` independently. A cleanup failure does
not skip the other attempt. The failed handoff remains available to native
recovery with its current boot ID and matching request ID.

Orange update check, apply, rollback, and OTA remain unsupported. Use a
verified production image artifact from the release page for an image update;
do not treat the diagnostic image or a local runtime binary as a release image.

Use this before final assembly if you want. You do not need the OLED or buttons installed yet.

For the same setup portal from the instrument menu, on first boot or later, see
[Open or reopen the full setup portal](setup-portal.md). The first-boot steps
below remain the quickest path when the board is fresh from the image.

## First boot

1. Flash the Octessera Orange Pi Armbian image to a microSD card.
2. Put the card in the Orange Pi and power it on.
3. Wait for a Wi-Fi network named `Octessera Setup` or `Octessera Setup xxxx`.
4. Join that network from a phone or laptop.
5. Open the setup page if it does not appear automatically:

   ```text
   http://192.168.42.1/
   ```

6. Choose your Wi-Fi network.
7. Pick SSH access:
   - SSH key is best. The key becomes the admin credential and can use `sudo` without a password.
   - SSH password works if you need it. The same password is used for SSH login and `sudo`.
   - You can also leave SSH off.
8. Set a hostname if you want one.
9. Press the final connect button.

The setup hotspot disappears when the Orange Pi joins your Wi-Fi. That is the good kind of vanishing trick.

## Security note

The setup hotspot is for nearby, first-boot setup. Until setup finishes, anyone close enough to join that hotspot can configure the device.

Set it up near the device. Do not leave it powered on in setup mode in a public place. SSH keys are safer than passwords.

Octessera does not add its own shared SSH password or baked SSH key. The underlying Armbian image may still expose its normal first-run console/bootstrap credentials. If you use that path instead of the setup portal, change the default password immediately.

The setup portal creates or updates Octessera's SSH access. It does not scrub Armbian's own root/bootstrap credentials from the image, though Octessera still keeps network SSH closed until setup enables it.

## If setup does not appear

- Give the Orange Pi a minute or two after first power-on.
- If the setup hotspot disappeared before you finished, reboot the Orange Pi or restart `octessera-setup.service` from console. The setup hotspot intentionally times out instead of staying open forever.
- Check that your phone or laptop is not clinging to another Wi-Fi network.
- Try opening `http://192.168.42.1/` directly.
- If the setup network never appears, use serial/console access and check:

  ```sh
  systemctl status octessera-setup.service
  journalctl -u octessera-setup.service --no-pager
  ```

## Reopen setup later

Use `System > Configure WiFi` on the instrument. This starts the same full
portal even when Wi-Fi is already configured. It is safer than manually
changing setup markers: the runtime stops playback, the native adapter submits
a request, and the root-owned setup service handles the portal lifecycle.

## SPI and OLED bring-up

The Orange Pi Armbian image includes the reviewed SPI1/CS0 user overlay. It enables the header SPI pins and exposes one `/dev/spidev1.0` device at a maximum of 16 MHz. The OLED HAL defaults to 16 MHz and accepts only the validated 1/2/4/8/12/16 MHz qualification ladder. The image requires `overlays=i2c1-pi` for the header I2C bus and does not use the Raspberry Pi `config.txt` or stock `spidev1_0` path.

The separate `octessera-h618-input-routing` overlay disables UART0, clears its
boot stdout path, and releases PH0/PH1 as GPIO inputs. Image customization and
the checked provision tool remove active `console=ttyS0` arguments and disable
`serial-getty@ttyS0.service`; SSH configuration and access are not changed by
this input-routing step. AUX2's A/B lines stay available in either boot state,
and its click line becomes available after this overlay is applied and the
board is rebooted.

This is only the electrical bus setup. Before connecting or testing an OLED, follow the [Orange Pi Armbian bring-up notes](../../hardware/docs/orange-pi-armbian-bringup.md) to verify the live pinmux, DC/reset GPIO mapping, power, and recovery path. Do not copy this overlay to another board.

## Musical patch and samples

The image stages the generated Raspberry/Pi-family default patch without
replacing an existing user config. On first boot,
`octessera-provision-musical-default.service` copies it to
`/var/lib/octessera/presets/default.json` only when that file is absent. The
three samples referenced by the patch are verified by SHA-256 and copied only
when their destination files are absent, under:

```text
/var/lib/octessera/samples/
```

The staged samples come from the [Stargate sample pack](https://github.com/stargatedaw/stargate-sample-pack), whose upstream README describes them as free to use and redistribute. The image keeps that source attribution in the sample manifest.

## USB audio and MIDI

The production image includes an Orange service for optional UAC2 playback plus
MIDI through the verified `musb-hdrc.4.auto` controller. It refuses to bind
another UDC or to disturb an existing gadget. UAC2 is configured for 44.1 kHz
stereo playback. If Jack is selected, the exact Jack route remains required;
USB-only and HDMI-only selections do not use it as a hidden fallback. Raspberry
uses its separate exact card0 connector path and does not fall back to card1.
Setup and teardown use one exclusive lifecycle lock, so a concurrent lifecycle
call fails without changing the gadget.
The USB product is `Octessera Audio + MIDI` for the combined service,
`Octessera MIDI` for MIDI-only, and `Octessera Line In` for audio-only. MIDI
and combined operation require the patched, qualified image kernel. It must
expose `interface_string`; the service writes and verifies the exact 14-byte
`Octessera MIDI` value before binding. `id` remains an ALSA identity field, not
a substitute. Windows may retain an older cached `MIDI function` friendly name,
so release validation checks the current raw bus descriptor and requires its
exact `Octessera MIDI` value.
MIDI is part of the production runtime. Host enumeration is still worth checking
on the assembled board; inspect the gadget service before connecting a host:

```sh
systemctl status octessera-orange-usb-gadget.service
ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
```

UART0 remains intentionally disabled by the reviewed input-routing overlay;
this USB path does not restore it.

## Boot, UI sleep, and Linux suspend OLED behavior

The source implements the boot handoff contract described in the [board
qualification and status page](board-qualification.md). A real board check is
still the authority for a particular image, OLED, power arrangement, and
enclosure.
When that image is available, Orange should show the same four-band
magenta, green, yellow, and cyan sweep as Raspberry: 8 px bands, a rigid 45°
panel-facing top-right lean while the mounted SSD1351 controller origin
decreases and the train travels left-to-right; canonical bottom-to-top
coordinates use `bottom_origin - row_y`. Pixels remain
white-source-only, with 30 frames over 1.2 seconds (25 fps).

Orange's initramfs writes one static RGB565 logo+wordmark frame, then stops.
The root-installed `octessera-orange-boot-splash.service` is the sole loop and
runs while the other services load. Native startup
waits for the exclusive `/run/octessera-boot` OLED lock, adopts the display
without resetting it, and stops the animation just before an acknowledged
first normal menu frame. Orange first-menu readiness follows the selected-route
rules above; a selected fault blocks readiness, while a recognized disconnected
selected USB or HDMI route may wait. A queued frame is not enough.
Normal shutdown and reboot retain the clean logo+wordmark frame; they do not
restart the boot sweep.

The menu's `OLED Sleep` setting is a UI display-sleep feature; it does not
sleep Linux or hand the OLED to another process. Linux suspend uses a separate
Orange-only `sleep.target` transaction. Production enables
`octessera-orange-oled-suspend.service` with `RequiredBy=sleep.target`, so the
`sleep.target.requires` relationship is hard and a failed OLED handoff blocks
suspend. The runtime quiesces and detaches the OLED handles without cleanup
writes, the strict helper draws the suspend and resume frames, and the runtime
reacquires the hardware before and after the suspend. Audio, MIDI, transport,
and the LED surface are not part of that ownership transaction.

The Orange H618 path uses `/dev/spidev1.0` and GPIO lines on
`300b000.pinctrl` (reset 76, D/C 270), rather than Raspberry `rppal`, BCM, or
`dwc2` paths. The ownership contract is source- and fixture-tested, but a real
suspend/resume check still needs an operator physically present. A blank or
unstable OLED is a qualification result to record, not evidence that the
source contract is working on the board.

The NeoTrellis must use the exact validated bus, wiring, and addresses. Do not
substitute another bus or address; Orange has no alternate Trellis fallback.

## Final bench bring-up checklist

Run this with the boards accessible and a person at the device. It is a short
bench checklist, not a replacement for the detailed [Armbian bring-up
notes](../../hardware/docs/orange-pi-armbian-bringup.md). This checklist assumes
the production image; a diagnostic image intentionally has no production
runtime service.

### Passive checks: read-only

- Confirm the board model, selected Orange profile, image mode, running kernel,
  and recovery path. Keep the image identity and board revision in your notes.
- Confirm the expected I2C and `/dev/spidev1.0` devices, the Orange gpiochip
  ownership, and at least one UDC in `/sys/class/udc`.
- If Jack is selected, confirm `aplay -l` exposes it as
  `hw:CARD=octesseradac,DEV=0`; do not count HDMI or an implicit default device.
- Confirm the production runtime and Orange USB gadget service report their
  expected status before connecting a host computer.

### Active checks: controlled hardware actions

- Run a short playback through each selected exact route, listen for dropouts, and
  inspect the available audio and service logs. This can reveal audible or
  logged trouble, but it does not prove zero internally recovered ALSA
  `EPIPE`/xrun events.
- With the runtime running, exercise the OLED, the four NeoTrellis boards, the
  NeoKey, and all four encoders. Use the native System diagnostics where
  available; do not substitute Raspberry GPIO numbers or overlays.
- After the DAC check, connect the verified host-data USB-C path and check MIDI
  and the advertised audio/MIDI device. Confirm the board stays powered and
  does not back-feed before binding or reconnecting the gadget.
- Perform one normal reboot from the instrument or an attended console. Wait
  for SSH and the runtime to return, then repeat the display, control, and DAC
  checks.

### Manually observed pass criteria

- The OLED is readable, stable, and has one writer; boot does not leave it
  blank or flickering.
- All 64 grid cells respond, all four NeoKey switches respond, and each encoder
  turns and clicks reliably.
- Sound comes from each selected route without an audible dropout, and available
  audio/service logs are inspected. This does not prove zero internally
  recovered ALSA `EPIPE`/xrun events. The host sees the expected MIDI/USB
  functions without a storage function appearing.
- After reboot, the setup/runtime status is healthy and the same observations
  still hold.

Do not run Linux suspend, power-loss, repeated unplug, or other recovery tests
unattended. Those are separate qualification exercises that require an
operator, a proven recovery path, and a log of what happened.

## Advanced path

Armbian first-run presets still work for fleet or scripted setup. Use those only if you already know how you want to handle Wi-Fi credentials and SSH keys safely.
