# Octessera user manual

Octessera is a collection of small algorithmic musical world-bubbles. Set up a
few systems, nudge them, anchor them with a little sequencing, and play the
result together with the machine.

## 1. Try the simulator

Start with the [desktop simulator](desktop-simulator.md) if you want to make a
sound before building anything. It needs no PCB, board, OLED, or soldering iron.

## 2. Build the device

Choose a Raspberry Pi Zero 2 W or Orange Pi Zero 2W. Use the matching board
image and enclosure top.

[Build and assembly manual](hardware/assembly-manual.md) · [Safety and
power](hardware/safety-and-power.md)

## 3. Flash and set it up

[Flash and first-boot guide](hardware/flash-and-first-boot.md) — matching image,
flashing, first boot, and network setup.

[USB roles](hardware/usb-roles.md) — host-computer connections.

## 4. Make music

1. In **Build**, choose `life`, `brain`, or `raindrops` for a layer.
2. Draw a few cells on the grid.
3. In **Shape**, choose a **synth**.
4. Press **Play** or **Space**.
5. Use **Play Mix** to adjust the layer's level, then explore the other Play
   pages.

[Controls cheat sheet](controls-cheat-sheet.md) · [Behaviors and Play
pages](behaviors-and-sparks.md) · [Recording](recording.md)

## Reference

### Practical performance

Use these as planning targets for dense patches:

| Mode | Synth voices | Sample voices | Bus FX | Global FX |
|---|---:|---:|---:|---:|
| Raspberry Latency | 16 | 16 | 8 | 2 |
| Raspberry Capacity | 32 | 32 | 8 | 2 |
| Orange Latency | 24 | 24 | 8 | 2 |
| Orange Capacity | 64 | 32 | 12 | 2 |

Voice counts are totals across all instrument slots, not per-instrument limits.
Adaptive voice stealing may reduce the active synth count as load rises. These
are practical targets rather than guarantees; behaviors, samples, and effects
all change the available headroom.

- [Controls cheat sheet](controls-cheat-sheet.md)
- [Behaviors and Play pages](behaviors-and-sparks.md)
- [Recording audio and OLED](recording.md)
- [Safety and power](hardware/safety-and-power.md)
- [USB roles](hardware/usb-roles.md)
- [Data backup and restore](data-backup-restore.md)
- [Troubleshooting](troubleshooting.md)
- [Printable quick reference](print/quick-reference.pdf)

### Samples

The default library has 320 media files: 318 WAV files available to the
sampler, plus two AIFF files outside the WAV-only browser. You can add your own
samples through the desktop sample browser or the board sample paths.

For the optional OLED microSD card, label it `OCTESSERA_SD`. This is SD2; the
selected board's boot card is SD1. Put WAV files under `octessera/samples`. If
you use **System > SD Card 2 > Start Transfer**, eject the host drive before
pressing **Back** or **Main** to stop the transfer.
