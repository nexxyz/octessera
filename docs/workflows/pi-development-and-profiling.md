# Pi development and profiling

This page covers host-only Pi builds and board profiling. It does not replace
the ordered Orange bring-up procedure or the [deployment runbook](deployment.md).

Canonical board IDs are `raspberry-pi-zero-2w` and `orange-pi-zero-2w`; their
images, pinouts, port roles, and deployment adapters are not interchangeable.
See [`../board-profiles.md`](../board-profiles.md) for feature owners and
supported board IDs.

## Builds without hardware

Host-stub Pi app build:

```bash
cargo build -p octessera-pi
```

Hardware HAL target check when the Rust target is installed:

```bash
cargo check --target aarch64-unknown-linux-gnu -p octessera-hal --features raspberry-pi-zero-2w
```

There are two cross-builders:

- `tools/pi/build-rpi-opi-cross.ps1` builds `octessera-pi` for either board
  (`-BoardProfile`, default Raspberry) into `target/pi-cross/` or `-OutDir`.
  Raspberry deploys use it.
- `tools/orange-pi/build-opi-cross.ps1` is WSL Docker-only and writes Orange
  artifacts under `target/orange-pi-cross/` (the smoke tools with their
  metadata sidecars).
  Orange deploys use its `-Profile release` output.

```powershell
./tools/orange-pi/build-opi-cross.ps1 -Binary orange-oled-smoke -Profile release
./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release
./tools/orange-pi/test-build-opi-cross.ps1
```

The Orange builder never contacts a board. Cargo and rustup caches live in
persistent named Docker volumes, and `-DryRun` prints the command without
starting Docker. It builds the two diagnostic smoke binaries and a local
Orange `octessera-pi` development binary, not a production image or its
`production-runtime` bundle.

## Pi UI and audio profiling

Pi UI/render profiling is quiet by default. Enable summaries with either form:

```bash
OCTESSERA_PI_UI_PROFILE=1 octessera-pi
octessera-pi --profile-ui
```

Summaries include loop cadence, runtime tick lateness/advance, render overruns,
snapshot/config sync, hardware polling, and LED/NeoKey/OLED phase timings.

Use Pi-side probes for rhythmic timing, trigger latency, audio-drain latency,
and DSP budget questions. PC/runtime-only probes do not measure hardware audio
timing. `tools/pi/run-pi-timing-probes.ps1` is Raspberry-only; never point it
at Orange.

The timing probes and the offline DSP profile (`--profile-dsp`) live only in the
instrumented `profiling` build; production images and fast deploys leave them
out and refuse those flags. Build one with
`./tools/pi/build-rpi-opi-cross.ps1 -Profiling` (writes
`target/pi-cross-diagnostics/profiling/octessera-pi`) or, on the Orange,
`./tools/orange-pi/build-opi-cross.ps1 -Binary octessera-pi -Profile release -Profiling`
(`target/orange-pi-cross-diagnostics/profiling/`). Copy it to the board and pass
its path with `-Binary`. On the Orange, run the DSP profile by hand with the
service stopped: `sudo systemctl stop octessera.service`, then
`<binary> --profile-dsp`, then start the service again. The 10-minute
`board-timing-smoke.sh` runs the production binary; only its load ramp needs the
profiling build.

```powershell
$PiTarget = "pi@<PI_HOST>"

# Runtime-only probe; leaves service and live audio untouched.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode RuntimeOnly -Durations 15s -Scenarios idle,pulses-stress -WakeIntervalsMs 2,4,6,8,10,12 -Snapshots

# Live-audio probe.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode Live -AllowServiceInterruption -Durations 10m -Scenarios idle -WakeIntervalsMs 2,4,6,8,10,12

# Audio-source drain latency probe.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode AudioDrain -AllowServiceInterruption -Durations 10m

# FX budget profile.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode DspFxLimits -AllowServiceInterruption

# Alternate render quantum.
./tools/pi/run-pi-timing-probes.ps1 -Target $PiTarget -Mode DspFxLimits -AllowServiceInterruption -AudioRenderQuantumFrames 256
```

RuntimeOnly leaves the service running; Live, AudioDrain, and DSP modes stop
`octessera.service`, restart it afterwards, and therefore need
`-AllowServiceInterruption`. Use `-PrintOnly` to see the remote command first. Both Pi modes pass the actual
elapsed interval to the shared runtime. These metrics diagnose trigger cadence
and event batching; use `AudioDrain` separately for queue/control-drain
measurements.

