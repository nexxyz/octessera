# Orange Pi Zero 2W Armbian bring-up

Goal: validate Orange Pi Zero 2W on Armbian before adding real `orange-pi-zero-2w` runtime behavior.

This is a hardware gate. Do not copy Raspberry Pi constants, overlays, or `rppal` GPIO assumptions into Orange Pi support until these checks pass on the target board and image.

## Target context

- Board: Orange Pi Zero 2W, 2 GB RAM.
- First image to test: Armbian Debian 13/Trixie for Orange Pi Zero 2W.
- Fallback image: official Orange Pi/vendor image if Armbian exposes peripherals poorly.
- Wiring goal: same Octessera PCB and harness as the Raspberry Pi Zero 2 W build.

Record the image URL, image date, kernel version, board name, and all command output during bring-up.

The diagnostic artifact is the canonical `orange-oled-smoke` ELF. Its adjacent
`orange-oled-smoke.metadata.json` sidecar uses schema 2, contains the exact
identity field set, and binds the copied ELF with a canonical lowercase
`binary_sha256`. Keep those two files together; do not rename either one.
The separate `orange-seesaw-smoke` artifact uses the same sidecar contract and
is limited to the proven Seesaw reset/HW-ID check on `/dev/i2c-2`.

The foreground `octessera-pi` candidate is a separate schema-2,
SHA-256-bound `runtime-candidate` with `runtime_ready=false`. It is hardware-
free in metadata mode and uses only the proven OLED plus real NeoTrellis/NeoKey
I2C drivers in polling mode. Audio uses the shared 44.1 kHz runtime rate and
requires exactly one CPAL output device named
`hw:CARD=octesseradac,DEV=0` with verified stereo support; there is no
default or HDMI fallback. Internal MIDI events are ignored, while explicit
MIDI platform actions are unavailable. USB, updates, reboot, SD transfer, and
service actions are explicitly unavailable. Qualified encoders use the direct
gpiocdev path; SW3's UART0-TX switch line remains excluded. The candidate is
for a foreground qualification run only; the builder rejects deployment/
service-ready output and the Armbian image path must not install it.

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
- NeoTrellis interrupt: physical pin 10 is UART0 RX in the third-party pinout. Disable the serial console or stop if the no-PCB-change goal fails.
- SW3 switch: physical pin 8 is UART0 TX in the third-party pinout. Its GPIO line is explicitly excluded/faulted until boot routing changes.

### Direct encoder mapping

The PCB netlist supplies the A/B/switch physical header pins. H618 port offsets
use the established `port base + pin` mapping (`PC12 = 76`, `PI14 = 270`).

| Encoder | A physical / H618 / offset | B physical / H618 / offset | Switch physical / H618 / offset | Candidate status |
|---|---|---|---|---|
| SW1 main | 29 / PI0 / 256 | 31 / PI15 / 271 | 32 / PI11 / 267 | qualified |
| SW2 aux1 | 33 / PI12 / 268 | 22 / PI6 / 262 | 11 / PH2 / 226 | qualified |
| SW3 aux2 | 13 / PH3 / 227 | 7 / PI13 / 269 | 8 / PH0 / 224 | excluded: active UART0 TX |
| SW4 aux3 | 37 / PI16 / 272 | 18 / PH4 / 228 | 15 / PI5 / 261 | qualified |

Qualified lines are requested with gpiocdev v2 pull-ups and both-edge
quadrature/switch detection. The switch request retains the existing 45 ms
debounce contract. The Raspberry path remains rppal-based.

- USB gadget/data: official Orange Pi docs describe two USB-C USB2.0 ports and say both can power the board. They do not prove a Pi-style dedicated OTG/data port. This is blocked until port role, VBUS/CC/ID, and UDC behavior are proven on hardware.

Current desk result: power, I2C, SPI, and the static encoder mapping are
plausible; live encoder edge qualification remains bounded to the native HAL
path. I2S and USB gadget mode remain the highest-risk hardware gates.

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

The overlay declares one address cell and zero size cells, enables `&spi1` with `&spi1_pins` and `&spi1_cs0_pin`, then creates one CS0 `rohm,dh2228fv` device capped at 1 MHz. The reviewed H618 pin groups are PH6/PH7/PH8 with function `spi1` and PH5 with function `spi1`. It does not touch SPI0, CS1, GPIO lines, services, or authorization. It is not the stock `spidev1_0` overlay.

