# Orange Pi Zero 2W distro evaluation

Goal: choose the best OS base before adding Orange Pi behavior or broad multi-board support.

Status:

- Desk evaluation: complete.
- On-device validation: pending Orange Pi Zero 2W hardware.
- Current recommendation: start hands-on evaluation with Armbian Debian 13/Trixie, keep the official Orange Pi image as the hardware fallback, and use DietPi as the footprint benchmark.
- Armbian bring-up details, including USB device/gadget mode checks: `hardware/docs/orange-pi-armbian-bringup.md`.

## Constraints

- Target board: Orange Pi Zero 2W, 2 GB RAM.
- Hardware goal: drop-in replacement for Raspberry Pi Zero 2 W.
- PCB constraint: no PCB changes.
- Wiring constraint: same OLED, NeoTrellis/NeoKey, encoders/buttons, and audio DAC wiring as the Raspberry Pi build.
- Current scope: PCB and software compatibility only. Ignore case compatibility until later.
- Repo goal: complete multi-board support for app code, HAL/profile handling, deploy scripts, image builds, release artifacts, and docs.
- Naming: use the canonical board profile IDs `raspberry-pi-zero-2w` and `orange-pi-zero-2w`.

## Candidate images

Evaluate these before adding Orange Pi behavior or broad multi-board refactors:

1. Armbian Debian 13/Trixie for Orange Pi Zero 2W.
2. DietPi Trixie if Orange Pi Zero 2W support is current.
3. Official Orange Pi/vendor image.
4. OpenWrt only if the first three options fail or a very small network-appliance image becomes the priority.

## Current ranking to test

| Rank | Candidate | Why test it | Main risk |
| --- | --- | --- | --- |
| 1 | Armbian Debian 13/Trixie | Best balance of normal Debian userspace, current packages, and reproducible build framework. | Board page is a rolling release, so production stability must be proven. |
| 2 | Official Orange Pi/vendor image | Most likely to expose board peripherals correctly during bring-up. | Older distro/kernel and weaker reproducible release-image story. |
| 3 | DietPi Trixie | Small footprint and appliance-friendly setup. | Support is DietPi-specific; kernel, Wi-Fi, Bluetooth, and audio issues may be out of scope. |
| 4 | OpenWrt | Very small appliance base. | More integration work for Rust app runtime, audio, I2S, and normal Linux service assumptions. |

## Desk evaluation results

| Candidate | Current status | Reproducible image path | Hardware-support expectation | Keep evaluating? |
| --- | --- | --- | --- | --- |
| Armbian Debian 13/Trixie | Board page lists Debian 13/Trixie for Orange Pi Zero 2W, current kernel, rolling release. | Strong. Armbian build framework supports board/release/minimal image parameters. | Good for I2C, SPI, and GPIO; I2S DAC depends on device-tree/overlay maturity. | Yes, primary target. |
| DietPi Trixie | DietPi lists an Orange Pi Zero 2W ARMv8 Trixie image. | Moderate. Good appliance tooling, but less board-focused than Armbian. | Likely OK for basic Linux bring-up; board-specific audio/kernel issues may be outside DietPi support. | Yes, footprint benchmark. |
| Official Orange Pi/vendor image | Vendor docs list Debian/Ubuntu/Android/Orange Pi OS images. | Weak to moderate. Vendor build/customization flow is less clean for release automation. | Best chance of vendor bootloader/device-tree peripheral support. | Yes, hardware fallback. |
| OpenWrt | Upstream support exists, but with caveats such as missing Wi-Fi driver in the referenced support work. | Strong if we accept OpenWrt buildroot-style workflow. | Good for lean appliance basics; weakest fit for Octessera audio/I2S and normal Linux userspace assumptions. | Low priority. |

## Scorecard

Score each candidate from 1 to 5.

