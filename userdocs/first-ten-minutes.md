# Your first ten minutes

The shipped patch is wired up and ready, so you can skip the manual and go
straight to poking things. This page walks you through it in ten small steps.
It works the same on the instrument and in the
[desktop simulator](desktop-simulator.md).

## 1. The panel in one picture

<!-- PHOTO NEEDED: top-down photo of the finished instrument with Main, Aux 1-3, the OLED, Back/Play/Shift/Fn and the grid labelled. Replace the text sketch below with it. -->

```text
+--------------------------------------------------------------+
|              +--------+         |  . . . . . . . .           |
|   (Main)     |  OLED  |         |  . . . . . . . .           |
|              +--------+  [Back] |  . . . . . . . .           |
|                          [Play] |  . . 8 x 8 grid .          |
|          (Aux 1)        [Shift] |  . . . . . . . .           |
|                           [Fn]  |  . . . . . . . .           |
|   (Aux 2)       (Aux 3)         |  . . . . . . . .           |
|                                 |  . . . . . . . .           |
+--------------------------------------------------------------+
```

| On the instrument | In the simulator |
|---|---|
| Turn *Main*, the big knob | Arrow keys or mouse wheel |
| Click *Main* | *Enter* |
| *Back* | *Backspace* or *Escape* |
| *Play* | *Space* |
| *Shift* | *Shift* |
| *Fn* | *Ctrl* |
| *Aux 1*, *2*, *3*: turn left / click / turn right | *1 2 3*, *Q W E*, *A S D* |
| A grid tile | Click it |

Turn *Main* to move through menus and click it to enter or confirm. *Back*
goes up a level. The top menu has five areas:

- **Build:** each layer's behavior, its settings and its seed.
- **Link:** tempo, swing, and how tiles become notes.
- **Shape:** instruments, FX buses and the global effects.
- **Play:** the performance pages.
- **System:** saves, recording, audio, connections, updates and shutdown.

New words? The [glossary](README.md#glossary) has them.

## 2. Press Play

Press *Play*. At a straight 120 BPM you hear six layers at once:

| Layer | Behavior | What you hear |
|---|---|---|
| 1 | `life` | A self-sustaining colony playing a dark saw-and-pulse synth with its own echo |
| 2 | `sequencer` | A beat on the Sampler: kick, clap and open hat |
| 3 | `looper` | A Kalimba |
| 4 | `sequencer` | A bass line on the Swell Bass FM voice |
| 5 | `twinkle` | A sky of stars on FM Glass, asleep for now |
| 6 | `raindrops` | Drops falling onto the synth drum kit, so a stray tom or rim shot lands wherever one hits the bottom row |

## 3. Touch it

Hold *Fn* and press the bottom tile of the left-hand column: that's Layer 1.
Now press a few tiles. Each one changes the `life` colony, and the synth line
follows whatever the colony makes of it. Some of your tiles die off, some
start something. That's the deal with `life`.

(*Shift* + *Back* clears the whole layer. Handy, but you have to mean it.)

## 4. Wake the stars

Hold *Shift* + *Fn* and press the fifth tile up in the left-hand column. The
stars on Layer 5 wake up and start chiming. Same again puts them back to
sleep. That combo mutes and unmutes any layer, so try it on the beat too.

## 5. Perform

Hold *Fn* and press the sixth tile up in the right-hand column to open
**Faders**. The first five columns are the synth's cutoff, the kalimba's
decay, the bass's FM index, the reverb and the bitcrusher. The right-hand
column pokes the stars: the top tile reseeds them (*Shift* clears them), and
the yellow ones below nudge Density, Birth Chance, Star Life, Fade Chance and
the step rate.

Then *Shift* + *Fn* and the bottom right tile opens **Play FX**. Hold a tile
for stutter, freeze, a filter sweep or a pitch jump on the top two rows, or a
tape stop, reverse, trance gate, dub throw, wash, riser or bitcrush below
them. Two at a time, one of each kind. *Back* leaves the page.

The other Play pages are in [Behaviors and Play pages](behaviors-and-play.md#play-pages).

## 6. Change a sound

Turn *Main* to **Shape** and follow
`Shape > Instruments > I3: Plucked fxb2 > Plucked > Preset > Load > Koto`.
It asks first, then the kalimba turns into a koto. For the drums, try
`Shape > Instruments > I6: Drum direct > Drum > Kit > Load > Hand Perc`.

The slot rows are named after their type and route, so yours may read slightly
differently if you've changed those. Layers 3 to 6 all play factory presets,
so `Preset > Load` (or `Kit > Load`) on any of their slots is a quick "what
else could this be?". Feeling braver? In
`Build > L1 > Beh.`, swap `life` for `brain` or `raindrops` and listen to the
same synth tell a different story.

## 7. Change the tempo

`Link > BPM` sets it, from 40 to 240. For a quicker grab, hold *Fn* and press
the fourth tile up in the right-hand column: the **Tempo** page has tempo
tiles from 50 to 240 BPM, a Tap tile in the top-left corner and a row of
nudges. More in [Tempo and swing](sound-and-link.md#tempo-and-swing).

## 8. Keep it or throw it away

Auto Save is off, so your edits live in memory until you save them.

- **Keep it:** `System > Saves > Default > Save Patch` makes it the patch the
  instrument starts with. For a named copy, use `System > Saves > Library`.
- **Throw it away:** there's no undo. `System > Saves > Default > Load Patch`
  goes back to your last save (it asks first). With Auto Save on, your last
  save is what you have right now, so there's nothing older to go back to.
- **Start over:** `System > Maintenance > Reset > Factory Patch` brings back
  the shipped patch. Your device settings stay. If it's the device settings
  (display, MIDI, USB, audio) you want back to factory, `Factory System` next
  to it does that and leaves your patches and presets alone.

## 9. Switch off

Choose `System > Shutdown` and confirm. It saves a recovery copy, then the
grid lights go out and the screen goes dark. Wait for that before you pull the
power; pulling the plug early takes your unsaved tinkering with it.

In the simulator, save what you want to keep and close the window.

## 10. Where next

- [Controls cheat sheet](controls-cheat-sheet.md): the buttons and combos on
  one page.
- [Behaviors and Play pages](behaviors-and-play.md): about 50 worlds and the
  performance pages.
- [Sound and Link](sound-and-link.md): how tile changes become notes,
  instruments and effects.
