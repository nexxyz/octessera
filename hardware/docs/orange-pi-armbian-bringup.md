# Orange Pi Zero 2W Armbian bring-up

This is the ordered Orange Pi Zero 2W bring-up and qualification procedure for
the established Armbian production path. Use it at the workbench. The detailed
production image, service, storage, audio, USB, and updater contracts live in
the [Orange production reference](orange-pi-production-reference.md); image
construction and proof commands live in
[`docs/workflows/image-construction-and-proof.md`](../../docs/workflows/image-construction-and-proof.md).
Historical diagnostic qualification remains in
[`orange-pi-selection-and-qualification-history.md`](orange-pi-selection-and-qualification-history.md).

This is a hardware gate. Do not copy Raspberry Pi constants, overlays, or
`rppal` GPIO assumptions into Orange Pi support until these checks pass on the
target board and image. The Raspberry and Orange images, pinouts, ports, and
recovery paths are not interchangeable.

## Target context

- Board: Orange Pi Zero 2W, 2 GB RAM.
- Production image: Armbian Debian 13/Trixie for Orange Pi Zero 2W.
- Wiring goal: the same Octessera PCB and harness as the Raspberry Pi Zero 2 W
  build, with Orange-specific pin and port proof.
- Exact profile: `orange-pi-zero-2w`; exact Armbian board ID: `orangepizero2w`.

Record the image URL, image date, kernel version, board name, board revision,
PCB/harness revision, artifact SHA-256, and all command output during bring-up.
Read [`docs/board-profiles.md`](../../docs/board-profiles.md) before treating a
profile-qualified artifact as the selected board.

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
  pin 26 is SPI1 CS1 and remains unused. Confirm `/dev/spidev1.0` and pinmux.
- Physical pins 16/36 appear GPIO-capable for OLED D/C and reset; confirm lines
  and polarity.
- Physical pins 12/35/40 are not proven Pi-style I2S/PCM pins. I2S is blocked
  until schematic, DTS, and Armbian overlay checks prove those pins.
- Physical pin 10 is UART0 RX and pin 8 is UART0 TX in the desk pinout. The
  approved input-routing overlay must disable UART0 and release PH0/PH1 before
  NeoTrellis interrupt and SW3 switch qualification.
- USB-C port role, VBUS/CC/ID behavior, UDC, and no-backfeed behavior are not
  proven by the desk documents. Stop before gadget binding if they are unclear.

### Direct encoder mapping

H618 offsets use the established `port base + pin` mapping (`PC12 = 76`,
`PI14 = 270`). Do not use Raspberry BCM numbering.

| Encoder | A physical / H618 / offset | B physical / H618 / offset | Switch physical / H618 / offset | Candidate status |
| --- | --- | --- | --- | --- |
| SW1 main | 29 / PI0 / 256 | 31 / PI15 / 271 | 32 / PI11 / 267 | implemented; hardware qualification pending |
| SW2 aux1 | 33 / PI12 / 268 | 22 / PI6 / 262 | 11 / PH2 / 226 | implemented; hardware qualification pending |
| SW3 aux2 | 13 / PH3 / 227 | 7 / PI13 / 269 | 8 / PH0 / 224 | A/B implemented; switch waits for UART0-disabled routing |
| SW4 aux3 | 37 / PI16 / 272 | 18 / PH4 / 228 | 15 / PI5 / 261 | implemented; hardware qualification pending |

The Orange event boundary reverses all four literal board A/B directions; the
Raspberry path remains `rppal`-based. AUX2 A/B may be requested while UART0 is
active; its switch request is omitted until input-routing boot. Switch debounce
is 45 ms.

## Ordered bring-up

### 1. Choose and inspect the image

Use the exact Orange production or diagnostic mode described in the
[production reference](orange-pi-production-reference.md). Do not use a
Raspberry image, a runtime-only updater ZIP as a full image, or the historical
`runtime-candidate` path as production qualification. For image construction,
kernel, sample, setup, and sanitation gates, follow the linked image workflow.

Before any board change, capture basic Armbian facts:

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
BCM numbering. The reviewed SPI1 source is
`userpatches/overlay/usr/local/share/octessera/device-tree/octessera-h618-spi1-cs0.dts`;
do not substitute the stock `spidev1_0` overlay.

