# Orange Pi Zero 2W Armbian bring-up

Goal: validate Orange Pi Zero 2W on Armbian before adding real `opi-zero-2w` runtime behavior.

This is a hardware gate. Do not copy Raspberry Pi constants, overlays, or `rppal` GPIO assumptions into Orange Pi support until these checks pass on the target board and image.

## Target context

- Board: Orange Pi Zero 2W, 2 GB RAM.
- First image to test: Armbian Debian 13/Trixie for Orange Pi Zero 2W.
- Fallback image: official Orange Pi/vendor image if Armbian exposes peripherals poorly.
- Wiring goal: same Octessera PCB and harness as the Raspberry Pi Zero 2 W build.

Record the image URL, image date, kernel version, board name, and all command output during bring-up.

Once the board is reachable over SSH, run the repo probe from Windows:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 -Target orangepi@192.168.x.x
```

The default probe is read-only. Add `-WithSudoChecks` only after SSH/recovery is stable. Add `-GadgetMidiSmoke -IUnderstandUsbRisk` only after the USB power/OTG gate below is understood.

`-WithSudoChecks` and gadget smoke require passwordless `sudo -n` or a root SSH session. If the board asks for a sudo password, run the default probe first, then either configure temporary passwordless sudo for bring-up or run the probe as root.

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
- OLED SPI: physical pins 19, 21, 23, 24, and 26 map to SPI0 signals in the third-party pinout. Confirm `/dev/spidev*` and pinmux before OLED testing.
- OLED D/C and reset: physical pins 16 and 36 appear GPIO-capable, but not display-specific. Confirm gpiochip lines and polarity.
- DAC I2S: physical pins 12, 35, and 40 are not proven as Pi-style I2S/PCM pins in the official docs found so far. This is blocked until schematic, DTS, and Armbian overlay checks prove I2S there.
- Encoder and button GPIOs: physical pins 7, 8, 11, 13, 15, 18, 22, 29, 31, 32, 33, and 37 need libgpiod mapping. Do not use BCM numbering.
- NeoTrellis interrupt: physical pin 10 is UART0 RX in the third-party pinout. Disable the serial console or stop if the no-PCB-change goal fails.
- SW3 switch: physical pin 8 is UART0 TX in the third-party pinout. Disable the serial console and verify edge events.
- USB gadget/data: official Orange Pi docs describe two USB-C USB2.0 ports and say both can power the board. They do not prove a Pi-style dedicated OTG/data port. This is blocked until port role, VBUS/CC/ID, and UDC behavior are proven on hardware.

Current desk result: power, I2C, and SPI look plausible; GPIO needs libgpiod mapping; I2S and USB gadget mode remain the highest-risk hardware gates.

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

## GitHub-built Armbian image

The `Armbian Image` GitHub Actions workflow can build a generic Orange Pi/Armbian image with Octessera setup helpers, diagnostics, and optional runtime payloads installed through Armbian `userpatches/`.

Start with validation only:

```bash
gh workflow run armbian-image.yml \
  -f board=orangepizero2w \
  -f release=trixie \
  -f kernel_branch=current \
  -f ui=minimal \
  -f compression=xz \
  -f extensions=preset-firstrun \
  -f run_build=false \
  -f artifact_mode=public-generic
```

Run a no-secret full build by changing `run_build=true`. Public generic artifacts must not contain Wi-Fi credentials, user passwords, SSH keys, or private first-run URLs. If you need first-boot personalization, use the private artifact mode with the protected `armbian-image-personalized` GitHub environment and repository/environment secrets; do not pass secrets as workflow inputs.

The only public first-run input is `public_preset_configuration_url`, and it must point to a non-secret HTTPS Armbian `PRESET_CONFIGURATION` file. Keep `preset-firstrun` in the extensions list when using that flow. Private preset URLs belong in the protected `ARMBIAN_PRESET_CONFIGURATION_URL` secret.

Optional Octessera payload tarballs must use HTTPS and a matching SHA256. Payloads are staged by default. The runtime is enabled only when the payload metadata explicitly says it is compatible, requests runtime enablement, and includes an executable `octessera-pi` payload.

### First-boot setup portal

The generic image installs `wifi-connect` plus Octessera setup helpers. If the board has no configured network and setup is not complete, `octessera-setup.service` starts a local hotspot named `Octessera Setup` or `Octessera Setup xxxx`.

The captive portal at `http://192.168.42.1/` configures:

- Wi-Fi network and country code;
- SSH mode: off, public key, or password;
- optional hostname.

In SSH key mode, the installed key is the admin credential and the `octessera` user receives passwordless `sudo`. In password mode, the `octessera` password is used for both SSH login and `sudo`.

Security model: this is local first-boot trust. Until setup completes, anyone nearby who joins the setup hotspot can configure the device. The image does not ship with a shared SSH password, SSH host keys, baked user keys, or enabled SSH. `ssh.service` and `ssh.socket` are masked until setup finalizes. SSH host keys are generated on-device only when SSH is enabled.

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