## Shared-path timing smoke

Changes to shared audio processing or musical-event emission (bus-FX routing,
general parameter modulation, transport, event dispatch) need a short on-device
smoke on both boards. `tools/pi/board-timing-smoke.sh` plays the board's default
patch with a candidate binary from a cloned store, OLED awake, for 10 minutes by
default (an optional fourth argument sets the seconds). It runs detached on the
board, so no SSH traffic disturbs the measurement. Afterwards it restores the
installed service, checks that the original presets are unchanged, and prints
underruns, pulse cadence, and runtime lateness. Harness processes (payloads,
pollers, key drivers, samplers) must never run on the DSP worker cores 2�3:
the smoke and the scenario payloads run on CPU 0, because polling on 2�3
preempts a worker and shows up as missed Capacity quanta.

```bash
tools/pi/board-timing-smoke.sh rpi "$PiTarget" target/pi-cross/octessera-pi
tools/pi/board-timing-smoke.sh opi "$OrangeTarget" target/orange-pi-cross/octessera-pi
```

Either Orange cross-build output works here. Logs land in
`target/board-timing-smoke/`. Run one board at a time, and compare with a smoke
of the previous build under the same conditions. Do not call a shared-path
change qualified if underruns appear, pulses batch (`max_pulses` above 1), or
cadence drifts from 240-241 dispatches per 5 s. It works on clean production
images.

An individual new instrument or Build behavior can use focused tests and its
own on-device profile unless it changes one of those shared paths. PC-only or
DSP-only results would not have caught a Playing display/control stall, which
is the whole reason this smoke exists.

### Aux/autosave and load studies

The smoke passes extra `OCTESSERA_TIMING_*=value` arguments (after the seconds)
to the candidate, and its cloned store is a valid study store, so the same
script runs the two opt-in studies on either board:

```bash
# Aux timing: scripted Cutoff turns through the real menu, then an automatic save.
tools/pi/board-timing-smoke.sh rpi "$PiTarget" target/pi-cross/octessera-pi 60   OCTESSERA_TIMING_AUTOAUX=1 OCTESSERA_TIMING_KEEP_AWAKE=1
# Load ramp (profiling build only): held notes from 8 to 64 in steps of 8, 20 seconds each.
tools/pi/board-timing-smoke.sh opi "$OrangeTarget" target/orange-pi-cross-diagnostics/profiling/octessera-pi 600   OCTESSERA_TIMING_LOAD=8,8,64,20
```

Raspberry and Orange share one Aux timing driver and one report format; only
the `raspberry-autoaux`/`orange-autoaux` prefix differs. The candidate turns
Auto Save on in the clone through the Saves menu, so the final edit has a due
save, and the report line carries that save's revision and timing. The smoke
prints the report line at the end, and the original presets stay untouched.

`OCTESSERA_TIMING_SEQUENCED_DELAY_US=<microseconds>` (also `-Profiling` only)
replaces the sequenced-output delay the board derives from its audio profile, to
find the smallest delay that still logs no `late timed events`.

`OCTESSERA_TIMING_LOAD=start,step,max,seconds` needs a `-Profiling` candidate;
a production binary refuses to start with it. It ramps held 2-second
notes across Synth, Pluck, and FM through the host musical-event path on top of
the loaded patch, logging one `timing-load step=...` line per step. Every ~0.7 s
it also presses grid tile (0,0) through the real input dispatch and reports
`probe_wait` (arrival to dispatch) and `probe_dispatch_to_audio` (dispatch to
the note reaching the audio service). The smoke clones the board's own patch,
so set it up on the board first and put it back afterwards: a Keys layer with
Link events enabled on the active layer (or the probe reports `probe_silent`),
and Voice Stealing `none`, so stealing does not hide the limit. On any other active layer the probe
press changes the patch (on Life it toggles tile (0,0) and soon breaks the oscillator, so the
background load drops at a random point); for an underrun-only capacity comparison of the unchanged
patch, add `OCTESSERA_TIMING_LOAD_PROBE=0` to skip the probe presses. Correlate the step lines with `underrun occurred` lines and the
`pi-ui-profile` windows in the journal. Add
`OCTESSERA_TIMING_RECORD=1` to run the same study while an Audio+Video take
records (up to 120 minutes) into the board's screen-recordings folder; delete
the take afterwards.

