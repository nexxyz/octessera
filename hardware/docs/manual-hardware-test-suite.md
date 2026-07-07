# Manual Hardware Test Suite

Use this checklist for hands-on hardware bring-up when no OLED is installed. The operator watches the hardware while the test runner controls the Pi over SSH.

## Starting State

- Pi is reachable as `pi@192.168.0.211`.
- `cellsymphony.service` is disabled and inactive unless a test step explicitly starts it.
- OLED is removed.
- NeoTrellis, NeoKey, DAC/audio, encoders, and power are connected.

Before each session:

```powershell
./tools/pi/pi-preflight.ps1 -Target pi@192.168.0.211
```

Then confirm the app is stopped:

```bash
sudo systemctl disable cellsymphony.service
sudo systemctl stop cellsymphony.service
```

## 1. Bus And Power Baseline

Run with the app stopped:

```bash
pinctrl get 2
pinctrl get 3
sudo i2cdetect -y -r 1 0x2e 0x3f
vcgencmd get_throttled
aplay -l
```

Expected:

- `GPIO2` / SDA and `GPIO3` / SCL are high when idle.
- NeoTrellis appears at `0x2E`, `0x2F`, `0x30`, `0x31`.
- NeoKey appears at `0x3F`.
- `throttled=0x0`.
- ALSA lists `snd_rpi_hifiberry_dac` / `pcm5102a`.

Stop if the clean scan does not match. Fix wiring before running active tests.

## 2. NeoTrellis LED Sweep

Goal: prove all 64 grid LEDs can be addressed and confirm physical coordinate orientation.

Test behavior to run from code for up to 90 seconds, or until the operator presses Enter:

1. Clear all grid LEDs.
2. Light each physical 4x4 board address as a different solid color:
   - `0x2E`: red
   - `0x2F`: green
   - `0x30`: blue
   - `0x31`: white
3. Sweep one red pixel left-to-right, bottom-to-top in runtime grid coordinates.
4. Light the four logical corners:
   - `(0,0)` red
   - `(7,0)` green
   - `(0,7)` blue
   - `(7,7)` white

Operator records:

- Whether all LEDs light.
- Whether colors match.
- Whether `(0,0)` appears at the lower-left of the play surface. If it appears at the upper-left, the HAL Y mapping is inverted.
- Any rotated, mirrored, or board-swapped regions.

## 3. NeoTrellis Button Events

Goal: prove all 64 grid buttons generate press and release events.

Test behavior to run from code for up to 90 seconds, or until the operator presses Enter:

1. Clear grid LEDs.
2. On every grid press, log `grid_press x y` and light that cell yellow.
3. On release, log `grid_release x y` and dim that cell blue.
4. Keep a count of seen cells.
5. Print missing coordinates after each pass.

Manual pass:

- Press each cell once, left-to-right, bottom-to-top.
- Repeat the four corners.
- Hold two adjacent cells briefly to check multi-key handling.

Expected:

- Every press and release logs exactly once.
- Coordinates match lower-left runtime semantics.
- No stuck cells remain lit as pressed after release.

## 4. NeoKey LED And Button Test

Goal: prove four keys and four LEDs work without OLED feedback.

Test behavior to run from code:

1. Light NeoKey LEDs one at a time:
   - key 0 red
   - key 1 green
   - key 2 blue
   - key 3 white
2. Then set all keys dim amber.
3. Confirm hands-off readiness, then run a no-touch idle noise check across NeoTrellis, NeoKey, and encoders. Confirmed input during this phase is a failure. Raw NeoKey one-sample glitches whose immediate reread burst is clean are warnings within tolerance.
4. On press, log and set pressed key bright white.
5. On release, log and return it to dim amber.

Expected logical mapping:

| NeoKey index | Runtime input |
|---:|---|
| 0 | Back / `button_a` |
| 1 | Space / `button_s` |
| 2 | Shift / `button_shift` |
| 3 | Fn / `button_fn` |

Operator records:

- Physical left-to-right order.
- Any LED color-order mismatch.
- Any missed press/release.
- Any idle ghost press, encoder event, grid event, or raw noise warning on any input.
- NeoKey raw one-sample glitches whose immediate reread burst is clean are reported as warnings within tolerance, because runtime input confirmation rejects them.

## 5. Encoder Turn And Click Test

Goal: prove all encoder A/B pins and push switches work.

Test behavior to run from code:

1. Log every encoder turn as `encoder_turn id delta`.
2. Log every encoder click as `encoder_press id`.
3. Optionally flash NeoKey key 0 on main encoder input and NeoKey keys 1-3 for aux encoders.

Expected IDs:

| Physical control | Runtime ID |
|---|---|
| Main encoder | `main` |
| Aux 1 | `aux1` |
| Aux 2 | `aux2` |
| Aux 3 | `aux3` |

Manual pass per encoder:

1. Turn one detent clockwise.
2. Turn one detent counter-clockwise.
3. Turn quickly for several detents.
4. Press and release the push switch.

Record:

- Direction polarity.
- Bounce or duplicate clicks.
- Missing detents.
- Wrong encoder ID.

## 6. Audio Output Test

The test harness asks the operator to connect speakers or headphones and set a safe volume before playing the direct ALSA tone:

```bash
timeout 15 speaker-test -D hw:0,0 -c 2 -t sine -f 440 -l 1
```

Expected:

- Audible 440 Hz tone.
- No underrun or device-open error.

Then run a runtime audio smoke test:

1. Start the app only after I2C devices are healthy.
2. Trigger a simple internal synth note from code or a known grid action.
3. Confirm output through the DAC path, not HDMI or headphone fallback.

## 7. Integrated Runtime Smoke Test

Use after individual hardware tests pass.

1. Start `cellsymphony.service` manually.
2. Watch logs:

   ```bash
   journalctl -u cellsymphony.service -f
   ```

3. Confirm no `critical hardware init failed` messages.
4. Press grid cells and NeoKeys.
5. Turn and click encoders.
6. Confirm audio still works.
7. Stop the service and verify the I2C bus releases:

   ```bash
   sudo systemctl stop cellsymphony.service
   pinctrl get 2
   pinctrl get 3
   sudo i2cdetect -y -r 1 0x2e 0x3f
   ```

Expected:

- Service stays active.
- No restart loop.
- I2C devices remain detectable after service stop.
- Runtime input/output behavior matches hardware semantics.

## Test Harness Command

Run the no-OLED interactive hardware-test mode directly over SSH:

```bash
sudo systemctl disable cellsymphony.service
sudo systemctl stop cellsymphony.service
/usr/local/bin/cellsymphony-pi --hardware-test
```

The mode initializes NeoTrellis, NeoKey, DAC, and encoders without requiring an OLED. It then runs the LED checks, logs grid/key/encoder events to stdout, and launches the ALSA test tone.
It prints a final `SUMMARY` with warning and failure counts.

For unattended launch, set `CELLSYMPHONY_PI_HARDWARE_TEST=1` instead of passing `--hardware-test`.

To run only the no-touch input noise check:

```bash
sudo systemctl disable cellsymphony.service
sudo systemctl stop cellsymphony.service
/usr/local/bin/cellsymphony-pi --hardware-noise-test
```

For unattended no-touch-only launch, set `CELLSYMPHONY_PI_HARDWARE_NOISE_TEST=1`.
The noise-only mode also prints a final `SUMMARY`. Warnings indicate raw noise that is below the runtime confirmation threshold; failures indicate confirmed input, read failures, or encoder/grid idle events.

Use skip flags to isolate one hardware family while another is disconnected or suspect:

```bash
/usr/local/bin/cellsymphony-pi --hardware-noise-test --skip-trellis
/usr/local/bin/cellsymphony-pi --hardware-noise-test --skip-neokey
/usr/local/bin/cellsymphony-pi --hardware-noise-test --skip-encoders
```