Image customization refuses any board other than the exact Armbian board ID `orangepizero2w`. It resolves the boot-selected `fdtfile`/current DTB path from the image boot configuration and refuses missing or ambiguous H618 candidates; it does not infer the target from `uname -r`. It compiles with symbols, decompiles and merges with fatal diagnostics, and asserts unchanged SPI0/I2C nodes plus the expected SPI1 pinctrl/CS0 result. DTBO and `armbianEnv.txt` writes are atomic. Non-secret source and DTBO SHA-256 values are recorded in `/etc/octessera/build-metadata.env` as `OCTESSERA_SPI1_CS0_DTS_SHA256` and `OCTESSERA_SPI1_CS0_DTBO_SHA256`.

The boot-environment parser rejects duplicate assignments, duplicate tokens, commented or inline-commented assignments, and malformed token lists. Full qualification image builds also require a reviewed immutable 40-character Armbian commit SHA; validation-only runs may use the workflow's default ref.

Do not enable this overlay on another board or kernel without a new device-tree review. Before any OLED transfer, prove the live SPI1 node and pinmux mapping on the target and keep a recovery path for `/boot/armbianEnv.txt`.

## GitHub-built Armbian image

The `Armbian Image` GitHub Actions workflow builds an Orange Pi Zero 2W/Armbian diagnostic image with setup helpers and bus diagnostics installed through Armbian `userpatches/`. It does not install or enable the Octessera runtime service.

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

Run a no-secret full build by changing `run_build=true` and setting `armbian_build_ref` to a reviewed full 40-character Armbian commit SHA; the mutable default ref is validation-only. Public generic artifacts must not contain Wi-Fi credentials, user passwords, user-specific SSH keys, or private first-run URLs added by Octessera inputs or overlays. If you need first-boot personalization, use the private artifact mode with the protected `armbian-image-personalized` GitHub environment and repository/environment secrets; do not pass secrets as workflow inputs.

The only public first-run input is `public_preset_configuration_url`, and it must point to a non-secret HTTPS Armbian `PRESET_CONFIGURATION` file. Keep `preset-firstrun` in the extensions list when using that flow. Private preset URLs belong in the protected `ARMBIAN_PRESET_CONFIGURATION_URL` secret.

Optional diagnostic payload tarballs must use HTTPS and a matching SHA256. Payloads are staged by default. Runtime-enabled or service-ready payloads are rejected; a `runtime-candidate` sidecar does not authorize installation, service enablement, or runtime readiness.

### First-boot setup portal

The generic image installs `wifi-connect` plus Octessera setup helpers. If the board has no configured network and setup is not complete, `octessera-setup.service` starts a local hotspot named `Octessera Setup` or `Octessera Setup xxxx`.

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

The Orange Pi path has its own configfs composer at
`tools/orange-pi/orange-pi-usb-gadget.sh`; do not copy the Raspberry Pi image
script or enable a service from this bring-up path. Run only after confirming
the OTG/data port and power arrangement are safe. Configfs and the requested
function modules must already be available:

```sh
sudo modprobe libcomposite
sudo mount -t configfs none /sys/kernel/config
sudo modprobe usb_f_midi
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh setup \
  --udc <exact-udc-name> --mode midi
```

Teardown:

```sh
sudo bash ./tools/orange-pi/orange-pi-usb-gadget.sh teardown \
  --udc <exact-udc-name>
```

Use `--mode uac2` for UAC2-only or `--mode combined` for both functions. The
composer requires an exact UDC argument, refuses pre-existing/pre-bound gadget
state, creates functions and configuration links before binding, and unbinds
first during teardown. It has no mass-storage mode and does not enable or
start any service. Its fake-configfs tests are offline:

```sh
bash ./tools/orange-pi/test-orange-pi-usb-gadget.sh
```

Host-side checks:

- Capture `lsusb -v` for each gadget configuration.
- Confirm DAW-visible MIDI naming and basic MIDI send/receive.
- Confirm UAC2 audio direction, sample rate, and reconnect behavior.
- Confirm unplug/replug and host suspend/resume behavior.
- Confirm no storage function is exposed by the Orange Pi gadget configuration.

The Linux Foundation VID/PID values used by the composer are only for local
validation. Do not treat them as release USB IDs.

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
- Run a short playback test and an underrun/dropout check before Octessera service testing.
- Confirm bit clock, word select, and data pins match the existing DAC wiring.

## Live qualification contract

Run this contract in order on one identified board. It is a bounded foreground
qualification, not permission to install or enable a service. Record the board
revision, PCB/harness revision, image/kernel/DT identity, artifact SHA-256, and
timestamps for every gate.

### Passive gate

Before any transfer, GPIO request, audio playback, USB bind, or runtime launch:

- Confirm the board is exactly `orangepizero2w`, the recovery path works, and no
  Octessera service or other process owns the connected hardware.
- Reconfirm the live DT/pinmux mappings for I2C, SPI1/CS0, OLED D/C/reset, I2S,
  USB role, and UDC. Record device nodes, GPIO ownership, `aplay -l`, and
  `/sys/class/udc`; do not infer a mapping from a Raspberry Pi number.