Run boards one at a time when comparing underrun counts, and keep the study
host quiet: a shell loop that spawns processes every second on the board shows
up as rare extra underruns. Sampling over SSH with `sudo` is not quiet either:
each `sudo` wakes D-Bus and systemd, and over a long soak a 20-second sampler
roughly tripled the Raspberry's underrun count and turned a clean Orange into
one that clicked every few minutes. Judge long runs only with sampling off.

### Underrun causes found so far

Each of these took a soak, a trace, and some head-scratching to find. They are
fixed in the images; the notes say what to look for if one comes back.

On the Raspberry, ordinary tasks that wake on the audio core (CPU 1) can take
it from the callback for a few milliseconds: the kernel's fair server reserves
them time even against real-time threads. The network-health job (then once a
minute), D-Bus, systemd launching that job and the hourly cron run were the culprits,
so all four are limited to cores 2 and 3 (`AllowedCPUs=2-3`; systemd's
`init.scope` is set at every boot by `octessera-performance-governor.service`,
as scopes keep no persistent drop-ins). Quiet Latency soaks with 16 held notes
went from about 10 underruns per 2 hours to none, and Live Show Mode stops the
job entirely. If underruns come back, trace CPU 1 and look for the callback
being switched out while still runnable (`prev_state=R`). The same
unit also keeps unbound kernel workers, such as the SD-card writeback behind a
long recording, off the audio core (`/sys/devices/virtual/workqueue/cpumask` =
`d`); a traced recording underrun showed about 60 of their slices interrupting
the callback in its last few milliseconds. Render blocks that take longer than
the audio they produce are logged as `audio block overran: ...` with their
event and voice counts.

The fence moves the same problem to the DSP workers on cores 2 and 3. A
traced 3-hour soak (2026-11) found that every rare `audio quantum missed its
deadline` line was an ordinary task running there through the fair server
while a worker waited, or even preempting a running one: D-Bus and
`bluetoothd` answering the app's Bluetooth status poll (both boards), systemd
starting the network-health job, and `avahi-daemon`. So the work itself is
now rarer: the Bluetooth worker sleeps until BlueZ signals a change and polls
only while scanning or pairing, the Raspberry network-health job runs every
10 minutes, and avahi answers for the hostname over IPv4 only, without the
workstation record or wide-area browsing. Look for a non-worker task between
a worker's `sched_wakeup` and its `sched_switch` on CPU 2 or 3.

The Orange is less exposed but not immune: NetworkManager's dispatcher, which
starts on every DHCP lease renewal (about every 20 minutes on Wi-Fi), D-Bus and
the hourly cron run took its audio core the same way, so D-Bus and the
dispatcher are fenced to cores 2 and 3 there. Cron and journald are fenced to
core 0 instead: a traced soak caught Armbian's `armbian-truncate-logs` job
(every 15 minutes) and journald logging its cron session holding a DSP worker
off core 2 for 4 ms. Core 0 runs only ordinary app threads at a higher
priority and is mostly idle, so the job just waits its turn there. A traced
Orange recording underrun showed an unbound kernel worker (SD-card writeback)
holding CPU 1 for about 2 ms with preemption off, so the Orange image also sets the workqueue mask to `d`
(`/etc/tmpfiles.d/octessera-audio.conf`).

Runtime lateness (the `runtime_late` profile figure) is sub-millisecond and not
audible, but a CPU 0 trace (scheduler, IRQ and hrtimer events) explained it on
both boards in 2026-10. About 50 µs of every wakeup was the kernel's default
timer slack for normal threads, so the app sets its slack to 1 ns at startup.
The Raspberry's worst waits (up to about 0.7 ms) were `avahi-daemon` and
`polkitd` running on CPU 0, so the image keeps both on cores 2 and 3 at
`Nice=10`, low enough for the DSP workers there to preempt them. The Orange's
worst waits were the app's own CPU 0 threads; interrupts took at most about
45 µs on either board. The largest of them was the rolling backup (due on the
first change after start, then every five minutes while the patch stays dirty):
at the runtime's priority its validate, serialize and write held dispatches off
for up to 2.5 ms, so the store worker runs at nice 10.

