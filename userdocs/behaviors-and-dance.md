# Behaviors and Dance pages

Each part runs one behavior. Think of a behavior as a small rule-based system: you seed it, nudge it, sometimes interrupt it, and listen to what it decides to become.

You can run several parts at once. One part might be a stable pulse. Another might be a strange little colony. Another might be your fingers. The fun starts when they disagree politely.

## Behaviors

| Behavior | Category | What it is good for |
|---|---|---|
| `none` | Play | Inactive part. Useful when you want space, or when you are setting up before sound. |
| `keys` | Play | Momentary finger-drumming. Press a cell to play; release it to stop. Immediate and human. |
| `sequencer` | Play | A manual grid sequence for anchoring the more generative parts. Add probability if it feels too square. |
| `looper` | Play | Records and replays grid presses/releases. Use Punch In/Out to overdub or perform. |
| `life` | Cellular | Conway-style cells that birth, survive, and die. A classic little organism for rhythm and texture. |
| `brain` | Cellular | Brian's Brain style states. It tends to leave trails and pulses rather than simply living/dying. |
| `ant` | Cellular | Langton-like motion. A tiny agent walks the grid and changes cell states as it goes. |
| `bounce` | Motion | Moving particles that bounce through the grid. Nice for kinetic patterns and repeating collisions. |
| `shapes` | Geometry | Geometric areas and edges as musical material. Good when you want a pattern with a visible skeleton. |
| `raindrops` | Fields | Drops/ripples across the grid. Great for sparse starts that bloom into motion. |
| `dla` | Growth | Diffusion-limited aggregation. Slow-growing clusters; more sculpture than step sequencer. |

The canonical behavior IDs are `none`, `life`, `sequencer`, `keys`, `looper`, `brain`, `ant`, `bounce`, `shapes`, `raindrops`, and `dla`.

## Trigger types

| Trigger | Meaning |
|---|---|
| `activate` | A cell becomes active. |
| `stable` | A cell remains active. |
| `deactivate` | A cell turns off. |
| `scanned` | The scan layer finds an active cell while scanning is enabled. |
| `scanned empty` | The scan layer visits an inactive cell while scanning is enabled. |

These triggers feed Sense, probability, note mapping, instruments, FX routing, and output. That is the bridge from cell state to sound.

## Dance pages

Dance pages are performance overlays. They temporarily borrow the grid so you can play the whole instrument, not just edit it.

Hold **Fn** and use the right grid column to choose a Dance page.

| Page | Grid role | Use it for |
|---|---|---|
| Mix | Grid turns into a mixer. | Change the volume of each part. |
| Pan | Grid becomes a stereo field. | Move around the parts' stereo position. |
| FX | Grid cells hold live effects. | Press mapped cells to trigger effects; release to stop them. |
| Trigger Gate | Grid becomes a trigger gate. | Quickly block, allow, or use custom probability for each part's triggers. |
| Transpose | Grid becomes a pitch offset picker. | Temporarily transpose eligible synth and MIDI parts. |
| XY | Grid becomes a mappable two-axis surface. | Live-manipulate assigned parameters with X/Y touch position. |

## Dance FX details

Dance FX are momentary. Pressing a mapped grid cell starts the effect. Releasing it stops the effect. Octessera limits them so the Pi does not melt into a sad little cracker:

- At most two momentary FX are active at once.
- Only one active cell of the same FX type is allowed.
- A second cell of the same FX type is ignored while that type is already active.
- Targets can be `global`, an FX bus, or an instrument insertion point.

## A useful patch recipe

The factory preset is a good orientation point: it has a self-sustaining `life` pattern on Part 1 and a basic sequencer rhythm on Part 2.

1. Use `sequencer` or `looper`, route it to a sampler, and create a basic rhythm loop to ground your track.
2. Add a generative part: `life`, `raindrops`, `bounce`, or `dla`, and route it to a synthesizer.
3. Use probability on your sequencer cells to make patterns play back in a more interesting way.
4. Bind one or two aux encoders to the parameters or actions you keep reaching for.
5. Open Dance Mix or Trigger Gate and perform the parts like little weather systems.

That is the heart of octessera for me: not a song file, not a rigid pattern, but a few small systems making music together.