- Confirm the candidate is an Orange Pi artifact with matching metadata. Stage
  it only under `/tmp`; do not install it, replace a release, or start a
  service.
- Confirm the exact USB-C OTG/data port from the schematic and board. With the
  board power arrangement documented, measure VBUS and CC/role state with the
  host disconnected and connected. Pass only when the expected host/device
  direction, peripheral role, and no-backfeed/no-brownout behavior are proven.

### Reboot and staging gate

`/tmp` is cleared by reboot. After every controlled reboot, do not run a probe
or active test until SSH has returned and the artifact has been staged again:

```powershell
$Target = "orangepi@<address>"
$Artifact = "<local-path-to-orange-oled-smoke>"
$Metadata = "$Artifact.metadata.json"
$RemoteArtifact = "/tmp/orange-oled-smoke"
$RemoteMetadata = "/tmp/orange-oled-smoke.metadata.json"
$SshOptions = @("-o", "BatchMode=yes", "-o", "ConnectTimeout=5")
$Deadline = (Get-Date).AddMinutes(5)
$Reachable = $false
while ((Get-Date) -lt $Deadline) {
  & ssh @SshOptions $Target "true"
  if ($LASTEXITCODE -eq 0) { $Reachable = $true; break }
  Start-Sleep -Seconds 2
}
if (-not $Reachable) { throw "post-reboot SSH poll timed out; stop" }
& scp @SshOptions $Artifact "${Target}:$RemoteArtifact"
if ($LASTEXITCODE -ne 0) { throw "artifact redeploy failed; stop" }
& scp @SshOptions $Metadata "${Target}:$RemoteMetadata"
if ($LASTEXITCODE -ne 0) { throw "metadata sidecar redeploy failed; stop" }
& ssh @SshOptions $Target "chmod 0755 '$RemoteArtifact' && '$RemoteArtifact' --print-build-metadata"
if ($LASTEXITCODE -ne 0) { throw "staged artifact metadata check failed; stop" }
$LocalSha = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$RemoteShaOutput = @(& ssh @SshOptions $Target "sha256sum -- '$RemoteArtifact'")
if ($LASTEXITCODE -ne 0) { throw "remote SHA-256 command failed; stop" }
if ($RemoteShaOutput.Count -ne 1) { throw "remote SHA-256 output was not exactly one record; stop" }
$RemoteShaRecord = ([string]$RemoteShaOutput[0]).Trim()
$ShaPattern = "^(?<Hash>[0-9a-f]{64})\s+(?<Path>$([regex]::Escape($RemoteArtifact)))$"
$ShaMatch = [regex]::Match($RemoteShaRecord, $ShaPattern)
if (-not $ShaMatch.Success) { throw "remote SHA-256 output had an invalid format; stop" }
$RemoteSha = $ShaMatch.Groups["Hash"].Value
if ($RemoteSha -ne $LocalSha) { throw "remote binary SHA-256 differs from the recorded local SHA-256; stop" }
```

The metadata validation and the independent remote SHA-256 comparison are both
required before launching anything. Repeat this poll-and-redeploy sequence
after every reboot, including one caused by an overlay change. The utility's
metadata mode reads only the adjacent exact-name sidecar and hashes its running
`/proc/self/exe`; it performs no hardware initialization.

Stage the Seesaw diagnostic under its canonical names and repeat the same
independent SHA-256 check. The metadata command is intentionally unprivileged:

```powershell
$SeesawArtifact = "<local-path-to-orange-seesaw-smoke>"
$SeesawMetadata = "$SeesawArtifact.metadata.json"
$RemoteSeesawArtifact = "/tmp/orange-seesaw-smoke"
$RemoteSeesawMetadata = "/tmp/orange-seesaw-smoke.metadata.json"
& scp @SshOptions $SeesawArtifact "${Target}:$RemoteSeesawArtifact"
if ($LASTEXITCODE -ne 0) { throw "Seesaw artifact upload failed; stop" }
& scp @SshOptions $SeesawMetadata "${Target}:$RemoteSeesawMetadata"
if ($LASTEXITCODE -ne 0) { throw "Seesaw metadata upload failed; stop" }
& ssh @SshOptions $Target "chmod 0755 '$RemoteSeesawArtifact' && '$RemoteSeesawArtifact' --print-build-metadata"
if ($LASTEXITCODE -ne 0) { throw "Seesaw metadata check failed; stop" }
$SeesawLocalSha = (Get-FileHash -LiteralPath $SeesawArtifact -Algorithm SHA256).Hash.ToLowerInvariant()
$SeesawRemoteSha = @(& ssh @SshOptions $Target "sha256sum -- '$RemoteSeesawArtifact'")
if ($LASTEXITCODE -ne 0 -or $SeesawRemoteSha.Count -ne 1) { throw "Seesaw remote SHA-256 check failed; stop" }
if (([string]$SeesawRemoteSha[0]).Trim() -notmatch "^$SeesawLocalSha\s+$([regex]::Escape($RemoteSeesawArtifact))$") {
  throw "Seesaw binary SHA-256 differs from the recorded local SHA-256; stop"
}
```