The Orange keeps Armbian's `ondemand` governor, but its dips to 480 and 720 MHz
(about 200 ms every 2 seconds, right in the runtime's heavy layer steps)
doubled the worst dispatch time. The image sets `MIN_SPEED=1200000` in
Armbian's `/etc/default/cpufrequtils`, which `armbian-hardware-optimize` applies
at boot as the `scaling_min_freq` floor (a separate unit lost that race: the
Armbian script keeps writing in the background after it reports finished). In 2026-11 cloned-store smokes (10 minutes OLED awake, 45 minutes
asleep) the dispatch maximum fell from 1.49 and 1.80 ms to 0.79 and 0.82 ms;
the CPU stayed below 60 °C with no cooling-device steps either way. The
Raspberry runs the `performance` governor at a fixed 1 GHz instead.

The Raspberry's Wi-Fi was the last outside cause. Its SDIO controller
(`mmcnr`, mmc-bcm2835) sometimes hangs mid DMA transfer
(raspberrypi/linux#5770). The kernel logs `Controller never released inhibit
bit(s)`, the shared DMA engine stalls for about 10 ms, and the I2S audio DMA
stalls with it. The `octessera-wifi-pio` overlay withholds the controller's DMA
channel, so the driver copies Wi-Fi data in PIO instead. The Octessera kernel
patch `tools/pi-kernel/patches/0001-mmc-bcm2835-mmc-threaded-irq.patch` moves
that copying from the hard interrupt into an interrupt thread, which the
performance-governor unit pins to cores 2 and 3. Under about 5 MB/s of Wi-Fi
traffic, a 40-minute recording soak went from one or two underruns (with DMA)
or four (with PIO in the hard interrupt) to none. If underruns come back with
Wi-Fi on, check `dmesg` for `inhibit` and `taskset -pc` on the `irq/*-mmc1`
thread.

The audio DMA interrupt can land on any Raspberry core (its affinity cannot
be changed), and the step that wakes the callback often runs deferred, in
that core's `ksoftirqd`. An overnight recording soak traced an underrun to
`ksoftirqd/2` waiting about 3 ms behind the SD-card writeback worker, so the
callback missed two periods. The performance-governor unit gives the
`ksoftirqd` threads `SCHED_FIFO` priority 1, still well below the callback's
70, so deferred interrupt work runs ahead of ordinary threads such as
writeback.

Recording exposed a second, memory-side stall on both boards. Writing a long
take fills the page cache, the kernel compacts memory, and by default it may
move Octessera's locked pages; the callback then waits in state `D` for each
move while `kcompactd` runs. `/etc/sysctl.d/60-octessera-audio.conf` sets
`vm.compact_unevictable_allowed = 0`, the real-time kernels' default, so locked
memory stays put. Raspberry Audio+Video soaks went from 14 to 18 underruns per
2 hours to none. Look for `kcompactd` next to callback `D` states in a trace.

### Tools that found real problems

- Orange has `perf`: `perf record -k CLOCK_MONOTONIC -C 1` on the audio core, or
  `-g -t <runtime tid>` for the runtime thread, lines up with journal timestamps.
- Both boards have ftrace: `sched_switch`/`sched_wakeup` on CPU 1 with
  `trace_clock=mono` separates "callback busy" from "callback not scheduled".
- `/proc/<pid>/task/<cpal tid>/stat` fields 10 and 12 (minor/major faults)
  sampled around playback start or background load show SD-card page faults in
  the audio thread.
- A memory-bandwidth hog on the other cores (copying a 48 MB buffer in a loop)
  starves the Raspberry's audio core; CPU-only and network load barely do.

After live probes, inspect the current boot's service journal:

```powershell
$PiTarget = "pi@<PI_HOST>"
./tools/board-dev/with-board-ssh.ps1 ssh -Board rpi -Target $PiTarget 'sudo journalctl -u octessera.service -b --no-pager'
```

These probes measure timing and audio-path behavior; they do not replace board
bring-up or image checks.

## On-device scenario suite

Before an image build or a release, run the scenario suite on both boards. It
covers what desktop e2e, golden tests, board-check and the timing smoke cannot
see: a real cold boot, persistence across a service restart, a recording, the
OLED's sleep and contrast commands on the SPI bus, and a mixed-load soak. It
tests the installed build, so deploy a candidate first. Both boards may run at
once:

```bash
tools/board-dev/board-scenarios.sh rpi "$PiTarget"
tools/board-dev/board-scenarios.sh opi "$OrangeTarget"
```

Name scenarios to run a subset: `boot save_restore recording oled soak keymap
presets stop_save show_mode update_fail fuzz resets bad_stores full_card usb_midi
usb_midi_cc usb_looper usb_notes_in usb_audio usb_audio_fx`.

- `boot` reboots the board and only reads the live service: kernel, splash,
  service, audio and menu-ready times from the journal and readiness marker, an
  advancing playback stream, no Octessera errors or failed units, and unchanged
  live store hashes.
- The rest run detached (about 42 minutes, no SSH until they end) on the real
  service through a temporary `95-octessera-scenarios.conf` drop-in under
  `/run/systemd/system/octessera.service.d/` (a reboot clears it; it sorts after
  `90-octessera-dev.conf`) and a cloned store. `board-keys.py` types on a uinput keyboard; the Raspberry uses the
  Bluetooth bus, so Bluetooth must be on there.
- `save_restore` changes Grid Bright, Button Bright, Num Style and BPM through
  the menu, restarts, checks the saved files, then turns one more detent to show
  the restarted runtime loaded them.
- `recording` records about 20 s of Audio+Video while the default patch plays
  and checks the AVI with `ffprobe`/`ffmpeg`: frames at 20 fps against the
  recorded time, matching audio length, moving and non-blank frames, non-silent
  audio. The board copy is deleted.
- `oled` sets OLED Sleep to 10 s, wakes the display and turns OLED Bright four
  detents. An ftrace instance on the OLED's SPI command bytes checks that the
  contrast is re-sent after the restart, the display goes off on the timer with
  no frame writes while asleep, comes back on with input, and each detent sends
  the next contrast step.
- `soak` plays the default patch for 25 minutes (`OCTESSERA_SCENARIO_SOAK_SECONDS`)
  with scripted navigation and Aux turns, OLED awake. It needs 0 underruns, 0
  late timed events, 0 missed Capacity quanta, no steady fd growth, and RSS that
  is flat after the first 20 minutes (at most 256 kB of growth over the last 5,
  and not rising at every sample), sampled once a minute from CPU 0. The 25
  minutes are deliberate: memory is locked with `MCL_ONFAULT`, so preallocated
  buffers count toward RSS as they are first touched, and a 90-minute
  measurement showed RSS warming up for 15 to 20 minutes and then staying put.
  A shorter soak only ever sees the warm-up.
- `keymap` turns Auto Save on, empties L1's sequencer and types on the grid
  keys: the bottom row, Left Alt + the leftmost key of each row, and
  Left Alt + `;`. The autosaved tiles must be exactly the world-space set
  (x 0..7, y 0), (0, 4..7) and (6, 5).
- `presets` starts on a clone with presets `A` and `AB` (the store's patch
  with L1 One-Shot Len 301 and 302 ms; BPM would also move the tempo-synced delay
  times the runtime derives from it), Auto Save on and View on Back off (so
  Back presses always land in the menu). A name edit only appends at the end,
  so the names grow rather than change. Without Refresh List it runs Save As
  `AC` (the draft `A` comes from picking `A` in Rename), turns BPM up three
  detents, Loads `A` over that unsaved change, Renames `AC` to `ACD`, Renames
  `A` onto `AB` with the overwrite confirmed, and Deletes `ACD`. After each
  step it waits for the store to change and then stay unchanged for 3 s,
  longer than the 2.15 s autosave coalesce window, so a save still owed from
  the step before is not taken for this step's own. The edit must save
  BPM + 15; after the load the saved patch must have One-Shot Len 301 and hold
  every value of `A`'s `runtimeConfig` (`presets-diff.log` lists any
  difference); `ACD` must be byte-identical to the saved `AC`, and `AB` to the
  original `A`; only `AB` may be left.
- `stop_save` turns Grid Bright up two detents and BPM up three with Auto Save
  on and stops the service as soon as the last key's mark is written: both
  values must be in the saved files after the restart, and systemd's
  `Stopping` line must come within 200 ms of that mark (the `sudo systemctl
  stop` itself is logged too). Then it records 5 s of Audio and stops the
  service: the take is the `.wav` in the Audio recordings folder newer than a
  marker touched just before it, no `.partial.wav` may be left, it must last
  at least 4 s and decode cleanly, and each stop must return in under 12 s.
- `show_mode` starts a show from the menu, rewrites its length to 70 s as the
  runtime would, and bind-mounts the installed `octessera-show-hold-apply`
  with only its step changed to 20 s, so the re-arm runs four times. At 45 s
  (after which it restarts the service) the show must have banked 40 s with
  exactly one release timer armed; at 100 s `length` and `elapsed` must be gone, the radio
  unblocked, the Raspberry's network-health timer active again and no release
  run failed in the journal. A second show ended from the menu must unblock
  the radio within 5 s and leave no `length` and no release timer. The board is off Wi-Fi for about two minutes; the
  cleanup ends any show it left.
- `update_fail` runs this tree's `octessera-update switch` against scratch
  releases beside the clone, with the service's binary pointed at their
  `current` link: `0.9.91` is the binary under test, `0.9.92` a script that
  exits 1, and `0.9.93` a script that ignores SIGTERM and never reports ready.
  It switches to `0.9.91`, back to `0.9.90`, then to each failing one,
  clearing the unit's start limit before each switch (only needed while an
  Orange still has a unit older than 8421caec, which allows 3 starts in 30 s). After
  each switch the links must point at a release that runs, the service must be
  ready within 120 s, the clone's `system.json` and `default.patch.json` must
  be byte-identical, and the message must name the version it ended on. A
  last restart on the release it ended on must load the clone. Each fake
  release also carries `root/usr/local/sbin/octessera-probe` holding its
  version (0.9.91 one extra file), installed into a scratch system root
  (`OCTESSERA_UPDATE_SYSTEM_ROOT`, never `/`) that starts with 0.9.90's probe:
  after `good` the probe reads 0.9.91 and 0.9.90 keeps its probe in
  `root-replaced/` and lists the extra file in `root-added`; after `back`
  the probe reads 0.9.90, the extra file is gone and neither release keeps
  `root-added` or `root-replaced`; after `dies` and `hangs` the probe reads
  0.9.90, nothing 0.9.91 added is left and 0.9.90 has no `root-added`. The journal is
  not checked for errors: the failing releases log them by design. The real
  `/opt/octessera` links are never touched.
- `fuzz` types random taps for 5 minutes on the store's own patch, 600 per
  round, each round seeded with the run's seed plus the round number: grid keys
  with and without Alt, the dials, Back, Play, Shift and Fn chords and short
  holds. Set `OCTESSERA_SCENARIO_FUZZ_SEED` to replay a seed from the summary.
  It ends with one known edit, Grid Bright two detents, which must reach the
  clone: a walk that switched the keyboard off (Bluetooth, on the Raspberry)
  would otherwise make the rest a silent no-op. The drop-in keeps the service away from root's power,
  device-apply, update, setup-portal, Orange storage-control (SD2 transfer)
  and Live Show Mode paths. It needs no panic, no exit other than the restart
  request (75) except a plain exit right after such a refused power request,
  no error line other than the sandbox's refusals, the service active at the
  end, and the clone loading after a restart. RSS is only reported, because it
  warms up for 20 minutes (see `soak`).
- `soak`, `stop_save`, `resets`, `bad_stores` and `full_card` run in the
  `fuzz` sandbox; only in these scenarios' windows are its refusals excused.
- `resets` (Auto Save on, the clone's USB and audio outputs at factory values,
  so Factory System asks root for no device change) runs Empty Patch, then
  Factory Patch, turns Grid and Button Bright up two detents each, runs
  Factory System, declines the restart, and stops and starts the service.
  After Empty Patch every layer must be `none`; after Factory Patch the saved
  patch must hold every non-null value of the patch half of
  `config/generated/pi/default.json`: the runtime fills that file's nulls for
  instrument types a slot does not use, delay times derived from BPM are left
  out, and `sound.optimizeFor` stays with System. After the restart, the changed System values
  must be back at that file's, so the exit save did not write the old ones
  back. Exactly one new `system.json.pre-reset-*` copy must be kept, as a copy
  (its inode is not the old `system.json`'s), and a root loop polling every
  20 ms must never find `system.json` missing. On the Raspberry, Factory
  System turns Bluetooth off and with it the uinput keyboard, so the decline
  is never typed and the dialog stays open; the app's MainPID must be the same
  before and after Factory System either way.
- `bad_stores` starts on four clones. A System file without `midi.notesIn`
  (as saved before the key existed) must load, and a Save System must write
  Notes In on. A System file with an unknown top-level key, one with an
  unknown key in `runtimeConfig`, and a patch cut to half its bytes must each
  stop at the restore screen: the journal announces it once, systemd does not
  restart the app, and the files stay byte-identical. The screen's choices
  take only the hardware encoders, so the scenario cannot pick one;
  `store_fault/tests.rs` covers what each choice writes.
- `full_card` copies the clone onto a 16 MB ext4 loop image (the stores share
  the root file system; the copy must match), fills it to 48 kB free and, with
  Auto Save on, runs Save As and turns Grid Bright and BPM. Every `.json` in
  the clone must stay non-empty and parseable; the small System file must take
  the new Grid Bright while the 157 kB patch keeps its old BPM, and the journal
  must say `No space left` for the BPM edit. After the filler is removed, the
  retried autosave must write the new BPM and a second Save As must add one
  preset, both in the same process. A recording is not part of it: recordings
  go to the media folders, not the store.

Two UAT cases are not scenarios. Bindings across an FX type change are runner
logic, covered by `a_bus_slot_binding_keeps_the_patch_valid_after_the_slot_type_changes`
and `a_play_fx_binding_keeps_the_patch_valid_after_the_play_fx_type_changes`.
Backup and restore over HTTP needs the transfer code that only the OLED shows;
`user_data_transfer` and `user_data_restore` tests cover the interrupted upload,
the staging leftovers and the restore.

`usb_midi`, `usb_midi_cc`, `usb_looper`, `usb_notes_in`, `usb_hold_trig`, `usb_echo_repeat`, `tempo_mod`, `usb_audio` and `usb_audio_fx` are driven from this
host over the board's USB connection, so plug both boards in and install `python-rtmidi`,
`sounddevice` and `numpy` (`pip install python-rtmidi sounddevice numpy`);
`ffmpeg` decodes the recording. The payload waits at a gate for each host
measurement, and SSH is used only between measurements. Run one board at a
time: Windows names neither board in its capture device (`Octessera Audio`
on the Orange, `Octessera Line In` on the Raspberry).

- `usb_midi` sets external sync, Clock In, Follow S/S and Send MIDI all inst.
  in the clone. L1 steps a Drum on every bottom tile at 1/8 (its kick is
  mirrored on channel 1), L2 a MIDI Drum on channel 10 with two stamped tiles
  and a 150 ms Duration. `host_usb_midi.py` sends Start and 24 PPQN clock at
  100 BPM for 120 s, 140 BPM for 60 s, then Stop, and stamps every incoming
  message with `perf_counter` in the loop that sends the clock. It needs
  300 ± 2 ms between notes at 100 BPM, under 2 ms of drift against the host
  clock, 214.29 ± 2 ms from one bar after the tempo change, 100 ms mirrored
  and 150 ms MIDI Drum notes (± 3 ms), and after Stop every note ended and
  nothing more for the next 2.7 s. A second pass with Swing 4:5 needs the
  offbeat on clock pulse 11 (275 / 325 ms). The journal must show no underruns
  and no late timed events, like the timing smoke. Windows timestamps are good
  to about 1 ms; intervals next to a clock pulse the host itself sent more
  than 1 ms late are skipped and counted.
- `usb_midi_cc` is `usb_midi` with CC1 on any channel mapped to Vel Scale.
  `host_usb_midi.py` sends Start and 100 BPM clock for 36 s with CC1 at 16, 48
  and 96 for 12 s each. From one step after each CC, the mirrored Drum must
  play a single velocity, and the three velocities must rise with the CC
  values. The journal must show no underruns and no late timed events.
- `usb_looper` is the `usb_midi` rig with Auto Save on and L1 an empty 8-step
  1/8 Looper on a Synth with Hold, tile (x, y) playing note 60 + x + 8y.
  `host_usb_midi.py ... looper` sends Start and 100 BPM clock for four passes
  and keyboard notes on channel 1 at 10 ms, early in step 3 and late in step 5,
  and one more 1 s after Stop. Each must sound once live and replay once a
  pass within half a step of its press time plus whole loops; the autosaved
  steps must hold one press and one release per tile, on steps 1, 3, 6 and,
  for the stopped tap, the step the playhead waits on (1).
- `usb_notes_in` is the `usb_midi` rig with slot 1 a Synth with Hold and L1
  stepping slot 5, a MIDI Inst. on channel 1, on a tile that plays note 60.
  The host plays 20 keys, then 10 with every note echoed back after 5 ms,
  holds key 60 (more than 1 s after the last sequenced note 60, since MIDI In
  drops a note-on of a key Octessera itself played within the last second)
  and releases it 2 ms after the sequenced note-off of note 60,
  then plays 10 with an 80 ms echo and sends Stop. Every key needs its mirrored
  note-on and a note-off within 1 s of the release, the echoed keys at most 2
  mirrored note-ons each, the held key its mirrored note-off within 1 s, and
  no note may arrive from 1 s after Stop on. The UAT design's other Notes In
  steps stay with the runner tests, which run the same code without the USB
  path: Notes In off while a key is held
  (`notes_in_off_ignores_notes_and_switching_it_off_releases_held_ones`),
  Stop and Panic while held (`stop_releases_held_keyboard_notes_including_forwarded_midi_ones`,
  `panic_releases_held_keyboard_notes`), and the Sampler's note range
  (`a_sampler_slot_plays_only_notes_36_to_43`).
- `usb_hold_trig` is the `usb_midi` rig with only the store's Life layer: Event
  Triggers on, On none, Hold Trig = note on and Off = note off for slot 1, a
  Synth with Hold, seeded with a diagonal of three cells whose middle one stays
  lit for one step and then dies. (With Event Triggers off a Hold Trig note
  keeps sounding by design: nothing releases a key no Off reaches.) `host_usb_midi.py ... hold_trig` clocks 100 BPM for 6 s. The mirrored
  Hold note must end while the clock still runs, at least 1 s before Stop.
  A case listed in `EXPECTED_FAIL` in `board-scenarios-check.py` reports
  `XFAIL` while it fails and `XPASS` once it passes; neither counts as FAIL.
- `usb_echo_repeat` is the `usb_midi` rig with Notes In on and L1 stepping C4
  every 1/16 on slot 1 (One-Shot Len 150 ms), first a MIDI Inst. on channel 1,
  then a Synth mirrored by Send MIDI all inst. `host_usb_midi_echo.py` clocks
  120 BPM for 4 s while it echoes every note back after 3 ms, then sends Stop
  and echoes for 3 s more: one C4 per step (32) and none after Stop. With the
  MIDI Inst. it then plays, echo off and stopped, 10 key 60 presses 40 ms
  apart, which must all be forwarded, and one key 60 press 300 ms after a
  sequenced C4, which must not be (the 1 s own-note rule), and key 60 held
  for 1 s through a 300 ms echo, which must be forwarded as exactly one
  note-on and one note-off.
- `tempo_mod` plays the store's own patch on the board's internal clock with
  Clock Out on USB MIDI, Link LFO 1 on BPM (1/4, depth 75%) and CC1 on BPM.
  `host_usb_midi.py ... tempo_mod` sends no clock: it sweeps CC1 0-127-0 (254
  messages in about 1 s) four times, 3 s apart, the last sweep stopping at 64,
  and records the board's clock out. From 1 s after the last CC, the clock
  must average the held BPM (40 + 64/127 � 200, about 140.8) within 3 BPM per
  pulse and still swing by at least 20 BPM (the LFO). The journal must show no
  underruns or late timed events and at most 1 pulse per runtime dispatch
  while the host plays (the peak 240 BPM pulses every 10.4 ms, the runtime
  dispatches every 8 ms); the summary also gives dispatch and runtime_late
  avg/max for comparison with the timing smoke. After Stop, the saved patch
  must hold the held BPM within 1, not an LFO value.
- `usb_audio` turns USB Audio on in the clone and rebuilds the USB gadget from
  it (the cleanup rebuilds it from the live store). `host_usb_audio.py` waits
  for the capture device and records 90 s of the store's own patch through
  WASAPI in exclusive mode, because shared-mode capture (ffmpeg dshow) runs
  the Windows microphone effects, which rounded off the clipped tops and
  slowly turned the level down. It needs RMS within 3 dB of the first run's
  baseline (`target/board-scenarios/usb-audio-baseline-<board>.json`), no
  exact-zero run of 64 frames or more while sounding, no full-scale sample and
  no underruns. The first 2 s after the stream opens are reported apart, as
  `startup_gaps`. Then eight synth notes per step pushed past full scale at
  Master Vol 100 must show flat-topped full-scale runs, while an Audio+Video
  recording shows the red `CLIP` in the OLED header; after Master Vol goes
  down to 20, `CLIP` must clear 2 to 3 s after the recording's last clipped
  sample.
- `usb_audio_fx` is `usb_audio` with a tape stop on master mapped to Play FX
  tile (3, 3). With the store's own patch playing, `board-keys.py` opens the
  FX page (Shift+Ctrl+`/`, that is Shift+Fn + the right column's bottom tile)
  and holds key `7` for 6 s while `host_usb_audio.py ... fx` records 32 s. The
  level must fall at least 20 dB below the take's opening level for 3 s or
  more and come back within 3 dB of it after the release, with no underruns
  while held.

Each run writes its evidence and a `summary.txt` of PASS/FAIL lines to
`target/board-scenarios/<board>-<stamp>/`. Afterwards the payload stops any
`board-keys.py` or updater switch it started, restores the service, removes
the drop-in, clone and recordings, and checks the live store (every file in
it, `patches/` and `backups/` included). Every run also gets a `restore`
check: every payload launched must leave a last line reading `service=active
live_store_unchanged=yes dropin_left=no gadget_restored=yes clone_mounted=no`.
