# Samples

The bundled library is about 300 WAV files from the attribution-free
[stargate-sample-pack](https://github.com/stargatedaw/stargate-sample-pack).
To add your own WAVs (up to about two minutes of 44.1 kHz stereo, or twice
that in mono, so the little board doesn't run out of memory):

| Where | Folder |
|---|---|
| Desktop, Windows | `%APPDATA%\octessera\samples` (the browser's `User data` favorite) |
| Desktop, Linux | `~/.local/share/octessera/samples` |
| Raspberry | `/home/pi/samples` (copy over SSH) |
| Orange | `/var/lib/octessera/samples` |
| OLED microSD card (SD2) | `octessera/samples` on the card |

SD2 is the optional card in the OLED board; the compute board's boot card is
SD1. Give it a single FAT32 or exFAT partition (or format the whole card that
way); the name doesn't matter, and other partition types are ignored. You can
swap the card while the instrument is on; give it a couple of seconds to show
up. It appears in the sample browser as
the `SD card` favorite. Just let any transfer or save finish before you pull it;
a card yanked mid-write gets a quick automatic check the next time it goes in,
but it can't bring back a half-written file. A patch that uses samples from the
card picks them up again once the card is back, so a patch loaded before the
card mounted, or a quick swap, doesn't leave those slots silent.

To copy files from a computer without pulling the card, use
**System > Connections > SD Card 2 > Start Transfer** in USB Gadget mode
(Raspberry) or on the Orange. USB audio, USB MIDI and recording need to be off
first. Transfers are slow, about 0.75 MB/s, so a big sample pack is a good
moment for a cup of tea. When you're done, eject the drive on the computer
first, then press **Back** or click **Main** on the instrument to end the
transfer. Don't pull the cable while files are still copying.
