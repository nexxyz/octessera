# Factory patch UI scenario

This heavy test builds the factory-patch candidate through the native menu,
the way a person at the instrument would: it reads visible OLED labels, turns
encoders, and presses grid keys. It runs in memory, captures runtime output, and
never pokes private menu state. If a row label changes, this is the test that
notices first.

```bash
cargo test -p playback-runtime factory_patch_ui_scenario -- --ignored
```

The pre-push hook runs it too. The recipe lives in
`crates/playback-runtime/src/native_runner/tests/ui_scenario/`
(`factory_patch_scenario.rs`, `factory_patch_configuration.rs`, and
`factory_patch_playback.rs`); this page is the readable version of that code.

## How the driver works

- It picks rows by their visible OLED label and fails with a trace and the
  latest OLED rows if a row never appears or does not refresh.
- Grid coordinates are zero-based display coordinates.
- `Fn+Grid x,y` means hold Fn, press and release the grid key, release Fn.
- Clocking sends PPQN pulses through `HostMessage::TransportPulseStep`.

## The sequence

### 1. Clear to an empty patch

`System > Maintenance > Reset > Load Empty`, then `Confirm Load Empty`.

### 2. Build and paint

1. `Build > L1 > Behavior > Cellular > life`; `Step Rate 1/16`; turn
   `Spawn Count` and `Spawn Interval` all the way down (`0` and `1`).
2. Paint the L1 double-line cross: rows 3 and 4, then columns 3 and 4.
3. `L2 > Behavior > Human > sequencer`; `Step Rate 1/8`.
4. Paint L2: `0,0 2,0 4,0 6,0`, then `2,1 4,1`, then `1,2 3,2 5,2 7,2`.
5. `L3 > Behavior > Human > looper`; `Step Rate 1/8`.

### 3. Link

1. `Link`: check `BPM 120`.
2. `L1 > Events`: check `On Trig note_on` and `On Inst I1`.
3. `L1 > Note Mapping > Set > Pentatonic > Major Pentatonic`; check
   `Set Maj Pent` and `Root D`; turn `1st Note` up by 2.
4. `L2 > Scanning`: `Scan Mode scanning`, `Scan Axis rows`, `Sections 1`,
   `Scan Unit 1/8`.
5. `L2 > Events`: `Event Triggers On`; check `On Inst I2`.
6. `L3 > Events`: `Event Triggers On`; check `On Inst I3`, `On Trig note_on`,
   and `Off Inst I3`; set the deactivation trigger to `note_off`.
7. `L3 > Note Mapping`: same scale, root, and `1st Note` steps as L1.

### 4. Shape and samples

1. `Shape > Instruments > I1`: `Type synth`; `Synth > Filter`, turn `Cutoff`
   up by 10; `Mixer > Route fxb1`.
2. `I2`: `Type sampler`, then `Sampler`. For each sample, turn `Sample Slot`,
   open `Browse`, answer the sample-list request with one file, pick it, choose
   `Assign`, and press a whole grid row:
   - `Kick2.wav` on row 0;
   - `distkit-clap.wav` on row 1;
   - `165028__rodrigo-the-mad__mini-909ish-open-hat.wav` on row 2.

   The slot turns are relative (0, then +1, then +2), so the hat ends up in
   the fourth slot; the expected fixture records exactly that.
3. `I3`: `Type synth`, `Note Mode hold`, `Mixer > Route fxb1`.
4. `FX Buses > B1 > Slot 1`: `Type delay`. `Slot 2`: `Type duck`,
   `Source I2`; check `Amount 60`.

### 5. Aux, XY, and Play FX

1. `Link > Aux Mappings > Aux 1 > Trn`: pick
   `Shape > Instruments > I2 > Sampler > Filter > Cutoff`.
2. `Play > XY > X Axis`: pick I1 `Synth > Filter > Cutoff`. `Y Axis`: pick
   I1 `Synth > Filter > Res`.
3. `Play > FX`: step the FX type and `Map` a cell each time: stutter on `0,0`,
   freeze on `1,0`, pitch_shift on `2,0`. Turn `Semitones` up by 7 and map a
   second pitch-shift cell on `3,0`.

### 6. Save, reload, compare

1. Walk the Build menu and check the generated values (behaviors, step rates,
   spawn values, looper `Length 16`, `Punch`, `Clear Loop`).
2. `System > Saves > Library > Save As` with the name `test.json`, then
   `Save` and `Confirm Save`.
3. In a fresh runner, `System > Load Preset > test.json` and `Confirm Load`.
4. Walk the Build menu again. Both the original and the reloaded config must
   match the expected fixture exactly.

### 7. Playback

1. Start transport and clock; musical events must appear.
2. Mute L1 with `Shift+Fn+Grid 0,0`; L2 must keep playing. Unmute it. (L1's
   Life cross settles after its first notes, so L2 is the reliable source.)
3. Mute L2 with `Shift+Fn+Grid 0,1`; its sampler must go silent. Unmute it and
   it must play again.
4. Mute L1 and L2, select L3 (`Fn+Grid 0,2`), hold looper keys `0..3,0` while
   clocking; the looper must produce output. Unmute L1 and L2.
5. `Fn+Grid 7,5` opens the XY page; pressing `6,6` and clocking must send a
   synth parameter command.
6. `Fn+Grid 7,2` opens the FX page; holding `0,0` must start a momentary FX.
7. Turning Aux 1 by +1 must send a sample-bank command.