The workflow intentionally does not copy Raspberry Pi `config.txt`, `dwc2`, BCM GPIO numbering, USB gadget setup, SD export, or fixed user-home assumptions.

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
USB_CONFIG_RE='CONFIGFS_FS|USB_LIBCOMPOSITE|USB_CONFIGFS|USB_F_UAC2|USB_F_MIDI|USB_F_MASS_STORAGE'
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
- UAC2, MIDI, and mass-storage configfs functions exist or can be loaded.
- Binding a minimal gadget does not disconnect power or network access unexpectedly.
- A host computer sees the expected device functions on the OTG/data port.

Treat an empty `/sys/class/udc` as a failed Orange Pi gadget validation. The Raspberry Pi script currently logs and skips when no UDC exists. That behavior is acceptable for a running Pi image, but not for Orange Pi bring-up.

If `/sys/class/udc` is empty, inspect Armbian overlays and the USB controller device tree. The likely fix is board-specific overlay or DTB work, not a change in Octessera runtime code.

Before binding any gadget, record the USB power topology:

- Which physical port is the OTG/data port.
- Whether the host powers the Orange Pi or the Orange Pi has separate power.
- How VBUS, CC, and ID/role detection are handled on the target board.
- Whether unplug/replug and host sleep/resume keep the board powered safely.

### Minimal gadget smoke test

Run only after confirming the OTG/data port and power arrangement are safe:

```sh
sudo modprobe libcomposite
sudo mount -t configfs none /sys/kernel/config 2>/dev/null || true
G=/sys/kernel/config/usb_gadget/octessera-smoke
UDC=$(ls /sys/class/udc | head -n 1)
sudo mkdir -p "$G/strings/0x409" "$G/configs/c.1/strings/0x409" "$G/functions/midi.usb0"
echo 0x1d6b | sudo tee "$G/idVendor"
echo 0x0104 | sudo tee "$G/idProduct"
echo Octessera | sudo tee "$G/strings/0x409/manufacturer"
echo 'Octessera Smoke MIDI' | sudo tee "$G/strings/0x409/product"
echo smoke | sudo tee "$G/strings/0x409/serialnumber"
sudo ln -s "$G/functions/midi.usb0" "$G/configs/c.1/midi.usb0"
echo "$UDC" | sudo tee "$G/UDC"
```

Teardown:

```sh
echo '' | sudo tee "$G/UDC"
sudo rm -f "$G/configs/c.1/midi.usb0"
sudo rmdir "$G/functions/midi.usb0" "$G/configs/c.1/strings/0x409" "$G/configs/c.1" "$G/strings/0x409" "$G"
```

Prefer the guarded repo smoke test over copying manual configfs commands:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 -Target orangepi@192.168.x.x -WithSudoChecks -GadgetMidiSmoke -IUnderstandUsbRisk
```

If multiple UDCs are present, choose the intended controller explicitly:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 -Target orangepi@192.168.x.x -WithSudoChecks -GadgetMidiSmoke -IUnderstandUsbRisk -Udc <udc-name>
```

After MIDI works, repeat with UAC2 and mass-storage before reusing Octessera's full gadget script.

Host-side checks:

- Capture `lsusb -v` for each gadget configuration.
- Confirm DAW-visible MIDI naming and basic MIDI send/receive.
- Confirm UAC2 audio direction, sample rate, and reconnect behavior.
- Confirm unplug/replug and host suspend/resume behavior.
- For mass storage, test host eject/safe removal, remount, and `fsck` on the backing filesystem.
- Confirm root, eMMC, and system SD devices cannot be exported as mass-storage backing devices.

The Linux Foundation VID/PID values used by the smoke test and current script are only for local validation. Do not treat them as release USB IDs.

## Peripheral validation

### I2C

- Enable the required Armbian I2C overlay in `/boot/armbianEnv.txt` if the bus is absent.
- Record the bus path that sees NeoTrellis and NeoKey devices.
- Confirm expected seesaw addresses before adding an Orange Pi profile.
- Confirm the detected bus is muxed to the exact physical pins used by the Octessera harness.

### SPI and OLED

- Enable the required SPI overlay in `/boot/armbianEnv.txt` if `/dev/spidev*` is absent.
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
- Run a short playback test and an underrun/dropout check before Octessera service testing.
- Confirm bit clock, word select, and data pins match the existing DAC wiring.

## Runtime service validation

After bare peripheral tests pass, validate the actual Octessera service with board-specific paths and permissions:

- service user and home/store/sample paths;
- realtime/audio group access;
- ALSA device selection for jack/I2S and USB gadget audio;
- USB SD transfer start/stop permissions;
- shutdown/reboot privilege policy;
- boot-to-ready timing;
- controls, OLED, audio, MIDI, and gadget parity with Raspberry Pi behavior.

## Repo follow-up after hardware passes

Only after the checks above pass:

1. Add real `opi-zero-2w` board profile values.
2. Add a non-`rppal` GPIO backend based on gpiochip lines.
3. Split gadget setup by board/image layer so Raspberry Pi keeps `dwc2` and Orange Pi uses the detected UDC path.
4. Parameterize service user, store paths, samples paths, deploy target, preflight checks, and image sanitation.
5. Add Orange Pi image automation as a parallel Armbian path, not a pi-gen variant.