### Active gate and order

Proceed only when the passive gate, staging gate, and USB electrical gate pass:

The OLED operation has a cooperative 3-second budget and a cooperative
1-second cleanup budget. Normal shutdown performs black and display-off
together; error and interruption cleanup uses one deadline, prioritizing
display-off before the fallback black frame. Synchronous SPI/GPIO calls may
outlast these checks, so neither budget is a wall-clock promise.

1. **Seesaw:** run `sudo -n /tmp/orange-seesaw-smoke --confirm-active-test` only after
   the passive and staging gates pass. It resets the four NeoTrellis addresses
   and NeoKey address on `/dev/i2c-2`, then reads their valid hardware IDs; it
   does not configure keypad events, write LEDs, poll keys, request GPIO, access
   OLED/SPI/audio, start runtime, or install a service.
2. **OLED:** run the diagnostic-only utility from `/tmp`. One invocation owns
   the cooperative pattern-to-black-to-display-off sequence, with operation
   and cleanup budgets, cleanup on errors, and handled interruption. Blocking
   SPI/GPIO syscalls are synchronous and may outlast those cooperative checks;
   record that limitation rather than treating the budgets as a wall-clock
   promise. Do not split it into separate commands:

   ```sh
   sudo -n /tmp/orange-oled-smoke --confirm-active-test
   ```

3. **I2S/DAC:** enumerate ALSA, select the exact CPAL endpoint
   `hw:CARD=octesseradac,DEV=0` at the shared 44.1 kHz runtime rate, and run
   one short playback plus an underrun check. A sound from HDMI or an
   implicit/default ALSA device is not an I2S pass.
4. **HDMI:** after the I2S result is recorded, enumerate HDMI separately and
   confirm it has not been selected as an audio fallback. Do not use HDMI to
   qualify the DAC wiring.
5. **USB gadget:** only after I2S and HDMI checks, recheck VBUS/CC/role and the
   exact UDC, bind one composer mode, verify host enumeration, then unbind and
   verify clean teardown. Use the Orange Pi composer; do not bind a pre-existing
   gadget or guess a UDC.
6. **Foreground candidate only:** after the preceding gates pass, a separately
   approved run may use the profile-matched diagnostic utility or foreground
   `runtime-candidate` from `/tmp`. Keep deployment, release, and service paths
   untouched; the candidate remains `runtime_ready=false`.

### Stop conditions

Stop the session, preserve logs and measurements, and do not retry or reorder a
gate if any of these occurs: the SSH poll times out; the board identity, boot
DT, pinmux, artifact metadata, or SHA-256 differs; `/tmp` staging or metadata
validation fails; the cooperative OLED operation or cleanup budget is
exhausted, or black/display-off cannot be confirmed; any bus hangs, GPIO
ownership mismatch, kernel fault, brownout, thermal rise, unexpected reboot, or hardware owner appears; the I2S card is absent, an
audio test falls back to HDMI, or playback underruns; VBUS/CC/OTG direction is
unproven, backfeed or power loss appears, UDC is absent/pre-bound, host
enumeration fails, or gadget teardown cannot unbind cleanly. Do not continue
to runtime qualification after a failed gate.

## Runtime service status

There is no Orange runtime service or deployment-ready release artifact to
validate. Do not copy the Raspberry Pi service, enable `octessera.service`, or
deploy `octessera-pi` as a service to this image. The foreground candidate
requires its exact audio endpoint for qualification but does not enumerate
HDMI, USB, or MIDI; its normal SIGINT cleanup joins workers after the shutdown
frame, black Trellis/NeoKey frames, and OLED-off operation. A future runtime
requires real qualified input, audio, and device adapters plus a separately
reviewed build/deploy/service contract.

## Repo follow-up after hardware passes

Only after the checks above pass:

1. Add real `orange-pi-zero-2w` board profile values.
2. Add a non-`rppal` GPIO backend based on gpiochip lines.
3. Split gadget setup by board/image layer so Raspberry Pi keeps `dwc2` and Orange Pi uses the detected UDC path.
4. Parameterize service user, store paths, samples paths, deploy target, preflight checks, and image sanitation.
5. Add Orange Pi image automation as a parallel Armbian path, not a pi-gen variant.
