# Performance and live shows

How much a patch can hold, how quick the instrument feels, and how to keep the
board quiet in front of an audience.

## Practical performance

These are planning targets for dense patches, not hard limits:

| Board and Perf. Mode | Synth voices | Sample voices | Bus FX | Global FX |
|---|---:|---:|---:|---:|
| Raspberry, Latency | 32 | 16 | 8 | 2 |
| Raspberry, Capacity | 48 | 32 | 8 | 2 |
| Orange, Latency | 48 | 24 | 8 | 2 |
| Orange, Capacity | 48 | 32 | 12 | 2 |

Voice counts are totals across all eight instrument slots. Behaviors, samples
and effects all eat into the headroom, and **System > Audio > Polyphony** can
steal older voices as load rises. The menu will happily let you fill all
twelve bus FX slots on either board; this table is the polite suggestion.
The synth column comes from a load test with Polyphony off and the factory
patch playing underneath: the Raspberry held 40 synth, FM and Plucked notes
cleanly in Latency mode and started dropping out at 48; in Capacity mode, and on
the Orange in both modes, it reached 64, the engine's synth voice limit, without
a dropout. The table keeps about a quarter of that as headroom. The sample
column is still an estimate.

How quick does it feel? With `System > Audio > Perf. Mode` set to Latency, the
audio buffering adds about 4 ms on the Orange and 9 ms on the Raspberry, plus a
couple of milliseconds to read the pad. Capacity renders ahead so it can share
the work across two cores, which brings the buffering to about 12 ms on the
Orange and 17 ms on the Raspberry. That's fine for most playing, though you may
feel it on fast finger drumming. Changing the mode restarts just the Octessera
app, not the whole board: a few seconds of silence and you're back.

## Playing in front of people

The board images skip the operating system's daily package-list and
manual-index jobs, plus a few other chores that do nothing useful on an
instrument. They never installed anything here anyway, but on the Raspberry
they made the audio stutter for a few seconds at a random time each day.

A handful of useful housekeeping jobs still run on a timer: log rotation, SD
card trim, temp-file cleanup and, on the Raspberry, a once-a-minute Wi-Fi
health check. Each has been run during playback without disturbing the sound,
but you can postpone them with **System > Live Show Mode > Start 48h** before
a show:

- It asks first, because it also switches Wi-Fi off. No SSH, no updates and no
  Wi-Fi setup until it ends, and nothing on the network can poke the board
  mid-song. Bluetooth keyboards and speakers keep working.
- Housekeeping is postponed for 48 hours of switched-on time. A reboot doesn't
  cancel that, and time spent switched off doesn't count, so a show packed into
  its case overnight picks up where it left off. The jobs run at their next
  scheduled time afterwards.
- Wi-Fi comes back by itself when the 48 hours are up. **End** finishes early
  and brings Wi-Fi straight back.
- A couple of tiny chores always keep running because things break without
  them: saving the clock and, on the Orange, trimming the in-memory logs.

To keep Wi-Fi off for good, set **System > Connections > Wi-Fi > Wi-Fi** to
Off. It stays off through reboots until you switch it back On, which also
means no SSH and no updates. Your saved network stays put, so it reconnects
straight away when you turn it back on. It's a separate switch from Live Show
Mode: a show still takes Wi-Fi down while it runs, and ending a show never
turns Wi-Fi on if you'd switched it off. Opening the Setup Portal does turn it
back on, since that's what you asked for.

The Raspberry's four cores share one fairly slow memory bus, so anything heavy
happening next to the music can cause crackles even when it runs on another
core. Do maintenance before the show, not during it: updates, `apt` over SSH,
copying big files onto the board. The Orange has a lot more memory headroom
and shrugs most of this off, but the habit doesn't hurt.

Plug the HDMI screen in before you start playing. On the Orange, plugging it in
or pulling it mid-song makes the board stop and read the screen's details,
which can nudge the timing by a few milliseconds for a moment.

One more thing: don't run `apt upgrade` on the board. Octessera boots its own
tuned kernel and device-tree setup, and a stock kernel or bootloader package
sneaking in can leave you with a board that boots but makes no sound, or
doesn't boot at all. The boards hold those packages so apt leaves them alone;
please don't go fishing with `--allow-change-held-packages`. Octessera updates
come through **System > Maintenance > Updates** or a fresh image.
