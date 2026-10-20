# Octessera

Octessera turns cellular automata into music you can play.

It is a collection of small algorithmic musical world-bubbles. Set up a few
systems, combine them, nudge them, anchor them with a little sequencing, and
perform with the result in real time. Instead of filling in a piano roll, you
are giving small rule-based worlds room to surprise you, and then deciding
which surprises to keep.

## Start here

- [Try the simulator](userdocs/desktop-simulator.md): make a sound without
  building any hardware.
- [Build the instrument](userdocs/hardware/assembly-manual.md): assemble a
  standalone Octessera around a Raspberry Pi Zero 2 W or an Orange Pi Zero 2W.

The standalone instrument is built on exactly one of those two boards, with an
8x8 light-up grid, four keys, encoders, an OLED, and a DAC. The desktop
simulator mirrors the same instrument, so you can explore it on a computer
before you warm up the soldering iron.

## What you can make

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

- [User guide](userdocs/README.md): the simulator, building and setting up the
  hardware, making music, controls, and reference pages. Start here if you want
  to play or build one.
- [Technical guide](docs/README.md): architecture, contracts, the two fixed
  board paths, build and deployment tools, and provenance. Start here if you
  want to change the code.

## Notes

[License](LICENSE) · [Sample pack](https://github.com/stargatedaw/stargate-sample-pack) ·
[Hardware attributions](hardware/ATTRIBUTIONS.md) ·
[Third-party notices](THIRD_PARTY_NOTICES.md)

This project was created with AI assistance. Octessera itself, however, does not contain any AI.
