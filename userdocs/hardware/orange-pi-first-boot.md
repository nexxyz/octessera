# Orange Pi first boot setup

The Orange Pi image starts a small setup website if it does not already know a Wi-Fi network.

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

Before flashing a production image, download the matching checksum file whose
name ends in `.img.xz.sha256` and verify that exact pair. For example, on Linux
or macOS:

```sh
sha256sum -c octessera-<version>-orange-pi-zero-2w.img.xz.sha256
```

On Windows PowerShell, compare the hash named in that matching file with the
image directly:

```powershell
$expected = (Get-Content .\octessera-<version>-orange-pi-zero-2w.img.xz.sha256).Trim().Split()[0].ToLowerInvariant()
$actual = (Get-FileHash .\octessera-<version>-orange-pi-zero-2w.img.xz -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected) { throw "image SHA-256 mismatch" }
```

Do not substitute a diagnostic workflow image, a checksum for another asset, or
a local runtime binary for the verified production image.

The production image also has a separate interactive `octessera` admin/setup
user. The service never runs as that user. Production supports the OLED,
NeoTrellis, NeoKey, four encoders, persistent store, samples, MIDI, and the
internal DAC. Readiness follows three checks: required audio is healthy, the
control surface initializes, and the first runtime frame renders. USB-only
audio is unsupported; the internal DAC at
`hw:CARD=octesseradac,DEV=0` is required. USB UAC2 may be added as a companion,
but `audioOut=usb` is rejected.

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
stereo playback, but it is not a replacement for the required internal DAC.
Setup and teardown use one exclusive lifecycle lock, so a concurrent lifecycle
call fails without changing the gadget.
The USB product is `Octessera Audio + MIDI` for the combined service,
`Octessera MIDI` for MIDI-only, and `Octessera Line In` for audio-only. MIDI
and combined operation require the patched, qualified image kernel. It must
expose `interface_string`; the service writes and verifies the exact 14-byte
`Octessera MIDI` value before binding. `id` remains an ALSA identity field, not
a substitute. A generic Windows `MIDI function` label means the image is
unpatched or unqualified and is not accepted for release validation.
MIDI is part of the production runtime. Host enumeration is still worth checking
on the assembled board; inspect the gadget service before connecting a host:

```sh
systemctl status octessera-orange-usb-gadget.service
ls /sys/kernel/config/usb_gadget/octessera-orange-pi/functions
```

UART0 remains intentionally disabled by the reviewed input-routing overlay;
this USB path does not restore it.

## Future boot, sleep, and shutdown OLED behavior

The current source implements the Phase 5 boot handoff, but no new constructor
image has been built and this behavior has not yet been physically qualified.
When that image is available, Orange should show the same four-band cyan,
yellow, green, and magenta sweep as Raspberry: 8 px bands, a +8 px top-right
lean, white-source pixels only, and 24 frames over one second.

Initramfs runs one bounded foreground sweep and fully reaps it. Early userspace
then loops the sweep until the native runtime requests release. Native startup
waits for the exclusive `/run/octessera-boot` OLED lock, adopts the display
without resetting it, and stops the animation just before an acknowledged
first normal menu frame. Orange first-menu readiness also waits for healthy
internal DAC status; a queued frame is not enough.

Sleep, resume, and shutdown/reboot remain separate lifecycle paths. The Orange
H618 path uses `/dev/spidev1.0` and GPIO lines on `300b000.pinctrl` (reset 76,
D/C 270), rather than Raspberry `rppal`, BCM, or `dwc2` paths. Until the new
constructor image and physical checks are complete, a blank or unstable OLED
is a qualification result to record, not evidence that the source contract is
working on the board.

## Advanced path

Armbian first-run presets still work for fleet or scripted setup. Use those only if you already know how you want to handle Wi-Fi credentials and SSH keys safely.
