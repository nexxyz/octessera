# Octessera

Octessera turns cellular automata into music you can play.

It is a collection of small algorithmic musical world-bubbles. Set up a few
systems, combine them, nudge them, anchor them with a little sequencing, and
perform with the result in real time. You are not filling in a piano roll; you
are giving small rule-based worlds room to surprise you.

## Start here

- [Try the simulator](userdocs/desktop-simulator.md) — make a sound without
  building the hardware.
- [Build the instrument](userdocs/hardware/assembly-manual.md) — assemble a
  standalone Octessera around a Raspberry Pi Zero 2 W or Orange Pi Zero 2W.

The standalone hardware uses one of those two fixed compute-board paths, with
an 8x8 grid, keys, encoders, OLED, and DAC. The desktop simulator mirrors the
same instrument so you can explore it on a computer first.

## What You Can Make

- Grow generative synth patterns from Life, Brain, Ant, Bounce, Raindrops, and
  other cellular-automata behaviors.
- Make hands-on drum sequences, load samples, and send parts to external MIDI
  gear.
- Layer up to eight instruments and let grid motion reshape another layer's
  pitch, filter, velocity, or probability through modulation.
- Route instruments directly or through FX buses, then shape the shared mix with
  bus and global effects.
- Use **Keys** to play a line live, and use **Play** pages to mix, pan, change
  trigger probability, move XY controls, and punch in momentary effects.

A first session might be a drifting `life` pattern under a hand-built drum
sequence, with a sampler providing the hits, a bus effect making room for the
synth, and a live Keys layer nudging the whole thing somewhere new.

## Two reading guides

- [User guide](userdocs/README.md) — simulator, assembly, setup, music-making,
  controls, and reference pages.
- [Technical guide](docs/README.md) — native architecture, contracts, fixed
  hardware paths, build tools, and provenance.

## Notes

[License](LICENSE) · [Sample source](samples/SOURCE.md) ·
[Hardware attributions](hardware/ATTRIBUTIONS.md) ·
[Third-party notices](THIRD_PARTY_NOTICES.md)