| Criterion | Armbian | DietPi | Vendor image | OpenWrt |
| --- | --- | --- | --- | --- |
| First boot and SSH setup | 4 | 4 | 3 | 2 |
| Package install and update reliability | 5 | 4 | 2 | 3 |
| I2C device exposure | 4 | 3 | 5 | 3 |
| SPI device exposure | 4 | 3 | 5 | 3 |
| GPIO interrupt support | 4 | 3 | 5 | 3 |
| I2S DAC/audio device support | 3 | 3 | 4 | 2 |
| Audio latency and buffer behavior | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Service/systemd setup | 5 | 4 | 3 | 2 |
| Idle CPU and RAM footprint | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Boot time | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Thermal behavior and governor support | Pending hardware | Pending hardware | Pending hardware | Pending hardware |
| Reproducible image-build path | 5 | 4 | 2 | 4 |
| Release stability confidence | 3 | 3 | 4 | 3 |

Preliminary hypothesis score, to replace with hardware data:

| Candidate | Score | Role |
| --- | ---: | --- |
| Armbian Debian 13/Trixie | 4/5 | Primary candidate. |
| DietPi Trixie | 4/5 | Footprint and ease-of-use benchmark. |
| Official Orange Pi/vendor image | 3/5 | Hardware-support fallback. |
| OpenWrt | 3/5 | Low-priority lean-appliance alternative. |

## Bring-up checks per image

Run these on each serious candidate before selecting the base image.

The desk evaluation cannot complete these checks. They require the Orange Pi Zero 2W and the no-PCB-change wiring harness.

### Basic OS

- Flash image and boot without desktop packages.
- Confirm SSH access and stable network reconnect after reboot.
- Record kernel version, distro version, image date, and bootloader/source image URL.
- Install build/runtime dependencies needed by Octessera.
- Confirm systemd service installation works without distro-specific hacks.

### Hardware paths

- Confirm I2C bus path and scan seesaw devices.
- Confirm SPI device path and run a minimal OLED transfer test.
- Confirm GPIO chip and line mapping for all buttons/encoders/interrupts.
- Confirm GPIO interrupts work without polling-only fallbacks.
- Confirm I2S DAC is available and selected as the intended audio output.
- Run a short audio playback test and record underruns/dropouts.
- Confirm USB device/gadget mode exposes a UDC through `/sys/class/udc`.
- Confirm configfs gadget functions for MIDI, UAC2 audio, and mass storage exist and bind on the OTG/data port.
- Confirm Armbian uses `/boot/armbianEnv.txt` overlays for required buses; do not use Raspberry Pi `config.txt`/`dtoverlay` assumptions.
- Confirm shutdown/reboot behavior and any required privilege policy.

### Performance and stability

- Measure idle CPU/RAM after boot and after Octessera service start.
- Measure boot-to-service-ready time.
- Run the existing runtime/audio benchmark scenarios.
- Run a long audio/service soak test.
- Record thermals, throttling, and governor behavior.

## Decision rules

- Prefer Armbian if it exposes I2C, SPI, GPIO interrupts, and I2S audio cleanly and remains stable under soak testing.
- Prefer Armbian only if USB gadget/device mode is available without board-specific kernel patches we cannot carry.
- Prefer the vendor image only if Armbian has peripheral problems that cost more than the weaker image/release workflow.
- Prefer DietPi only if it clearly reduces setup and runtime footprint without making hardware support harder.
- Do not add Orange Pi behavior or broad board-profile refactors until one lead distro is selected and the no-PCB-change hardware path looks viable. Raspberry-only no-behavior-change foundation refactors are allowed.

## Expected repo work after selection

1. Extract current Raspberry Pi assumptions into `raspberry-pi-zero-2w` profile without behavior changes.
2. Add `orange-pi-zero-2w` profile and the required GPIO backend only after hardware validation.
3. Parameterize bus paths, GPIO mapping, audio device selection, diagnostics, service account/home paths, deploy target, image sanitation, and release artifact names.
4. Keep `platform-core`, `playback-runtime`, and `realtime-engine` shared.
5. Keep Raspberry Pi and Orange Pi image pipelines parallel.
