# Controls cheat sheet

This is the map I wish I had taped next to the prototype while I was still learning where everything lived.

Octessera has regular controls, and then it has places where the controls temporarily become something else: Sparks pages, grid assignment modes, sample assignment, and context help. When in doubt, watch the OLED. It is small, but it tries to tell you what you are currently touching.

## Regular controls

| Control | Area | What it does |
|---|---|---|
| *Turn main encoder* | **Menu navigation** | Navigate the menu, or change the value currently being edited. |
| *Click main encoder* | **Menu selection** | Select menu entries, enter groups, edit values, or confirm actions. |
| *Back* | **Exit current location** | Exit the current edit, leave the current overlay, or go back one menu level. |
| *Play* / *Space* | **Transport** | Play or pause. |
| *Shift* + *Play* | **Stop** | Stop playback. In external sync, this arms resync instead of stopping the external clock. |
| *Shift* + *Back* | **Clear active layer** | Re-initialize the active layer. Very useful. Also very easy to press on purpose only. |
| *Shift* + *Fn* + *click main encoder* | **Context help** | Hold *Shift* + *Fn*, then click a menu option with *Main* to open help for that row. |

## Grid navigation shortcuts

| Control | Area | What it does |
|---|---|---|
| *Fn* + left grid column | **Navigate to Layer 1–8** | Jump to the chosen layer. Bottom row is layer 1. |
| *Hold* *Fn* | **Navigation preview** | Left column shows layers: cyan for navigation/current focus, green for configured layers, gray/black for inactive or unavailable cells. Right column shows Sparks pages in yellow, with the active page in green. |
| *Fn* + right grid column | **Navigate to Sparks pages** | Jump to *Mix*, *Pan*, *FX*, *Trigger Gate*, *Transpose*, or *XY*. If Sparks is already active, this exits Sparks. |
| *Fn* + *aux encoder click* | **Bind focused value/action** | Assign the highlighted menu value to that aux turn, or the highlighted action to that aux click. |

## Aux encoders and auto-map

Auto-map maps *Aux encoders 1–3* to the most important parameters and actions of the menu area you are currently in. It temporarily overrides the manual binding you might have created.

Each *Aux encoder* has two possible bindings:

- **Turn binding**: turning the encoder changes a value.
- **Click binding**: clicking the encoder triggers an action.

You can bind aux controls yourself with *Fn* + *aux encoder click* while a bindable menu row is focused.

How to read OLED markers:

| Marker | Meaning |
|---|---|
| `1-Cutoff` | *Aux 1* turn is bound or auto-mapped to Cutoff. |
| `1!Assign` | *Aux 1* click is bound or auto-mapped to Assign. |
| `1-/1!` style rows | That aux has both a turn binding and a click binding in this context. |
| `not active` toast | The binding still exists, but the target is hidden or inactive right now. |

## Cell-to-audio flow

![Cell-to-audio flowchart](print/cell-to-audio-flow.svg)

Flowchart source: [`print/cell-to-audio-flow.svg`](print/cell-to-audio-flow.svg).

Scanning is optional. If a layer is not scanning, it can still emit state-note events such as `activate` or direct grid events from `keys` and `looper`.

## Special modes

| Area | What changes |
|---|---|
| Sample assignment | *Shift* + cell maps the whole row. *Shift* + *Fn* + cell maps the whole column. |
| Trigger probability map | Cells set trigger chance for the selected layer: never, low, high, or always. *Shift* + cell maps the whole row. *Shift* + *Fn* + cell maps the whole column. |
| Sparks Mix | Grid turns into a mixer, where you can change the volume of each layer. |
| Sparks Pan | Grid lets you move around the layers' stereo position. |
| Sparks FX | Press mapped cells to trigger live effects. Releasing the cell stops the effect. |
| Sparks Trigger Gate | Grid lets you quickly block, allow, or use custom probability for each layer's triggers. |
| Sparks Transpose | Grid lets you temporarily transpose eligible synth and MIDI layers. |
| Sparks XY | Mappable two-axis surface for live-manipulating parameters. |

## Tiny survival notes

- The OLED is the truth. If the grid behaves in an unexpected way, you are probably in an overlay or Sparks page. Take a look at the OLED, and back out using *Back* or navigate away using *Fn*.
- Help is *Shift* + *Fn* + *click main encoder*. I made it a chord so it is hard to hit by accident.
- If a behavior gets too busy, try probability before you delete the pattern. Let it breathe.