### 2. Verify setup and SSH

The software/static setup layer is source-bound, but it does not prove a board
created an AP, joined a network, served a captive page, applied credentials, or
preserved secrets. Run the complete flow on both fixed board paths when doing a
shared setup qualification:

- create and join the setup AP, then load the captive page;
- apply Wi-Fi, hostname, SSH mode, and login settings;
- reconnect over the configured network;
- attach while setup is already running;
- observe the 30-minute timeout and portal closure;
- inspect failure/partial-state messages, AP traffic, HTTP responses,
  status/receipt files, logs, and artifacts for secret leakage.

Once SSH is reachable, run the read-only Windows probe:

```powershell
.\tools\orange-pi\run-opi-bringup.ps1 -Target orangepi@192.168.x.x
```

The qualification-critical owner proof requires passwordless `sudo -n` or a
root SSH session. Add `-WithSudoChecks` only after SSH/recovery is stable. The
probe never binds a gadget; use the separate composer only for an explicitly
authorized USB test.

### 3. Qualify passive peripherals

Before active transfers, GPIO requests, audio, or gadget binding:

- confirm the live DT/pinmux for I2C, SPI1/CS0, OLED D/C/reset, I2S, USB role,
  and UDC;
- record device nodes, GPIO ownership, `aplay -l`, and `/sys/class/udc`;
- scan I2C for the NeoTrellis/NeoKey devices on the correct physical bus;
- run a minimal OLED transfer and confirm MOSI, SCLK, CS, DC, and reset pins;
- use `gpioinfo` and edge events for encoders, buttons, NeoKey, and NeoTrellis;
- record polarity and pullup/pulldown requirements;
- expose the expected I2S card, use `hw:CARD=octesseradac,DEV=0` at 44.1 kHz,
  and run a short playback plus underrun check.

An HDMI/default ALSA sound is not an I2S pass. I2S remains blocked until the
DAC pins and live audio card are proven.

### 4. Qualify USB gadget behavior

Before binding, record which USB-C port is OTG/data, the power topology, VBUS,
CC and role handling, and host sleep/replug behavior. An empty
`/sys/class/udc`, an unproven role, backfeed, brownout, pre-bound controller,
or failed teardown is a failed Orange validation. Do not use Raspberry `dwc2`
assumptions or bind a pre-existing gadget.

Run the fake-configfs contract check first:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

Only during an authorized live qualification, capture `lsusb -v`, confirm
DAW-visible MIDI naming and send/receive, confirm exact UAC2 output/rate and
reconnect behavior, test host suspend/resume, and confirm no mass-storage
function. The Linux Foundation VID/PID values are local-validation-only, not a
public product identity; defaults remain disabled.

USB Audio and USB MIDI are experimental local bench-validation paths, not public
first-release support claims. Before connecting a host to an instrument powered
from the enclosure USB-C input, use a data-only cable or power-isolating adapter.
Software cannot prevent a host cable from back-feeding 5V while retaining data.
This is the no-backfeed safety gate.

### 5. Run the safe runtime evidence command

After image identity, passive devices, and recovery are recorded, run the
non-destructive fixed-board diagnostic separately:

```sh
/usr/local/bin/octessera-pi --fat-diagnostic \
  --board-profile orange-pi-zero-2w \
  --evidence-dir "/tmp/octessera-fat-diagnostic-orange-<fresh-run-id>"
```

This collects identity, readiness, service, storage, audio-route, USB-state,
and sanitized evidence without binding USB, playing audio, or actuating the
control surface. `--hardware-test` and `--hardware-noise-test` are Raspberry
interactive modes and are rejected on Orange.

### 6. Record or stop

Record the exact image/kernel/DT identity, source and artifact hashes, command
outputs, board/PCB revisions, measurements, logs, and operator observations.
Stop and preserve evidence if any mapping, power, recovery, UDC, I2S, GPIO,
OLED, control-surface, thermal, or service gate fails. Do not reorder a failed
gate or call a source/build check physical FAT.

The production runtime's selected-route readiness, service account, storage,
updater, and power boundaries are normative in the [technical reference](orange-pi-production-reference.md).
