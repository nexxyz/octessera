# Factory Patch UI Scenario

This scenario builds the factory-patch candidate through the native menu as a user would. It runs in memory, captures runtime output, and never calls private menu setters from the scenario recipe.

Command:

```bash
cargo test -p playback-runtime factory_patch_ui_scenario -- --ignored
```

## Input notation

- `M-ENC Turn +N/-N`: main encoder turns.
- `M-ENC Click`: main encoder press.
- `Back`: Back button press.
- `Grid x,y`: grid press at zero-based display coordinates.
- `Fn+Grid x,y`: hold Fn, press grid, release grid, release Fn.
- `Shift+Fn+Grid x,y`: hold Shift+Fn, press grid, release grid, release modifiers.
- `Aux1 Turn +N`: Aux encoder 1 turn.
- `Clock N`: send `N` PPQN pulses through `HostMessage::TransportPulseStep`.

The test driver chooses rows by visible OLED labels. It fails with a trace and the latest OLED rows if a row does not appear or does not refresh naturally.

## Named sequence

### 1. Clear to blank patch

1. Open `System`.
2. Select `Clear all`.
3. Confirm `Confirm Clear All`.
4. Return to root.

Expected result: transport stops, MIDI panic/note safety runs, all layers and instruments are `none`, grids are clear, and device preferences stay intact.

### 2. Build and grid seed

1. `Build > L1 > Behavior > Cellular > life`.
2. Set `Step Rate` to `1/16`.
3. Set `Spawn Count` to `0`; minimize `Spawn Interval`.
4. Paint L1 double-line cross:
   - `Grid 0..7,3`, `Grid 0..7,4`
   - `Grid 3,0..7`, `Grid 4,0..7`
5. `L2 > Behavior > Play > sequencer`.
6. Set `Step Rate` to `1/8`.
7. Paint L2 pattern:
   - bottom row: `Grid 0,0`, `2,0`, `4,0`, `6,0`
   - next row: `Grid 2,1`, `4,1`
   - next row: `Grid 1,2`, `3,2`, `5,2`, `7,2`
8. `L3 > Behavior > Play > looper`.
9. Set `Step Rate` to `1/8`.

### 3. Link

1. `Route`; verify `BPM 120`.
2. `L1 > Events`; verify activation routes to `I1` and `note_on`.
3. `L1 > Note Mapping`; verify pentatonic scale and root `D`; nudge `Start Note` to D.
4. `L2 > Scanning`; set `Scan Mode scanning`, `Scan Axis rows`, `Sections 1`, `Scan Unit 1/8`.
5. `L2 > Events`; set `Event Triggers On`; verify route to `I2`.
6. `L3 > Events`; set `Event Triggers On`; verify activation to `I3`; set deactivation action to `note_off`.
7. `L3 > Note Mapping`; verify pentatonic scale and root `D`; nudge `Start Note` to D.

### 4. Shape and samples

1. `Shape > Instruments > I1`.
2. Set `Type synth`.
3. `Synth > Filter > Cutoff`; turn the cutoff upward.
4. `Mixer > Route`; set `fx_bus_1`.
5. `I2`; set `Type sampler`.
6. `Sampler > Sample Slot 1 > Browse`; respond to the sample-list request with `Kick2.wav`; pick it; choose `Assign`; assign full bottom row.
7. `Sample Slot 2 > Browse`; respond with `distkit-clap.wav`; pick it; assign full next row.
8. `Sample Slot 3 > Browse`; respond with `165028__rodrigo-the-mad__mini-909ish-open-hat.wav`; pick it; assign full next row.
9. `I3`; set `Type synth`, `Note Mode hold`, `Mixer > Route fx_bus_1`.
10. `FX Buses > B1 > Slot 1`; set type `delay`.
11. `B1 > Slot 2`; set type `duck`, source `I2`, amount `60`.

### 5. Aux, XY, and Play FX mappings

1. `Link > Aux Mappings > Aux 1 > Turn`.
2. Pick `Shape > Instruments > I2 > Sampler > Filter > Cutoff`.
3. `Play > XY > X Axis`; pick `Shape > Instruments > I1 > Synth > Filter > Cutoff`.
4. `Play > XY > Y Axis`; pick `Shape > Instruments > I1 > Synth > Filter > Res`.
5. `Play > FX`; set `FX Type stutter`; choose `Map to Grid`; press `Grid 0,0`.
6. Set `FX Type freeze`; map `Grid 1,0`.
7. Set `FX Type pitch_shift`; map `Grid 2,0`.
8. Set `Semitones` to a different value; map another pitch-shift cell at `Grid 3,0`.

### 6. Playback assertions

1. Start transport; clock the pattern and assert musical output appears.
2. `Shift+Fn+Grid 0,0`: mute L1; clock and assert L2/L3 still output.
3. `Shift+Fn+Grid 0,0`: unmute L1.
4. `Shift+Fn+Grid 0,1`: mute L2; clock and assert L1/L3 still output.
5. `Shift+Fn+Grid 0,1`: unmute L2.
6. Mute L1 and L2, select L3, press several looper grid keys, clock across the loop, and assert output from the looper layer.
7. Unmute L1 and L2.
8. `Fn+Grid 7,5`: activate XY page; press an XY grid cell; clock once; assert a synth parameter command reaches audio.
9. `Fn+Grid 7,2`: activate FX page; press and release mapped FX cells; assert momentary FX start/stop commands are captured.
10. `Aux1 Turn +1`; assert the sampler bank cutoff command reaches audio.

## Bugs this scenario caught

- Runtime XY modulation updated native config but did not queue live audio commands for synth/sample filter targets.
- Aux turns for sampler filter cutoff used a broad instrument update instead of the single sample-bank parameter fast path.
