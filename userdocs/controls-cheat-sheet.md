# Controls cheat sheet

Keep this beside the instrument while you learn where everything lives. The
[printable quick reference](print/quick-reference.pdf) is the one-sheet
version, and the [controls reference](controls-reference.md) has the fine
print: the keyboard map, the Aux encoders and every mark on the OLED.

Octessera has regular controls and a few temporary modes (Play pages, tile
assignment, the probability painter, context help). When in doubt, look at
the OLED. It always shows what the controls are acting on.

## Learn these five first

| Control | What it does |
|---|---|
| Turn *Main* | Move through menus, or change the value you're editing. |
| Click *Main* | Select, enter, edit or confirm. |
| *Back* | Leave an edit, an overlay or a menu level. |
| *Play* (*Space* in the simulator) | Start or pause. |
| Hold *Fn*, press a grid edge tile | Left column picks a layer, right column picks a Play page. Hold *Shift* + *Fn* for the other page bank. |

## Regular controls

| Control | What it does |
|---|---|
| Turn *Main* | Move through the menu. While editing a number, plain turns are coarse and *Fn* turns are fine; *Shift* and *Shift* + *Fn* turns stay coarse. |
| Click *Main* | Select a row, enter a group, start or finish an edit, or run an action. |
| *Back* | Finish the current edit, leave the current overlay, or go up one menu level. On the main menu it opens the performance view, and the next *Back* closes it (`System > Display > View on Back` turns that off). |
| *Play* | Play or pause. In the sample browser it previews the highlighted sample instead. |
| *Shift* + *Play* | Stop and return to the start. Under external MIDI sync it instead arms a one-off resync at the next bar; playback carries on, then the transport snaps back to its origin. |
| *Fn* + *Play* | Stop and return to the start, always, even in the sample browser or under external sync. |
| *Fn* + turn *Main* right (not editing) | While paused or stopped, step the active layer's world forward one generation. Left does nothing; while playing, the OLED asks you to pause first. |
| *Shift* + *Fn* + *Play* | Does nothing. |
| *Shift* + *Back* | Clear and re-initialize the active layer. Very useful, and you have to mean it. While editing text, it deletes a character instead. |
| *Shift* + *Fn* + click *Main* | [Context help](controls-reference.md#context-help) for the highlighted row. |
| *Fn* + click *Main* | Open the [Aux mappings](controls-reference.md#aux-mappings-modal) list. |
| *Fn* + click an Aux encoder | Bind the highlighted value or action to that encoder ([more](controls-reference.md#aux-encoders-and-auto-map)). |

## Grid navigation

| Control | What it does |
|---|---|
| Hold *Fn* | Preview: the left column shows the layers, the right column the technical Play pages. Hold *Shift* + *Fn* for the musical ones. ([Colors](controls-reference.md#grid-preview-colors)) |
| *Fn* + left column | Jump to that layer (bottom row is Layer 1). This also closes any open Play page. |
| *Fn* + right column | Open a technical Play page. From the bottom: Mix, Pan, Route, Tempo, LFO, Faders. |
| *Shift* + *Fn* + right column | Open a musical Play page. From the bottom: FX, XY, Transpose, Rate, Trigger Gate. |
| *Shift* + *Fn* + left column | Mute or unmute that layer's triggers without switching to it. Muting releases its held notes, and the layer turns magenta in the left column. |

The Play pages themselves are explained in
[Behaviors and Play pages](behaviors-and-play.md#play-pages).

## Assignment modes

These take over the grid until you press *Back*. In all of them, *Shift* +
tile applies to the whole row and *Shift* + *Fn* + tile to the whole column.

| Mode | Opened from | What a press does |
|---|---|---|
| Sample assignment | `Sampler > Assign` | Stamps the sample slot plus Tile Tune, Fine and Level. High, Medium and Low tiles light magenta, yellow and green; Base tiles pale white; other slots dim gray. |
| Drum assignment | `Drum > Assign` | Stamps the selected voice plus the Tile values. The selected voice's tiles light bright white, other voices dim white. |
| MIDI Drum assignment | `MIDI Drum > Assign` | Stamps Note and Tile Vel. Tiles with that exact stamp light bright white, other stamped tiles dim white. |
| Trigger probability map | `Link > Lx > Trigger Prob. > Map Prob Grid` | Cycles the tile through zero, low, high and full (dark, magenta, yellow, green). |
| Play FX mapping | `Play > FX > Map to Grid` | Stores the current momentary effect on the tile. |

When stamping, pressing a tile that already holds exactly that stamp clears it;
anything else gets overwritten. More in
[Sound and Link](sound-and-link.md#drum-and-sampler-tiles-are-stamps).

## Tiny survival notes

- The OLED is the truth. If the grid does something odd, you're probably on a
  Play page or in an assignment mode. Check the OLED, then press *Back* or
  jump away with *Fn*.
- Help is *Shift* + *Fn* + click *Main*. I made it a three-finger combo so you
  won't open it by accident (you also won't remember it the first week, which
  is why it pops up right after the startup screen).
- If a behavior gets too busy, try probability before you delete the pattern.
  Let it breathe.
