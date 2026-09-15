# Orange Pi Zero 2W Armbian bring-up

This is the ordered Orange Pi Zero 2W bring-up procedure for
the established Armbian production path. Use it at the workbench. The detailed
production image, service, storage, audio, USB, and updater contracts live in
the [Orange production reference](orange-pi-production-reference.md); image
construction commands live in
[`docs/workflows/image-construction-and-proof.md`](../../docs/workflows/image-construction-and-proof.md).

This is a hardware gate. Do not copy Raspberry Pi constants, overlays, or
`rppal` GPIO assumptions into Orange Pi support until these checks pass on the
target board and image. The Raspberry and Orange images, pinouts, ports, and
recovery paths are not interchangeable.

## Target context

- Board: Orange Pi Zero 2W, 2 GB RAM.
- Production image: Armbian Debian 13/Trixie for Orange Pi Zero 2W.
- Wiring goal: the same Octessera PCB and harness as the Raspberry Pi Zero 2 W
  build, with Orange-specific pin and port mapping.
- Exact profile: `orange-pi-zero-2w`; exact Armbian board ID: `orangepizero2w`.

Read [`docs/board-profiles.md`](../../docs/board-profiles.md) for the board
profile and artifact naming contract.

## Safety gates before connecting the Octessera PCB

Start bare-board. Do not connect the Octessera PCB or harness until all of these
checks pass:

- Compare the Orange schematic/header pinout against the Raspberry Pi Zero 2 W
  wiring used by Octessera.
- Confirm 5 V, 3.3 V, and GND land where the PCB expects them.
- Confirm every connected GPIO is 3.3 V logic and tolerates existing
  pullups/pulldowns.
- Confirm I2C, SPI, I2S, encoder/button, OLED reset/DC/CS, and interrupt lines
  expose the required functions on Armbian.
- Confirm power input and USB host/device wiring cannot back-power the board or
  brown it out during gadget binding.
- Confirm recovery before editing boot overlays: UART console, known-good SSH,
  or reflashing that does not depend on the gadget port.

If any pin or power check fails, stop. The no-PCB-change assumption is not valid
for that board/image combination.

Primary desk references:

- [Orange Pi Zero 2W product page](http://www.orangepi.org/html/hardWare/computerAndMicrocontrollers/details/Orange-Pi-Zero-2W.html)
- [Orange Pi Zero 2W H618 user manual v1.1](https://orangepi.net/wp-content/uploads/2023/10/OrangePi_Zero2w_H618_User-Manual_v1.1.pdf)
- [Orange Pi Zero 2W pinout table](https://git.munts.com/muntsos/doc/OrangePiZero2WPinout.pdf)

Use desk references only as a starting point. Trust physical pin numbers first,
then verify the board revision, schematic, Armbian device tree, and live pinmux.

## Preliminary header desk comparison

- Power positions appear to match 5 V, 3.3 V, and ground; confirm with a
  multimeter before connecting the PCB.
- Physical pins 3/5 appear to provide I2C1 SDA/SCL; confirm the live bus.
- Physical pins 19/21/23/24 appear to provide the reviewed SPI1 data/CS0 path;
  pin 26 is the reviewed SPI1 CS1 SD2 path. Confirm `/dev/spidev1.0`, the SD2
  node, and live pinmux.
- Physical pins 16/36 appear GPIO-capable for OLED D/C and reset; confirm lines
  and polarity.
- Physical pins 12/35/40 are not established as Pi-style I2S/PCM pins. I2S is blocked
  until schematic, DTS, and Armbian overlay checks establish those pins.
- Physical pin 10 is UART0 RX and pin 8 is UART0 TX in the desk pinout. The
  approved input-routing overlay must disable UART0 and release PH0/PH1 before
  NeoTrellis interrupt and SW3 switch checks.
- USB-C port role, VBUS/CC/ID behavior, UDC, and no-backfeed behavior are not
  established by the desk documents. Stop before gadget binding if they are unclear.

### Direct encoder mapping

H618 offsets use the established `port base + pin` mapping (`PC12 = 76`,
`PI14 = 270`). Do not use Raspberry BCM numbering.

| Encoder | A physical / H618 / offset | B physical / H618 / offset | Switch physical / H618 / offset | Implementation |
| --- | --- | --- | --- | --- |
| SW1 main | 29 / PI0 / 256 | 31 / PI15 / 271 | 32 / PI11 / 267 | available |
| SW2 aux1 | 33 / PI12 / 268 | 22 / PI6 / 262 | 11 / PH2 / 226 | available |
| SW3 aux2 | 13 / PH3 / 227 | 7 / PI13 / 269 | 8 / PH0 / 224 | A/B implemented; switch waits for UART0-disabled routing |
| SW4 aux3 | 37 / PI16 / 272 | 18 / PH4 / 228 | 15 / PI5 / 261 | available |

The Orange event boundary reverses all four literal board A/B directions; the
Raspberry path remains `rppal`-based. AUX2 A/B may be requested while UART0 is
active; its switch request is omitted until input-routing boot. Switch debounce
is 45 ms.

## Ordered bring-up

### 1. Choose and inspect the image

Use the exact Orange production or diagnostic mode described in the
[production reference](orange-pi-production-reference.md). Do not use a
Raspberry image or a runtime-only updater ZIP as a full image. Follow the
linked image workflow for image construction and its source checks.

Before any board change, inspect the basic Armbian state:

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

Install `gpiod` if `gpioinfo` is missing. Armbian uses `/boot/armbianEnv.txt`
and U-Boot overlays, not Raspberry `/boot/config.txt`, `dtoverlay=` names, or
BCM numbering. The reviewed SPI1 OLED+SD2 source is
`userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-oled-sd2.dts`;
do not substitute the stock `spidev1_0` overlay. SD2 chip select is header
pin 26; H618 PH9 is SPI1 CS1 with mux `0x4`, while the OLED is SPI1 CS0.
Before using the shared OLED/microSD wiring, verify its coexistence with the
live kernel and the electrical setup.

### 2. Verify setup and SSH

To exercise the setup portal on either fixed board path:

- at the instrument, choose `System > Setup > Configure WiFi > Open Portal`;
- join the setup AP and load the captive page;
- apply Wi-Fi, hostname, SSH mode, and login settings;
- wait for the OLED terminal result before reconnecting;
- observe the 10-minute user window after portal readiness and its timeout;
- inspect AP traffic, HTTP responses, the current status file at
  `/run/octessera-setup-status/current.json`, logs, and artifacts for secret
  leakage.

An AP disconnect while settings apply is expected. Setup requires a usable
global `wlan0` IPv4 address; it does not require Internet access, a default
route, DNS, or ICMP.

Once SSH is reachable, run the read-only Windows probe:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 -Target orangepi@192.168.x.x
```

The owner check requires passwordless `sudo -n` or a root SSH session. Add
`-WithSudoChecks` only after SSH/recovery is stable. The probe never binds a
gadget; use the separate composer for USB tests.

### 3. Check passive peripherals

Before active transfers, GPIO requests, audio, or gadget binding, check:

- confirm the live DT/pinmux for I2C, SPI1/CS0, OLED D/C/reset, I2S, USB role,
  and UDC;
- device nodes, GPIO ownership, `aplay -l`, and `/sys/class/udc`;
- scan I2C for the NeoTrellis/NeoKey devices on the correct physical bus;
- run a minimal OLED transfer and confirm MOSI, SCLK, CS, DC, and reset pins;
- use `gpioinfo` and edge events for encoders, buttons, NeoKey, and NeoTrellis;
- record polarity and pullup/pulldown requirements;
- expose the expected I2S card, use `hw:CARD=octesseradac,DEV=0` at 44.1 kHz,
  and run a short playback plus underrun check.

The production image constructs the AHUB0 dummy-codec route
on APB0/DMA3/TDM0 and names the playback card exactly `octessera-dac`
(`octesseradac` in ALSA). It installs and composes the canonical overlay during
image construction; this is not an experimental or manual overlay procedure.

An HDMI/default ALSA sound does not establish the I2S route. Use the DAC pins
and live audio card when checking that route.

### 4. Check USB gadget behavior

Before binding, verify which USB-C port is OTG/data, the power topology, VBUS,
CC and role handling, and host sleep/replug behavior. An empty
`/sys/class/udc`, an unproven role, backfeed, brownout, pre-bound controller,
or failed teardown is a stop condition. Do not use Raspberry `dwc2` assumptions
or bind a pre-existing gadget.

The official hardware name for the gadget-capable MUSB/peripheral controller is
USB0. Use unambiguous physical connector names; do not infer them from user
numbering.

Run the fake-configfs contract check first:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

For host checks, inspect `lsusb -v`, the UAC2 endpoint `Octessera Audio`, the
combined composite product `Octessera Audio + MIDI`, and Windows
`DEVPKEY_Device_BusReportedDeviceDesc`. The setup gate is writable ConfigFS
`interface_string`; the actual MIDI interface descriptor, bus-reported value,
and MIDI Services endpoints must equal `Octessera MIDI`. Verify high-speed
combined UAC2+MIDI, 44.1 kHz stereo capture of a board-generated 1 kHz tone on
both channels, and exact MIDI traffic in both directions. See the [Orange
production reference](orange-pi-production-reference.md#usb-identity-boundary)
for the fixed USB identity contract.

The Orange SD2 source/image contract includes the fixed
`/run/octessera-orange-storage-control/storage.sock` seam and label-safe
`OCTESSERA_SD` lifecycle.

For USB hardware testing:

- Use the exact release constructor image for the named identity and functional
  checks.
- Verify physical connector mapping, VBUS/CC/no-backfeed electrical behavior,
  physical reconnect and host suspend/resume, SD2 mass-storage
  start/eject/stop recovery, and the configured VID/PID values.

Before connecting a host to an instrument powered from the enclosure USB-C
input, follow the [safety and power
guidance](../../userdocs/hardware/safety-and-power.md) for
the fixed USB-A host path. Ordinary host cables carry VBUS; avoid
USB-C-to-USB-C/PD and do not treat the connector choice as isolation. This is
the no-backfeed safety gate.

### 5. Run the runtime diagnostic command

Run the non-destructive fixed-board diagnostic:

```sh
/usr/local/bin/octessera-pi --fat-diagnostic \
  --board-profile orange-pi-zero-2w \
  --evidence-dir "/tmp/octessera-fat-diagnostic-orange"
```

The diagnostic reports identity, readiness, service, storage, audio-route, and
USB state without binding USB, playing audio, or actuating the control surface.
`--hardware-test` and `--hardware-noise-test` are Raspberry interactive modes
and are rejected on Orange.

### 6. Fault handling

Stop before use when any mapping, power, recovery, UDC, I2S, GPIO, OLED,
control-surface, thermal, or service check fails. Fix the source, image, wiring,
or device before continuing.

The production runtime's selected-route readiness, service account, storage,
updater, and power boundaries are defined in the [technical
reference](orange-pi-production-reference.md).
