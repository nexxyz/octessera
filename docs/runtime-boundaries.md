# Runtime Boundaries

This contributor contract keeps UI, host adapters, native runtime logic, core
behavior logic, and audio rendering in separate layers. When a change makes you
wonder where something belongs, this page should answer it; if it doesn't, the
page needs fixing, not the layering.

The authoritative menu/control behavior spec is
[`menu-and-controls-spec.md`](menu-and-controls-spec.md). Board-specific facts
are in [`board-profiles.md`](board-profiles.md).

## Board runtime loop

- `apps/pi-zero/src/hardware_runtime_scheduler.rs` owns Raspberry and Orange timing policy. See [Runtime scheduler](#runtime-scheduler).
- Both boards run one runtime loop (`apps/pi-zero/src/runtime_loop.rs`) on an
  `octessera-runtime` thread with one `PiHostAdapter`. Between scheduled ticks
  the loop blocks on the grid/NeoKey/keyboard input channel, so live presses
  dispatch on arrival instead of at the next tick. Encoders still use the tick;
  under an external clock the loop checks incoming MIDI at least every 1 ms.
- At startup both board binaries `mlockall` with `MCL_ONFAULT` and lock their
  own and their libraries' read-only mappings up front, so memory pressure or
  first use cannot page audio-path code in from the SD card. The services grant
  `LimitMEMLOCK=infinity`.
- Per-board hooks cover only audio upkeep (Orange recovery, health, and
  readiness; Raspberry load status and Jack fault) and what a power request
  does.

### Runtime scheduler

`apps/pi-zero/src/hardware_runtime_scheduler.rs` owns Raspberry and Orange
timing policy: a wake exactly when the next internal pulse is due, 8 ms
realtime work as the ceiling for UI/XY/snapshot work, 50 ms stopped maintenance, 33 ms playing
snapshot cadence/publication gating, native one-shot and continuous display
deadlines, bounded wake/input pickup, accepted-revision dedupe/retry,
configured sync-source requests, and playing input-snapshot suppression. It
owns no board, hardware, audio, adapter, or thread.

## Board thread map

`main` pins itself to CPU0 (affinity only, `audio_priority::pin_main_thread_to_cpu0`) before it
spawns anything, so every thread below inherits CPU0 at normal priority unless the last column
says otherwise. CPU placement and priorities apply on the boards only; host builds skip them.
Unnamed threads have no `Builder::name`.

| Thread | Spawned by | Channels | Stops | Priority / CPU |
| --- | --- | --- | --- | --- |
| `octessera-runtime` | `apps/pi-zero/src/runtime_thread.rs`, from both board startups | input `HostMessage` channel, platform result lane, audio control requests | runtime loop returns; `main` joins it | inherited CPU0; 8 MiB stack |
| Jack callback (cpal) | cpal stream, `audio/audio_output/cpal_audio_output.rs` | engine event queue into `EngineSource` | stream dropped | `SCHED_FIFO` 70, CPU1, set on its first callback |
| USB/HDMI mirror callback (cpal) | cpal mirror stream, `cpal_audio_output/cpal_audio_mirror.rs` | PCM mirror ring | stream dropped | `SCHED_FIFO` 60, CPU0 |
| `oct-dsp-tree-0`, `oct-dsp-tree-1` | `realtime-engine` `source_worker/lifecycle/worker.rs` (routing tree, Capacity mode) | work and completion mailboxes | work channel closes at engine shutdown | `SCHED_FIFO` 70, CPU2 / CPU3 (`pi_worker_start_hook`) |
| `oct-src-reaper` | `rodio-engine-source` `source_worker_reaper.rs` | bounded retired-audio and shutdown-envelope channels | after the shutdown envelope | inherited |
| optional audio recovery (unnamed, USB and HDMI) | `audio/audio_output/audio_optional_recovery.rs` | `SelectedOutputs` change wait, PCM mirror producer | stop flag and join in `Drop` | inherited |
| `octessera-audio-prep` | `host_audio_prep.rs` | `AudioControlRequest` in, `HostMessage` results out | request channel closes | CPU2/3, normal priority |
| `octessera-wav-recorder`, `octessera-avi-recorder` | `media-recording` `service.rs` | recording tap chunks, OLED ingress | stop flag on stop or `Drop`; finishes the take | CPU2/3 (`with_worker_start`) |
| `octessera-avi-repair` | `audio_recording.rs` `spawn_partial_take_repair` | none | after the last partial take | CPU2/3 |
| `octessera-signal` | `playback-runtime` `native_runner/signal_view/analysis.rs` | mutex slot and condvar | stopped flag in `Drop`, seen within a second | `SCHED_IDLE`, CPU0 |
| `octessera-midi-out` | `playback-runtime` `timed_midi_sender.rs`, created by `midi_host.rs` | mutex inbox and condvar | `Drop` marks it stopped | `SCHED_FIFO` 50, CPU0 |
| `octessera-midi-connect` | `midi_host.rs` | connect commands in, connection events out | command channel closes | inherited |
| midir input (midir) | midir, per connected input | input handler into the runtime | port closed | `SCHED_FIFO` 50, CPU0, on its first message |
| render worker (unnamed) | `render_loop.rs` `RenderWorker::spawn` | mutex render state and condvar, acknowledgement channels | `RenderCommand::Shutdown` | inherited |
| Seesaw worker (unnamed) | `seesaw_io.rs` | `HostMessage` input out, `SeesawCommand` in | Orange: shutdown request once the LEDs are dark; Raspberry: process lifetime | inherited |
| Seesaw health monitor (unnamed, Raspberry) | `seesaw_io/raspberry_worker.rs` | scan generation counter | never; exits the process after a stalled scan | inherited |
| `octessera-usb-keyboard` | `keyboard_capture/mod.rs` | `HostMessage` input out | stop flag and join on shutdown or `Drop` | inherited |
| `orange-oled-suspend` (Orange) | `orange_oled_suspend.rs` | Unix socket, render worker handle | stop flag; `shutdown` joins | inherited |
| `octessera-orange-<encoder>` (Orange) | `octessera-hal` `orange_encoder_gpio.rs` | `HardwareEvent` out | stop flag in `Drop` | inherited |
| rppal encoder interrupts (unnamed, Raspberry; one each for A, B and the switch of every encoder) | rppal `set_async_interrupt`, from `octessera-hal` `encoder_gpio.rs` | `HardwareEvent` out | pin dropped with its `EncoderGpio` | inherited |
| rppal Seesaw INT interrupt (unnamed, Raspberry) | rppal `set_async_interrupt`, from `octessera-hal` `seesaw_interrupt.rs` | edge signal to the Seesaw worker | pin dropped with its `SeesawInterrupt` | inherited |
| platform worker (unnamed) | `platform_service/platform_service_worker.rs` | `PlatformWorkItem` in, platform result lane out | job channel closes | nice 10, CPU0 |
| `octessera-device-update` | `platform_service/platform_update_lane.rs`, started by the first update job | update job channel in (one job at a time; a request while one runs is refused), platform result lane out | update job channel closes with the platform worker | nice 10, CPU0 |
| `octessera-setup-portal` | `setup_portal_worker.rs` | platform result lane | stop flag | inherited |
| `octessera-user-data-transfer` | `user_data_transfer/session.rs` | TCP listener, shared transfer state | stop flag; `stop` joins | inherited |
| `octessera-user-data-restore` | `user_data_transfer/restore_worker.rs` | shared transfer state | after its one restore | inherited |
| `octessera-bluetooth` | `bluetooth/mod.rs` | `WorkerEvent` in, `HostMessage` status out | event channel disconnects; holds its own sender, so in practice the process lifetime | inherited |
| `octessera-bt-agent`, `octessera-bt-signals` | `bluetooth/bluez.rs` | D-Bus signal streams; signals forward `WorkerEvent::Changed` | signals: worker channel closes; agent: D-Bus connection ends | inherited |
| pair / connect (unnamed, per request) | `bluetooth/bluez.rs` | `WorkerEvent` result out | after its one D-Bus call | inherited |
| `octessera-bt-audio` | `bluetooth/monitor.rs` | recording chunks in | stop flag in `Drop` | `SCHED_OTHER`, nice 10, CPU2/3 |

## Layer Responsibilities

- UI layer (`apps/desktop/src/`)
  - renders runtime snapshot data
  - captures user interaction and emits `DeviceInput`
  - contains no transport/menu/audio/interpretation logic

- Runtime orchestration layer (`crates/playback-runtime`, `apps/desktop/src-tauri/src/runtime_worker.rs`)
  - owns lifecycle (`start`/`stop`)
  - schedules transport pulses and realtime status through Rust runtime code
  - owns native menu state, config payloads, snapshots, platform effects, and `NativeRunner`
  - menu platform actions are the typed `PlatformAction` enum (`native_menu/platform_action.rs`); its wire string form (`platform_action/wire.rs`) exists only for saved Aux, Shift-Aux and Faders-tile click bindings (`{"kind":"platform_effect","action":…}`) and help keys, and an unknown saved action fails schema validation
  - builds the startup menu once: hosts construct `NativeRunner::new_with_unbuilt_menu`, apply the synchronous System and saved-patch store results, then call `finish_startup_load`; loads applied before that skip their own menu builds
  - owns schema-v2 global modulation state, legacy migration, keyed Link LFO menu/binding paths, canonical global Play XY serialization, and transient global-LFO endpoint composition; it emits resolved audio commands while the realtime engine owns rendering
  - owns the schema-v2 canonical device audio contract: atomic `AudioOutputSet` values for Jack, USB, and HDMI output preferences, the Raspberry-only `UsbDataRole` Gadget/Host contract, Host validation that disables USB Audio and USB MIDI, whole-set validation, strict canonical `audioOutputs` and USB field validation, and next-boot apply confirmation; adapters open the selected exact physical routes under this native contract
  - owns the typed schema-v2 DSP runtime configuration and native `System > Audio > Engine` menu; it emits direct coalescible DSP audio commands without rebuilding prepared audio or changing structural render-plan revisions
  - stamps audio configs, owners, and safe controls with narrow generations and momentary instance epochs; see [Generations and epochs](#generations-and-epochs)
  - `PlaybackRuntime::dispatch` is the canonical host-message/result observation path; desktop and Pi loops schedule work and render its presented output
  - consumes typed runtime-config changes published by `NativeRunner` during canonical dispatch; hosts must not derive playback scheduling config from snapshots
  - maps typed adapter failure facts to recovery policy and owns the best-effort stop-and-silence operation
  - requires every host adapter to implement internal-audio silence and external-MIDI panic explicitly; safety cannot fall through to a successful no-op
  - classifies worker emission and persistence faults as retain/retry outcomes instead of safety-stop failures
  - applies native core behavior transitions through `platform-core`
  - owns the foldered Note Mapping presentation and selection actions while keeping `pitch.scale` as the persisted wire field; binding and modulation pickers consume the core registry's flat IDs
  - publishes snapshots, including authoritative transient indicator fields and unscaled semantic NeoKey colors, plus platform effects, audio commands, MIDI events, runtime status, and native-owned modal frames
  - owns OLED animation detection and timing with the display transients (`native_runner/display_animations.rs`): each capture is compared with the one before, and the resolved step travels in the presentation input (`oledAnimation` in the snapshot); the shared `oled_frame` renderer composes the in-between frame for desktop and both boards. Animations are presentation only and never gate input, menu or audio work
  - owns revisioned OLED frame production; Raspberry and Orange render from the native presentation scene instead; see [OLED frames](#oled-frames)
  - requires a serialized `display.bodyLayout` (`rows`, `list`, or `card`); `oled_frame` owns the text layout and fonts; see [OLED body layout](#oled-body-layout)
  - owns monotonic wall-clock deadlines for the four 100 ms event-dot fade steps and 90 ms beat/measure flash; hosts request timed expiry snapshots and only scale native NeoKey RGB values
  - owns setup-portal menu confirmation, playback stop/reset and note cleanup, the typed `RuntimePlatformEffect::SetupPortalOpen` and `SetupPortalClose` effects, and all typed setup status/modal presentation
  - owns live Play trigger-gate semantics. Disabling a layer immediately drains held notes owned by that layer from internal Synth, FM, Plucked, and Sampler paths and external MIDI, leaves other layers untouched, suppresses future triggers, and does not resurrect released notes when re-enabled. Already-ringing one-shot Drum hits are not held notes and finish their own decay.
  - owns the portable Data Backup/Restore bundle contract: canonical patch migration and validation, the allowlisted preference delta, manifest/hash metadata, and typed transfer status; it does not perform transfer or filesystem I/O
  - delegates its display-ordered grid index API to the pure fixed-grid projection helpers in `platform-core`; it does not own a second projection table or geometry rule
  - owns MIDI input/output through host adapters only; Tauri/midir and Pi MIDI device access stay outside canonical runtime crates
  - passes a runner note-off to internal audio and MIDI only while that channel and note still sound on its lane (held, or within a one-shot's duration), so scanned-empty and other note-off triggers do not flood silent notes; a stop-and-silence recovery forgets every sounding note
  - owns musical time in the host `Instant` domain and the sequenced output delay; see [Musical time](#musical-time)
  - `TimedMidiSender` sends each MIDI output from its own thread, holding timestamped messages until due; see [MIDI output thread](#midi-output-thread)

- Core logic layer (`crates/platform-core`)
  - deterministic behavior execution, grid state, interpretation, mapping, transforms, and native layer engine logic
  - owns the typed 25-entry note-set registry, category tree metadata, canonical flat IDs, roots, intervals, and root-relative pitch-class expansion; unknown note-set IDs are not expanded
  - generated platform capability constants from `resources/platform-capabilities.json`
  - checked-in generated display palette constants from `resources/display-palette.json`, copied into `OUT_DIR` by the `platform-core` build script, so runtime, Pi, and desktop adapters share color values without moving UI policy into the core
  - owns the fixed 8×8 logical row-major index and logical/display tile and index projection helpers; the lower-left world-space rule is pure native logic, not a runtime projection table
  - no UI framework code
  - no platform-specific I/O
  - no desktop, Pi, Tauri, storage, MIDI-device, filesystem, or hardware adapter code

- Output adapters (`apps/desktop/src-tauri/src/`)
  - desktop audio sink maps native events/audio commands to the realtime engine and rodio source
  - desktop and board adapters hand runtime audio commands to the shared `playback_runtime::apply_runtime_audio_command` and implement only its transport; see [Audio commands](#audio-commands)
  - desktop `AudioRuntime` retains the rodio `Sink` and contains engine panics in `ContainedAudioSource`; see [Desktop audio source](#desktop-audio-source)
  - the test-only `headless` feature (`apps/desktop/src-tauri/src/headless.rs`) wires the same runtime, host adapter, sample preparation and recording tap without Tauri or a sound card, pulls the engine's audio itself and advances the runtime by exactly the audio it rendered; it adds no runtime behavior and is not built into the app
  - MIDI input/output uses Tauri-side midir adapters
  - storage and sample-browser filesystem access remain host adapter responsibilities; both hosts write store JSON through `crates/store-files` (atomic replace, rotated `backups/bak-<millis>.json` keeping the newest 20, JSON file read) and keep their own store layouts
  - desktop and Pi retain sample path resolution/containment, caching, preparation, preview/bank orchestration, and typed error adaptation
  - Pi setup adapters own the board-side setup marker and current-status file seam; they do not execute the root coordinator or inspect setup credentials. Data Backup/Restore is not started or advertised by `Setup Portal`.
  - `System > Maintenance > Backup / Restore` is a separate native action; Pi adapters bind its service, desktop returns typed unsupported; see [Backup and restore service](#backup-and-restore-service)
  - device-update effects run through `/usr/local/sbin/octessera-update` on both boards; `NativeRunner` owns menu/action semantics; see [Device updates](#device-updates)
  - returns typed failure facts and carries runtime request/revision identity through asynchronous platform/audio-prep jobs; it does not choose recovery policy
  - Pi `AudioManager` owns the bounded `audio_load_status_channel` sender source; see [Audio load status](#audio-load-status)

- Realtime audio engine (`crates/realtime-engine`, `crates/rodio-engine-source`)
  - owns all internal musical audio rendering, instrument route/pan, FX bus sends, FX bus processing, sidechain ducking, and final stereo mix
  - two executors: the inline path (desktop and Latency mode) and the routing tree (Capacity mode on the boards); see [Executors](#executors)
  - the routing tree keeps one `OwnerEnvelope` per worker; `SourceWorkerLifecycle` owns fault mailboxes, joins, and final owner destruction; see [Routing-tree ownership](#routing-tree-ownership)
  - generates synth slot/sample/pan constants from `resources/platform-capabilities.json`, and its build script generates the 4096-step sine table behind `src/sine.rs`. Oscillators, LFOs, panning, and crossfades use that table; filter and EQ coefficient design keeps `f32::sin`/`cos` for precision near DC
  - the block renderer picks each voice's source (Synth, FM, Plucked, Drum) once per block and runs a loop specialized for that source (`synth/engine/render_voice`); the single-frame path shares the same per-source sample functions, so frame and block rendering stay bit-identical
  - `crates/realtime-engine` owns the `EngineEvent` set that hosts send to the engine; `crates/playback-runtime` maps validated runtime audio commands onto it
  - `crates/rodio-engine-source` owns the engine event queues, shared non-realtime file opening and WAV decoding into `SampleBuffer`, plus refill, previous-master cache repeat/silence, and overload status publication
  - stores the typed DSP bus-idle configuration received through direct control events, and reports persistent-worker utilization only from valid paired completions; the CPU Warn threshold is read by the OLED CPU meter alone, and neither setting alters prepared audio topology
  - `EngineSource` receives prepared sample buffers and control events; shared file opening/decoding remains strictly outside the audio callback
  - the process-wide `SignalTap` (`realtime_engine::synth::master_signal_tap`) is the one display tap, with one selected source; see [Signal tap](#signal-tap)
  - musical events may carry an `at` instant that `EngineSource` maps to an output frame; see [Timed events](#timed-events)
  - receives an explicit `AllNotesOff` internal command for releasing Synth, FM, Plucked, and Drum voices and clearing Sampler and preview voices; internal safety does not use MIDI CC 120/123
  - separates the emergency fence, the ordered musical queue, the structural-owner queue, the latest preview, and latest-value control cells; see [Control lanes](#control-lanes)
  - owns live-parameter smoothing, which `playback-runtime` never does itself; see [Parameter smoothing](#parameter-smoothing)
  - accepts control latency and skipped intermediate scalar values to protect musical timing. Notes, Drum hits, and CC remain ordered and are never coalesced. A full or disconnected control lane reports typed overload; terminal retirement pressure fails closed for new NoteOn while preserving NoteOff, CC, and panic service
  - is the only path for Synth, FM, Plucked, Drum, and Sampler internal audio before device output
  - shared JSON audio configuration normalization and FX shape/type validation live in `realtime-engine`; desktop and Pi retain sample preparation, preview/bank orchestration, and typed error adaptation
  - desktop and Pi return the same typed audio-command/config failures (one shared validation, and a full or disconnected preparation lane reports Audio `OperationFailed` or `AudioThreadFailed` on both), clamp note, CC, and note-length values into engine range with the shared `playback_runtime::clamp_musical_event` instead of rejecting them, preserve revision identity for full-config preparation, and route `SamplePreview` through the selected realtime instrument path

### Generations and epochs

The runtime stamps complete audio configs, instrument owners, FX-slot owners, and safe scalar controls with narrow generations; a complete config establishes generation `G`, immediate child controls use `G`, and the first child replacement advances that owner to `G+1`. Momentary start/update/stop uses a separate instance epoch. These stamps prevent stale controls from crossing asynchronous preparation or owner replacement without introducing a general transaction layer.

### OLED frames

PlaybackRuntime owns revisioned OLED production after runtime-error enrichment: typed presentation conversion, two-buffer render/compare cache, monotonic positive frame revision, pending publication, and normalized host metrics. Every emitted snapshot has a positive `oledFrameRevision`; a changed `oled_frame` precedes the referencing snapshot, and unchanged frames publish no pixel payload. Absent or malformed presentation input suppresses the snapshot before the first valid frame, and thereafter applies non-OLED state while retaining the last-good OLED revision and emitting a typed retain-last-good fault/status. Desktop consumes these frames. Raspberry and Orange do not: their render worker renders OLED, LEDs, and HDMI from the native presentation scene plus `native_presentation_state()` (metrics and runtime error), including the initial acknowledged frame, the power terminal frame, and the Raspberry audio-fault frame. Physical OLED revision advances only after a successful write. Boot, fault, shutdown, suspend, and ownership lifecycle frames remain adapter-owned.

### OLED body layout

Native snapshots require serialized `display.bodyLayout` (`rows`, `list`, or `card`). `list` is the layout of a page that opens a list to pick from: it draws like `rows` but always counts as having menu rows, even when none are serialized. `rows` retains existing menu/modal identity, 28-character/8-row normalization, bars, scrollbars, and selected-row scrolling; `card` is used for setup-portal and Backup / Restore status, preserves complete semantic lines, uses the shared 20-column no-scrollbar card rectangle, and reserves the final visual row for its semantic action. `crates/playback-runtime/src/oled_frame/text_layout.rs` owns the character-column metrics, normalization, wrapping, and card layout, and `oled_frame` owns the compiled m5x7 and Tom Thumb fonts and their pixel measurement (breadcrumb fit, hint shortening, label overflow); Pi calls those exported pure functions rather than maintaining a second wrapping algorithm. Concise MIDI rehydration explicitly restores `rows` so a prior status card cannot leak into the MIDI presentation.

### Musical time

The runtime owns musical time in the host `Instant` domain: starting internal playback anchors the runtime clock at `HostAdapter::now`, every pulse gets its exact instant on the 24 PPQN grid, and each pulse reaches the runner as its own step (a catch-up of more than four pulses is one step). Sequenced output (pattern notes, tile hits, CC, MIDI clock, and Start/Continue with the downbeat clock) is stamped pulse instant + the sequenced output delay. Each board picks it at startup for its boot Perf. Mode (`audio_profile::sequenced_output_delay`): the smallest delay that logged no late timed events in board sweeps plus about 1 ms, so 8.8 ms (Latency) and 17 ms (Capacity) on the Raspberry and 4.5 ms and 11 ms on the Orange; the desktop uses 30 ms, whose 1 ms worker wait lasts a Windows timer tick. Under an external clock, clock-driven output is stamped from each realtime byte's arrival instant (taken in the MIDI input callback) plus the same delay. Live input, previews, Stop, panic, and all-notes-off stay untimed. Stop and pause flush the MIDI sender: pending note-offs go out at once and future output is dropped.

### MIDI output thread

`TimedMidiSender` sends each MIDI output from its own thread, holding timestamped messages until due; on the boards it runs SCHED_FIFO 50 on CPU0, below the CPU0 mirror callback and away from the audio callback (CPU1) and the DSP workers (CPU2/3). midir's MIDI input thread takes the same placement on its first message, so a clock byte's arrival instant is not taken late behind ordinary CPU0 threads. Because it outranks the runtime thread on CPU0, a queued message wakes it only when the message is due before the thread's current sleep ends; a pulse's notes cost the runtime one preemption, not one per message. MIDI port listing, connecting and the once-a-second port watch run on the board's MIDI connection worker; the runtime thread only queues those requests and installs the output the worker connected.

### Audio commands

Desktop and board adapters hand runtime audio commands to the shared `playback_runtime::apply_runtime_audio_command` and implement only its `RuntimeAudioCommandHost` transport: engine sends, preparation-lane enqueues, and the adapter-owned momentary FX map. It runs `validate_audio_command` (slot/bus ranges, parameter paths, finite values, FX types and string params, full-config and instrument-slot parsing), checks momentary epochs, maps string-path controls into fixed native parameter IDs, and prepares momentary payloads before audio transport. Adapters prepare structural owners off-callback and preserve playback-runtime generations/epochs unchanged. Safe latest-value controls bypass preparation queues; complete configs, owner replacements, and previews use bounded preparation lanes.

### Desktop audio source

Desktop `AudioRuntime` retains the rodio `Sink` and constructs the inline `EngineSource` with the existing load-status sender, wrapping that final source once with the desktop recording tap and once with `ContainedAudioSource`, which pulls it one engine block at a time inside `catch_unwind`; a panic there leaves the output silent for good without touching the source again and reports `audio_thread_failed` through the desktop audio failure channel; desktop does not select the Orange routing-tree capacity path or create source-worker lifecycle state.

### Desktop second-instance hand-off

The desktop host owns one small thread, `octessera-instance-handoff` (`apps/desktop/src-tauri/src/instance_handoff.rs`), beside its audio and runtime threads. It accepts loopback TCP connections on an ephemeral port recorded with a random token in `octessera.handoff.json` in the store directory, and on a request carrying the token unminimizes, shows and focuses the main window. A second instance that finds the store lock held makes that request and exits; a failed request keeps the "already running" startup error. It touches no runtime state.

### Backup and restore service

`System > Maintenance > Backup / Restore` is a separate native action. PlaybackRuntime owns its action/status/card presentation and existing archive contract; Pi adapters select a usable regular `wlan0` IPv4, bind the authenticated service to `0.0.0.0:8081`, and publish the dynamic URL/code/lifetime. No regular address returns typed unavailable without binding or retry. Desktop returns typed unsupported. The service is never opened through the setup AP or coordinator.

### Device updates

Device-update effects are executed by `/usr/local/sbin/octessera-update` on both boards (Raspberry through `sudo -n`, Orange through the `/run/octessera-update/update.sock` socket), which owns selecting this board's device ZIP, checksum verification, the guarded `octessera-update-switch@<version>.service` restart (Apply reuses a release that is still installed instead of downloading it again), stopping the runtime before snapshotting `system.json` and `default.patch.json` into the release it leaves and restoring the older release's snapshot when going back and, when going forward, the newer snapshot only if the store is unchanged since that rollback (hashes in `store-restored.json`), and swapping back when the runtime does not report ready, with the store put back to the snapshot taken before the switch; `NativeRunner` owns menu/action semantics and confirmation. Neither board selects the other board's asset.

### Audio load status

Pi `AudioManager` owns the bounded `audio_load_status_channel` sender source; the primary Jack source receives a sender in both modes (the Capacity routing-tree source and the Latency inline source, which reports its block render ratio and clipping but no worker utilization or missed quanta), and the native runtime loops drain the newest status before snapshot/readiness work. Secondary-route sources publish no load metrics.

### Executors

The engine has two executors: the inline path (desktop and Latency mode) renders every voice on the audio thread; the routing tree (`routing-tree-executor`, Capacity mode on the boards) splits instruments across two workers that render their voices, whole bus chains, and instrument- and bus-targeted momentary FX one quantum ahead. Both add each voice straight into its instrument's buffer; the two paths sound the same but are not bit-identical. CPU1 sums the worker outputs and owns global-target momentary FX, global/master processing, and final stereo work.

### Routing-tree ownership

The routing tree keeps one `OwnerEnvelope` per worker (voice partitions, the logical `BusChainCarrier` positions, and routing-tree state) in lifecycle-owned capacity-one home mailboxes, work/completion transport, worker local state, or a short callback lease; `SourceWorkerRuntime` owns no DSP bundles or bus carriers at callback boundaries, while `SourceWorkerLifecycle` owns fault mailboxes, completion receivers, joins, and final owner destruction. Stamped work validates runtime, plan, sequence, frame, and sample-clock identity; invalid completions go to fault escrow. Lifecycle-first drop on another thread waits for runtime close; same-thread reverse destruction is prohibited.

### Signal tap

The process-wide `SignalTap` (`realtime_engine::synth::master_signal_tap`) is the one display tap, with one selected source: after each refill `EngineSource` copies the final master block; the inline block renderer copies an instrument slot's block or, through a block loop specialised for a tapped bus, that bus chain's output; in Capacity mode the worker that owns the instrument or bus copies its block. Each copy is mono and pair-averaged into a preallocated lock-free ring, and costs a few relaxed atomic loads per block while nothing is selected. `playback-runtime` selects the source and reads the ring when it captures the home band or a Shape signal view; analysis and drawing stay on the runtime and render threads.

### Timed events

Musical events may carry an `at` instant: `EngineSource` maps it to an output frame through a min-filtered refill anchor, starts the event at its frame offset inside the rendered block (`SynthEngine::with_event_frame_offset`), parks later events in a fixed 64-entry park, and plays late ones at once while counting them in a shared atomic counter (`EngineSource::set_late_timed_events_counter`), which the board runtime loop logs at most once a second in both Perf. Modes; untimed events start at the block start. Synth and sample voices start and release on their frame in both executors; a timed CC changes the slot's filter modulation from the start of the block holding its frame, so it can lead by up to one block, because voices read the CC modulation once per block.

### Control lanes

The engine separates one atomic emergency fence, an exact ordered musical queue whose last 128 slots only note-offs and tile releases may take, a bounded structural-owner queue, one latest preview, and fixed latest-value cells for master, DSP, mixer, Synth, FM, Plucked, Drum-kit, Sampler, persistent-FX, and epoch-keyed momentary controls. The callback applies at most 32 latest cells and 256 total control work units, performs at most one retirement-producing owner mutation, and never allocates, blocks, spins, or destroys retired/source-owned heap state.

### Parameter smoothing

The engine owns live-parameter smoothing: instrument and bus volume and pan ramp per frame over 5 ms, instrument tone parameters glide per slot over 10 ms (advanced once per render block before the voices read them), and bus and master FX levels glide per frame over 10 ms; prepared configs and slot rebuilds land without gliding. A settled parameter costs one flag check per block or per frame. `playback-runtime` sends modulated values unrounded and never smooths audio parameters itself; XY Glide stays the separate runtime path.

## Signal Analysis Worker

- The home band and Shape signal views run their tap read, scope trace, FFT, and history on one analysis worker thread owned by `playback-runtime` (`native_runner/signal_view/`), never on the runtime or audio thread.
- On the boards the worker runs at `SCHED_IDLE` next to the runtime on CPU 0, so the runtime and the render worker preempt it as soon as they wake (`apps/pi-zero/src/audio_priority/dsp_worker_scheduling.rs`).
- The 2048-point FFT is a real-input transform: a 1024-point complex FFT over the sample pairs, split into the real bins, with per-stage twiddle tables and powers instead of magnitudes. Band ranges are computed once per rate and band count.
- User-visible rates and ranges are in [`menu-and-controls-spec.md`](menu-and-controls-spec.md#oled-display).

## Orange production service

The Orange image keeps the same native ownership through the shared Pi host
adapter, plus Orange-specific construction, audio recovery, and power handling.

- `octessera.service` runs the native runtime as the locked
  `octessera-runtime` account; the separate `octessera` account is for
  interactive setup and administration.
- The adapter owns OLED, NeoTrellis, NeoKey, four-encoder, store, sample, and
  MIDI device I/O. Seesaw scans on the shared INT line (PH1) with a 100 ms
  safety poll; Seesaw INT and encoder GPIO use the HAL's gpiocdev v2 edge
  backend.
- Internal Synth, FM, Plucked, Drum, and Sampler audio routes through
  `AudioService` and the realtime engine at 44.1 kHz; MIDI leaves through the
  native host adapter.
- Jack is the always-on primary route; USB and HDMI audio are independently selectable mirrors of the Jack mix. See [Jack and mirror outputs](#jack-and-mirror-outputs).
- A panic inside a board's Jack or mirror output callback is caught there: that stream outputs
  silence from then on without touching its source again and is marked terminal. A terminal Jack
  stream ends the runtime with the Jack audio fault, and the service restarts it; a terminal
  mirror is reported Faulted and stays route-local until it is selected again.
- The image constructs the AHUB0 vendor dummy-codec route and the exact
  `octessera-dac` playback card (`hw:CARD=octesseradac,DEV=0`); no manual or
  experimental audio overlay is used. The optional routes are UAC2
  (`hw:CARD=UAC2Gadget,DEV=0`) and HDMI (`hw:CARD=HDMI,DEV=0`). Simultaneous
  outputs run on independent clocks and can drift or echo; nothing aligns them.
- The HDMI connector is `/sys/class/drm/card0-HDMI-A-1`. Raspberry runtime
  code pins the card0 identity and never scans or falls back to card1.
- The service grants FIFO priority 70 through `LimitRTPRIO=70`. Its only
  ambient and bounding capability is `CAP_SYS_TTY_CONFIG` for native VT
  leasing; it adds no `CAP_SYS_NICE`.
- USB gadget composition is an image-side decision from
  `/var/lib/octessera/presets/system.json`: `audioOutputs.usb` controls UAC2
  and `usb.midiOutEnabled` controls MIDI, giving no gadget, MIDI only, UAC2
  only, or combined. The staged `usr/share/octessera/defaults/pi-default.json`
  is image-build metadata, not an active default.
- Reboot and power-off use the fixed socket lane in the
  [device apply boundary](#device-apply-boundary). Check, Apply, and Rollback
  use the separate root-owned updater socket and only the Orange device
  ZIP. The updater never replaces Armbian, kernel, device-tree, or image
  assets.
- SD2 transfer goes through the root-owned, exact-command
  `/run/octessera-orange-storage-control/storage.sock` lane and the shared
  label-safe SD helper. The board/storage adapter owns the UDC, gadget
  mass-storage setup/teardown, local unmount/remount, and recovery; the
  external host owns its eject.
- Offline DSP profiling (`--profile-dsp`) is a native `apps/pi-zero`
  diagnostic branch that runs before hardware startup, compiled only into the
  `profiling` build (production builds refuse the flag). It exercises the
  existing realtime-engine scenarios and reports snapshots; it implements no
  playback, control, device, or TypeScript fallback.
- Failure policy is three attempts in a 30-second start-limit window. See [Restart policy](#restart-policy).

### Jack and mirror outputs

Jack is the always-on primary route in both Latency and Capacity modes on
both boards. USB and HDMI audio are independently selectable mirrors of the
canonical Jack mix, and HDMI audio is separate from HDMI video. Readiness
depends on Jack; an absent, waiting, or faulted mirror stays route-local. Both mirror rings
exist from startup; an unselected one is inactive, so the Jack mix only checks one flag for it.
The side outputs start, stop and reopen on one worker per output on both boards, off the runtime
thread, and never restart the Jack stream.

### Restart policy

Failure policy is three attempts in a 30-second start-limit window: the
initial start plus two `Restart=on-failure` retries five seconds apart. A Perf. Mode app
restart exits with status 75 so systemd starts it again on both boards; it counts as one of
those starts.
After that the unit stays `start-limit-hit` until someone runs
`sudo systemctl reset-failed octessera.service` and then
`sudo systemctl start octessera.service`.

## Native Audio Recording Boundary

- `crates/media-recording` owns bounded realtime-safe ingress, worker-owned 44.1 kHz stereo signed-16 little-endian PCM WAV writing, PNG-frame/PCM AVI writing, frame-timeline gap handling, lifecycle, container finalization, and collision-safe partial/final naming.
- `Start Audio` writes WAV; `Audio+Video` writes an AVI with PNG-frame video and PCM audio. See [Recording formats](#recording-formats).
- Desktop resolves its platform Audio/Music directory through Tauri and uses `Octessera/recordings` for audio and `Octessera/screen-recordings` for Audio+Video. Raspberry defaults are `/home/pi/recordings` and `/home/pi/screen-recordings`; Orange defaults are `/var/lib/octessera/recordings` and `/var/lib/octessera/screen-recordings`.
- Each worker reserves a partial path and renames it on a complete take; worker failures keep the partial file. See [Take files](#take-files).
- PlaybackRuntime owns recording lifecycle presentation; host adapters own ingress and finalization. See [Recording status](#recording-status).
- Board adapters and the desktop adapter attach exactly one tap to the canonical final stereo mix; mirrored USB/HDMI output callbacks do not create recording owners, and recording ingress performs no filesystem I/O in an audio callback.
- Desktop UI only emits the native recording actions and renders native status; it has no recording or file-writing path.

### Recording formats

`Start Audio` writes WAV under the audio recording root. `Audio+Video` writes an AVI under the screen-recording root with 272x128 lossless PNG-frame video (`MPNG`) at 20 fps (OLED on the left, the four NeoKeys as a bottom-aligned 16-pixel column in the middle with black back/play/shift/layer glyphs, the 8x8 NeoTrellis grid on the right as 16-pixel blocks, LED colors at full brightness) and 44.1 kHz stereo signed-16 little-endian PCM audio. Audio is the master timeline: each video unit advances every 2,205 audio frames, and the latest accepted OLED frame and native LED colors at or before that tick are held until the next one.

### Take files

Each worker reserves a `.partial.wav` or `.partial.avi` path, syncs and renames it atomically to `.wav` or `.avi` on a complete take, and uses `.incomplete.wav` or `.incomplete.avi` when audio gaps or audio ingress overflow occurred; dropped screen frames are logged and the previous picture is held. Worker failures retain the partial file; collision-safe reservations add a suffix instead of replacing an existing take.

### Recording status

PlaybackRuntime owns recording lifecycle presentation: successful starts report `Recording started`, duplicate starts report `Recording is already running`, manual stops report `Recording saved` or `Recording incomplete`, clean max-duration completion reports `Max time: saved`, and an Audio+Video take cut at its file-size cap reports `Max size: cut short`. Recording follows the normal OLED Sleep timer. Host adapters own recording ingress and finalization and return these status results.

## Constructor Boundary

- Every image change goes through the board constructors
  (`release-board-artifacts.yml`).
- Raspberry runtime updates do not replace the full image, kernel, or device
  tree. Raspberry full-image update remains manual.
- The Raspberry `octessera.service` runs as `pi` in a `ProtectSystem=strict` sandbox. See [Raspberry service sandbox](#raspberry-service-sandbox).

### Raspberry service sandbox

The Raspberry `octessera.service` (image and provisioned dev boards) runs as `pi` with
`ProtectSystem=strict`, `PrivateTmp=yes` and `ProtectControlGroups=yes`; it may write only
`/home/pi` (store, samples, recordings, restore), `/run`, `/var/lib/octessera` (show hold) and
`/opt/octessera`. It keeps `NoNewPrivileges=no` because its power, update, USB role and SD2
calls run through `sudo -n` inside the unit. The power and update helpers share this sandbox
(the updater stages into `/opt/octessera`); USB role and SD2 work runs as its own root unit,
started with `sudo -n /usr/bin/systemctl start`, outside this sandbox and its bounding set. For the
same reason it carries none of the directives that imply `NoNewPrivileges` for a non-root
`User=` (for example `RestrictNamespaces=`, `LockPersonality=`, `ProtectKernelModules=` or
`ProtectKernelTunables=`), and no `ProtectHome=`, because everything it owns lives in
`/home/pi`.

## Vendored CPAL Boundary

- The vendored CPAL path and exact patch are documented in
  [`third_party/cpal-0.15.3/PROVENANCE.md`](../third_party/cpal-0.15.3/PROVENANCE.md).
  The patch affects direct Pi CPAL use and desktop/rodio transitively.
- Its boundary is exact fixed-board ALSA PCM opening and host mechanics only:
  typed host errors, stream/buffer/pause/play/wakeup/teardown semantics, and
  WASAPI compatibility.
- Runtime and adapters retain route-selection policy. Broad device discovery and
  fallback are not authorized; CPAL does not own DSP or mixing.
- Rebase or removal requires exact PCM and provenance/legal review.

## Setup Portal Boundary

- `crates/playback-runtime` owns the menu/effect/status presentation. It does not execute setup, authorize a request, or resume playback after setup.
- The fixed Pi adapters write only the start and stop markers and read only the current status file. See [Setup request seam](#setup-request-seam).
- The root-owned `octessera-setup.service` coordinator owns the hotspot, the portal, and the network switch. See [Setup coordinator](#setup-coordinator).
- Desktop returns typed `unsupported` setup status. No TypeScript behavior, `sudo`, `systemctl`, secret handling, root-private state access, network discovery, or fallback path belongs in the runtime or desktop UI.
- `System > Maintenance > Backup / Restore` is a separate direct action and does not use this setup boundary. See [Backup and restore sessions](#backup-and-restore-sessions).
- Archive validation and preference projection stay in `crates/playback-runtime`; Pi host code owns archive/media I/O and the staged restore. See [Staged restore](#staged-restore).

### Setup request seam

For this seam, the fixed Pi adapters create exactly the `start\n` marker at `/run/octessera-setup-request/inbox/start`, then read only `/run/octessera-setup-status/current.json`. `Stop hotspot` creates an empty `inbox/stop` marker for the running session; the coordinator polls it, stops cleanly, and publishes its terminal status only after its own cleanup, so a terminal status means the hotspot is gone. A new start first removes a stale stop marker. They do not read root-private control state, credentials, or setup secrets.

### Setup coordinator

The enabled path unit starts the only root-owned `octessera-setup.service`. The coordinator owns the hotspot (hostapd, a wildcard-DNS dnsmasq, and the per-session WPA2 passphrase shown on the OLED from the `portal_ready` status), the HTTP portal on the AP address `192.168.42.1:80` (bound only once the hotspot is up), country handling, setup-value validation and in-memory holding, the NetworkManager switch to the chosen network, finalization, cleanup, and the one current status file. The confirmed instrument `System > Connections > Wi-Fi > Setup Portal > Open Portal` action triggers this path; a fresh production image does not start an automatic hotspot. The AP user window is 10 minutes after readiness. Browser submission is provisional, and an AP disconnect is not a result; the OLED terminal result is authoritative. Success requires a usable global `wlan0` IPv4 address only, not Internet access, a default route, DNS, or ICMP, then shows the Sys. Info popup over the unchanged menu position without a reboot.

### Backup and restore sessions

`System > Maintenance > Backup / Restore` is a separate direct action and does not use this setup boundary. Its Pi service selects a current usable regular `wlan0` IPv4 address, binds `0.0.0.0:8081`, uses a generated 10-character code for 15 minutes, and reuses the active session on reopen without extending it. Back only hides the OLED card; Stop closes the service and revokes the code. An export that leaves damaged patches or presets out names each in an `X-Octessera-Export-Warning` header and queues one `user_data_export_status` toast with the count (`Export: N items skipped`). Expiry or authentication revocation closes it, while the existing physical restore confirmation and restore-time input barrier remain native contracts.

### Staged restore

Data Backup/Restore archive validation and preference projection stay in `crates/playback-runtime`; Pi host code owns archive/media I/O and applies a physically confirmed, staged restore. Confirmation is consumed atomically before adapter status draining, publishes the typed native `RuntimeUserDataRestoreStatus { phase: RuntimeUserDataRestorePhase::Restoring }` snapshot with `Restoring...`/`Please wait`, consumes normal device input until the worker reports `Succeeded` or `Failed`, and serializes the restore with platform store jobs and direct System/patch saves. The swap moves each folder's contents, not the folder, and never copies, moves or deletes across a mount, so a mounted SD card stays in place and untouched. Bundled library folders it may not write (the Orange image owns them) stay in place too; any other folder it may not write fails the restore with a rollback. The pre-restore archive and the `.old`/`.new` recovery trees live in a runtime-owned staging folder next to the store, on the same filesystem (Orange `/var/lib/octessera/restore`, created by the image; Raspberry `/home/pi/.octessera-restore`). A successful restore keeps the two newest pre-restore archives. When the pre-restore archive skipped a damaged patch or preset, the new store also gets the old `patches/`, `current.json` and `recovery-save.json` copied raw into `backups/pre-restore-<session>/`, which later restores carry over with the rest of `backups/`; a failed restore that rolled back cleanly deletes its own archive and `.new` trees, and a failed rollback retains them all there. If power is cut mid-swap, the next start (both boards, before the store loads) finishes it: a session with only `.new` trees (swap never began) loses them; a session with any `.old` tree rolls forward, tree by tree, and its `.old` trees are deleted only once that session's pre-restore archive exists; any error leaves the session as it is for the next start. The `.oct` archive carries music and selected portable preferences, with optional media; destination-local USB role, MIDI/audio endpoint IDs, and favorites remain local. Credentials, binaries/images, and hardware identity remain outside the portable bundle.

## Device Apply Boundary

- Orange's `/run/octessera-device-apply/reboot.sock` is a root-owned, group-limited systemd socket for exactly three requests: `reboot\n`, `poweroff\n`, and `usb-gadget\n`; it remains one fixed socket for compatibility and does not discover or select commands.
- `octessera.service` requires and starts `octessera-device-apply-reboot.socket` before the runtime; the socket is enabled separately through `sockets.target` and waits only for `local-fs.target`. Musical-default provisioning remains an independent runtime prerequisite, not a socket prerequisite. This dependency only repairs socket activation ordering and does not change the helper protocol or security boundary.
- The socket service accepts only the three exact requests and invokes only the matching fixed command. See [Device apply socket](#device-apply-socket).
- Native Rust owns the confirmed save/apply/shutdown semantic; the image helper owns only fixed request validation and command execution. See [Device apply socket](#device-apply-socket).
- `System > Maintenance > Updates` uses the separate root-owned `/run/octessera-update/update.sock` socket. See [Updater socket](#updater-socket).
- Orange SD2 transfer uses the separate root-owned `/run/octessera-orange-storage-control/storage.sock` socket. See [Orange SD2 transfer](#orange-sd2-transfer).
- Raspberry SD2 transfer starts a fixed root unit through `sudo -n /usr/bin/systemctl start`. See [Raspberry SD2 transfer](#raspberry-sd2-transfer).
- Raspberry USB data-role apply is live and runs as its own root systemd unit. See [Raspberry USB data role](#raspberry-usb-data-role).

### Device apply socket

The socket service validates the regular `/var/lib/octessera/presets/system.json` file, its expected runtime ownership/mode, and the canonical audio/gadget schema only for `reboot\n` and `usb-gadget\n`, then invokes only `/usr/bin/systemctl reboot`, `/usr/bin/systemctl poweroff`, or `/usr/bin/systemctl restart octessera-orange-usb-gadget.service` for the matching exact request before returning `accepted\n`; `usb-gadget\n` is also rejected while `/run/octessera-usb-storage.state` marks an active SD2 transfer; malformed, unknown, extra-byte, timeout, or failed requests return `rejected\n` without invoking another command.

Native Rust owns the confirmed save/apply/shutdown semantic and sends the power requests through the adapter lane after external MIDI panic and internal audio silence; the platform worker sends `usb-gadget\n` during a live apply, after saving System, while music keeps playing. The image helper owns only the fixed request validation, reboot config validation, and power command execution; it does not accept command arguments, config payloads, or arbitrary commands. Orange has no `octessera-runtime` sudoers entry.

### Updater socket

When Internet is available, `System > Maintenance > Updates` uses the separate root-owned `/run/octessera-update/update.sock` socket for Octessera runtime-only `check\n`, `apply\n`, and `rollback\n` actions. The socket service runs `octessera-update serve`, which answers one action with a bounded `ok`/`error` result; it does not expose a general command or privilege broker. The updater changes only the runtime release under `/opt/octessera`. Armbian, kernel, device-tree, and image replacement remains a reflash.

### Orange SD2 transfer

Orange SD2 transfer uses a separate root-owned `/run/octessera-orange-storage-control/storage.sock` socket. Its systemd service accepts only one bounded `storage-start\n` or `storage-stop\n` request from `octessera-runtime`, invokes only the fixed storage action, and returns bounded `accepted`/`rejected`, sanitized `HOST_STATE`, and error fields. The lifecycle owns the shared SD-card/gadget locks and state, refuses root/parent/wrong/duplicate-label candidates, never binds while locally mounted, and restores the configured UAC2/MIDI gadget after a successful stop. The service runs in the host mount namespace (no filesystem sandbox), because a private namespace would keep its unmount and remount from reaching the runtime; Orange has no storage sudoers entry.

### Raspberry SD2 transfer

Raspberry SD2 transfer runs the exact `sudo -n /usr/bin/systemctl start octessera-usb-storage@start|stop.service` call; the oneshot root unit runs `octessera-usb-gadget storage-start|stop`, which unmounts/remounts the SD, loads the gadget modules, and swaps the Audio/MIDI gadget for mass storage. The root work runs as its own unit because the umount, mount, modprobe and `system.json` reads need capabilities outside the app's bounding set. A failed unit reports `USB SD2 transfer storage-start|stop failed` with the systemctl error; the helper's own reason is in the unit's journal. After a successful start the app reads the UDC `state` under `/sys/class/udc` to show `USB SD2 transfer active` or `USB SD2 transfer waiting for host`. The `octessera-usb-storage` sudoers entry permits only those two starts and the gadget restart.

### Raspberry USB data role

Raspberry USB data-role apply is live: the Pi platform worker serializes the validated System write and the exact `sudo -n /usr/bin/systemctl start octessera-usb-role@host|gadget.service` call (or `sudo -n /usr/bin/systemctl restart octessera-usb-gadget.service` when only USB Audio or USB MIDI changed). The root work runs as its own systemd unit because sudo inside the app's service would inherit its narrow capability bounding set. A helper failure keeps the saved System for the next boot. Host apply is rejected before writing when the authoritative SD2 state exists; the root helper changes only the exact managed `dtoverlay=dwc2,dr_mode=...` line in the single `[all]` block, then switches the live controller: Host tears down the gadget first, then the helper unbinds the single dwc2-bound controller, loads the runtime `dwc2` overlay with the matching `dr_mode` from the firmware `overlay_prefix` directory, binds it again, and Gadget runs `octessera-usb-gadget setup`. A live-switch failure exits nonzero after the boot config is already updated, so the next boot applies the role. The gadget service rebuilds the gadget from the saved System file; while an SD2 transfer is active the app skips the USB step, and stopping SD2 rebuilds the gadget instead. Systemd skips gadget setup in Host mode; SD2 start rejects Host before mutation, while SD2 stop remains available to validate active state, tear down mass storage, remount the original SD when required, remove state, and leave the gadget absent. Constructor/provisioning own helper installation and boot reconciliation; deploy-fast and runtime updates do not mutate boot configuration.

## OLED Boot Handoff Boundary

- The final systemd paths on both boards use the same source-defined OLED boot sweep from `resources/oled/boot-sweep-v1.json`.
  - The mounted SSD1351 controller origin decreases by 303 px while the physical panel motion is left-to-right. In canonical bottom-to-top coordinates the top-row origin is 127 px less X than the bottom-row origin (`bottom_origin - row_y`), which produces the physical-panel slash.
  - Green, magenta, cyan, and yellow bands read left to right (the Build, Link, Shape, and Play menu colors), with 8 px bands and 4 px separators.
  - 30 frames over a 1,200,000,000 ns cycle (25 fps). The clean logo+wordmark frame follows, and continuous boot loops hold it for a 2,000,000,000 ns rest between loops.
  - This is a source and test contract; check the real panel through the board test procedures.
- First boot is not designed to wait six minutes. Upstream filesystem resize is ordered before the runtime, and six minutes is only the upstream maximum timeout; the normal animated splash covers that work.
  - The root-installed systemd animators on both boards are the only loops and run alongside service loading once their fixed OLED devices are ready. Raspberry explicitly wants and waits for `systemd-udev-settle.service` before opening its GPIO/SPI devices, rather than depending on device units or polling. Either board may stay blank until systemd starts its animator.
  - Native startup requests release, waits for the shared OLED lock, adopts the already-initialized display without reset, and stops the animation or persistent handoff status immediately before publishing the acknowledged first normal menu frame.
  - If native ownership has not arrived within the 30-second handoff window, both boards write one static `STARTUP DELAYED` / `PLEASE WAIT` frame and keep polling the same handoff state as the only OLED writer.
  - A matching stop request releases without black/off; only genuine writer errors or termination signals use the black-frame/display-off cleanup. Sleep and shutdown are separate OLED lifecycle paths, not boot-handoff states.
- Native runtime owns the confirmed instrument-menu lifecycle presentation. `Going to sleep`, `Shutting down`, and `Rebooting` are exact native toasts over the shared static `SPLASH_SLEEP_SHUTDOWN` background and existing toaster geometry.
  - Ordinary Reboot and Shutdown share one board-neutral lifecycle. See [Reboot and shutdown lifecycle](#reboot-and-shutdown-lifecycle).
  - Save, safety, or acknowledgement failures never submit power. Power submission failures leave the accepted lifecycle closed and are reported as typed/logged failures.
  - None of this covers arbitrary administrative `systemctl poweroff` or `systemctl reboot` commands.
- Both boards catch SIGINT, SIGTERM, and SIGHUP and write the pending autosave before exiting. See [Stop signals](#stop-signals).
- Orange's boot-loop handoff has a monotonic 30-second deadline. See [Orange handoff deadline](#orange-handoff-deadline).
- Hardware fault mode is shared by both boards and has one runtime renderer. See [Hardware fault mode](#hardware-fault-mode).
- Orange startup-fatal presentation for the remaining failures is adapter-owned and fixed. See [Orange startup fatal](#orange-startup-fatal).
- Orange runtime failure does not stop or mask NetworkManager, and it does not stop an already-enabled SSH service. Fresh images keep SSH off until setup selects a key or password, and setup enables SSH only for those two modes. Wi-Fi association after a warm reboot is not guaranteed.
- An unconfigured Raspberry image keeps SSH disabled without masking its vendor SSH units, so explicit Raspberry Pi Imager customization can enable it. Imager customization must retain the fixed `pi` username because the Raspberry runtime requires that account; the setup portal's SSH-off choice remains unchanged.
- Orange NeoTrellis operation requires the exact validated bus, wiring, and addresses; adapters do not provide an alternate bus, address, or hardware fallback.
- The menu `OLED Sleep` setting is display-only; Linux suspend is a separate Orange `sleep.target` ownership transaction. See [Orange suspend](#orange-suspend).
- The board runtime owns `/run/octessera-boot`, including `oled.lock`, `status.json`, and `stop.request`. The lock is exclusive: the animator, native runtime, and lifecycle utilities may not write the OLED concurrently. `status.json` uses the strict phases `animating`, `release_requested`, `released`, `native_owned`, `first_menu_rendered`, and `failed`; the normal path reaches `first_menu_rendered`, while matching failure state remains attachable for recovery.
- First-menu readiness requires an actual OLED write acknowledgement. Every valid Pi audio set includes always-on Jack, and readiness depends on Jack. Optional USB or HDMI mirrors may be absent, waiting, or faulted without blocking or replacing Jack. A snapshot being queued, or a runtime service merely starting, is not readiness.
- Normal Raspberry and Orange output apply their board-specific clockwise RGB565 framebuffer transform in the HAL. See [OLED orientation](#oled-orientation).
- HDMI `Terminal` leaves Linux in control of `/dev/tty1`; native grid output takes a native VT lease around `/dev/fb0`. See [HDMI terminal and grid](#hdmi-terminal-and-grid).
- Each board's selected initramfs writes one static frame; its root-installed systemd service remains the only loop. See [Initramfs frame](#initramfs-frame).
- Boot source, service, and selected-initramfs changes are constructor-required for both boards.

### Reboot and shutdown lifecycle

Ordinary Reboot and Shutdown share one board-neutral lifecycle, in order: the recovery save must succeed; the lifecycle atomically accepts the terminal transition and stops non-terminal follow-ups and input; it independently attempts external MIDI panic and internal audio silence; only after both succeed does it force-publish and physically acknowledge the latest terminal snapshot, zero the grid and NeoKey LEDs, and detach the OLED handles while keeping its pixels on; finally it invokes the board adapter's fixed power path.

### Stop signals

Both boards catch SIGINT, SIGTERM, and SIGHUP once the runtime loop starts (`systemctl stop`/`restart`, a system shutdown, or the updater stopping `octessera.service`). The handler only sets a flag; the loop sees it at the top of its next iteration, writes the pending autosave at once instead of after the debounce and two-second coalescing, waits for the store worker to finish its queued writes, and returns so the app exits. The units keep systemd's default stop timeout, which leaves ample room for that write.

### Orange handoff deadline

Orange's boot-loop handoff has a monotonic 30-second deadline that begins immediately after `handoff.start()`, before OLED initialization or adoption. At the deadline it writes the static `STARTUP DELAYED` / `PLEASE WAIT` frame once and keeps polling; matching stop requests release without black/off. Genuine errors or termination signals attempt an exact 32768-byte black RGB565 frame, then display-off `0xAE` independently, close once, and publish one attachable `failed` state. A failed handoff keeps the current boot ID and matching valid request ID so native recovery can attach it. Signal handlers only set a flag and do no I/O.

### Hardware fault mode

Hardware fault mode is shared by both boards and has one runtime renderer. A failed Grid, Keys, encoder, Seesaw INT or Seesaw startup (plus I2C init on Raspberry), or any audio startup failure, attaches the OLED from the animator through the normal native attach (Orange's attach also clears any startup fatal), and flashes one fault frame. The screen is switched on at full contrast for 10 s, then switched off again so a fault left overnight does not burn in; any encoder turn or press shows it for another 10 s. Fault mode reuses encoders the runtime already opened and opens them itself otherwise; if they cannot be opened, the screen stays on. On the Raspberry, a hardware fault also logs the Seesaw bus SDA and SCL levels and appends them, with the failing parts, to `i2c-fault-lines.log` beside the store (last 20 entries), because the journal does not survive the power cycle a stuck bus needs. Hardware failures show `HARDWARE FAULT` with every failing part (or `Several parts`): `not responding` and `Unplug power and plug it back in.` for I2C timeouts, because a warm reboot does not reset the Seesaw boards; otherwise `not found` with only the physical checks that exist for the named parts (grid cable, NeoKey socket, screen socket; the encoders are soldered, so none). Hardware failures keep the service alive until a power cycle. Audio failures show `AUDIO NOT READY` / `Retrying...` / `If this stays for a few minutes, power-cycle the device.` and exit non-zero after 30 s so `octessera.service` restarts and retries, which heals an ALSA device still held by the previous instance or a late codec: the PCM5102 has no control interface, so these are driver or configuration problems, never a detected missing part. Orange restores the default SIGINT/SIGTERM/SIGHUP handlers first, so stopping the service ends fault mode at once, as on Raspberry. Detailed causes stay in the `octessera.service` journal (`journalctl -u octessera.service --no-pager`).

### Orange startup fatal

Orange startup-fatal presentation for the remaining failures is adapter-owned and fixed: `OLED NOT READY` or generic `STARTUP FAILED`. The splash remains the OLED writer for these. A fatal frame is bright initially, redraws once at a dim but readable level after 60 seconds if unchanged, and becomes bright again with a changed code. Raspberry does not consume the Orange fatal state.

### Orange suspend

The menu `OLED Sleep` setting is display-only UI behavior. Linux suspend is a separate Orange `sleep.target` ownership transaction. Production installs `octessera-orange-oled-suspend.service` through `RequiredBy=sleep.target`, creating the hard `sleep.target.requires` relationship rather than a `.wants` relationship; a failed preparation therefore blocks suspend. The runtime quiesces and preserving-detaches its OLED handles, a strict AF_UNIX helper renders the suspend/resume frames, and the runtime reacquires the lock and hardware without changing the established `first_menu_rendered` handoff phase. Audio, MIDI, transport, and LED contracts remain unchanged.

### OLED orientation

Normal Raspberry and Orange output apply their board-specific clockwise RGB565 framebuffer transform (`source (x, y) -> transmitted (127 - y, x)`) in the HAL before writing accepted native RGB565BE bytes. Orange uses a reusable 32 KiB transform buffer, preserves RGB565 byte pairs, and keeps SSD1351 remap `0x74`. The Python splash is a separate renderer with the same physical orientation; its SSD1351 command byte is sent with D/C low and command data with D/C high, matching the Rust transport framing.

### HDMI terminal and grid

HDMI `Terminal` leaves Linux in control of `/dev/tty1`; native grid output takes a native VT lease around `/dev/fb0` and restores text on release. It does not force a connector or start/use a display server. A missing framebuffer or transient open, geometry, or write failure is nonfatal and retried. During boot handoff the splash is an observer until `first_menu_rendered`; a native startup fatal condition lets it reclaim OLED presentation for the typed fatal frame. These are source contracts; hardware display behavior is exercised separately.

### Initramfs frame

Raspberry's selected initramfs writes one clean logo+wordmark frame and does not sweep, loop, publish a marker, or adopt the OLED. Its root-installed systemd service remains the sole animator and runs concurrently with service loading; Orange's selected initramfs likewise writes one static RGB565 frame with its fixed Python closure, while its root-installed renderer, lifecycle modules, assets, and `octessera-orange-boot-splash.service` remain the only loop. These are adapter differences around one shared handoff contract.

## Dependency Rules

- UI may import type contracts and render snapshots only.
- UI must not call native core, transport, audio, MIDI, or storage bridges directly.
- Runtime may import native core and output/input adapters.
- Core crates must stay platform-agnostic.
- `crates/platform-core` and `crates/playback-runtime` must not depend on Tauri, HAL, Pi hardware crates, storage implementations, or host filesystem/sample-browser adapters.
- Platform adapters must not create independent musical audio sinks that bypass the realtime engine mixer.

## Data Flow

1. UI interaction -> `DeviceInput`
2. Runtime receives input -> native `platform-core` transition through `NativeRunner`
3. `NativeRunner` publishes typed runtime-config changes -> `PlaybackRuntime` updates transport/MIDI scheduling state
4. Rust runtime advances transport pulses -> native behavior/menu processing
5. Runtime publishes snapshot -> UI render (OLED + NeoKey LEDs)
6. Runtime publishes musical events/audio commands/platform effects -> host adapters (audio/MIDI/storage)

## Shared Runtime Contract

- The shared Pi/desktop playback seam is defined by `crates/playback-runtime/src/protocol.rs` and mirrored by the UI/device contract types where needed.
- Host -> runner messages are limited to `device_input`, `transport_pulse_step`, split MIDI realtime wire messages (`midi_realtime_clock`, `midi_realtime_start`, `midi_realtime_continue`, `midi_realtime_stop`), `midi_control_change`, and `runtime_result`.
- Runner -> host messages include `snapshot`, optional `oled_frame`, `platform_effects`, `musical_events`, `midi_events`, `audio_commands`, and `runtime_status`. Every `snapshot` leaving `PlaybackRuntime` carries a positive OLED revision. `oled_frame` is changed-frame-only and precedes its matching snapshot; Raspberry and Orange pair accepted native frame bytes to normal snapshot publication or retained-last-good bytes during a typed cache fault; desktop remains a separate native consumer.
- `transport_pulse_step` is the deterministic PPQN advancement boundary; hosts must not substitute wall-clock timer semantics above this seam.
- External MIDI realtime (`clock`, `start`, `continue`, `stop`) remains explicit at the boundary and is not inferred from UI/runtime scheduling code. Desktop MIDI input is routed natively from the host adapter into the runtime worker; UI code must not observe raw MIDI bytes for display or transport state.
- Incoming Control Change messages (`0xB0`-`0xBF`) are the only other MIDI input that passes the host filters; notes stay out. Hosts handle the realtime bytes of a drain first, then send one `midi_control_change` per channel and controller with its latest value (`MidiControlChangeBatch`), so a knob sweep cannot crowd out clock pulses.
- `runtime_result` carries host-side outcomes for storage, MIDI port enumeration/selection, sample-browser operations, device-update status, and asynchronously identified sanitized system-info requests back into the shared runner.
- `SystemInfoRequest` is a typed platform request; adapters return typed `SystemInfoResult` or identified `SystemInfoError` values. The native runner owns loading/error/unavailable presentation, row formatting, clipping, scrolling, and dismissal. Desktop UI renders only the resulting snapshot/OLED frame.
- `NativeRunner` may emit an internal typed runtime-config change during dispatch; `PlaybackRuntime` consumes it before returning presented host messages. It is not a host adapter responsibility and is never reconstructed from snapshot fields.
- Central modulation processing is the single native path for held tick/XY sources and global LFO output. See [Modulation processing](#modulation-processing).
- Background audio preparation returns identified typed success/failure results through `runtime_result`; prep failures retain the last good runtime/audio state.
- Sample-bank preparation is atomic on both hosts, and sample preview resolution/decoding runs on the audio-prep worker. See [Sample preparation](#sample-preparation).
- Pi audio preparation treats superseded revisions as cancellation: no stale-prep fault is returned or latched.
- Identified asynchronous results retain their request ID/revision through the runner round trip; `PlaybackRuntime` observes each result once and clears only the matching fault.
- Emission and persistence faults clear only after the corresponding native emission or identified save/recovery acknowledgement succeeds. Native save confirmation/toast feedback is emitted after that acknowledgement, not when a save request is queued.
- Save and load rules (two documents, System vs patch ownership, autosave, confirmed loads) live in [`menu-and-controls-spec.md`](menu-and-controls-spec.md#save-and-load).
- Stop-and-silence recovery independently attempts runner transport stop, internal instrument silence, and external MIDI panic on both hosts.
- `snapshot` is the runtime display/input-facing state payload; `musical_events`, `tile_hits`, `midi_events`, `platform_effects`, and `audio_commands` are the resolved outputs that Rust schedules or dispatches.
- `oled_frame` carries a positive revisioned 128×128 `rgb565be` Base64 payload on the JSON bridge. See [OLED frame payload](#oled-frame-payload).

### Modulation processing

Central modulation processing is the single native path for held tick/XY sources and global LFO output: behavior ticks and active XY captures update held sources, global LFOs advance only at 24 PPQN, and the process sums persistent, held, and LFO contributions, clamps once, and applies once per target endpoint. An active LFO step processes only dirty LFO endpoints plus other contributors sharing those keys; unrelated held grid/XY sources are retained without being resolved, reapplied, or cloned. Ordinary menu/Aux base edits rebase and recompose only the edited key/endpoint, preserving held sources so clearing a source restores the new base; changed persistent targets from one tick share one revision and delayed autosave request. Config/patch transactions reset candidate modulation state, install every persistent owner, then resample active XY so its captured base is the loaded owner value. Every LFO phase advances and wraps while playing, also while the LFO is off or at depth zero, so a re-enabled LFO stays in phrase; only enabled targeted LFOs trigger modulation work; XY Glide (`smoothingMs`) is the separate native monotonic-elapsed-time path for mapped normalized axes and does not move into desktop TypeScript or realtime-engine code.

### Sample preparation

Sample-bank preparation is atomic on both hosts: every configured sample path must resolve and decode before a new bank is queued; unresolved or undecodable samples return typed `sample` failures and leave the previous banks and signatures in place. Sample preview resolution/decoding runs on the audio-prep worker, never on the runtime or host-adapter thread, and reports success only after the prepared preview reaches the audio queue.

### OLED frame payload

`oled_frame` carries a positive revisioned 128×128 `rgb565be` Base64 payload on the JSON bridge; typed native pixels remain immutable bytes before serialization. Desktop, Raspberry, and Orange adapter caches stage candidates and promote only on matching snapshot references, validate dimensions, format, Base64, exact byte length, revision idempotency/conflicts, and expose sticky typed cache faults. Same-revision byte conflicts remove candidates or poison accepted revisions until a strictly newer valid snapshot matches. Raspberry and Orange normal OLED publication consumes accepted native bytes with no semantic renderer fallback; before acceptance they publish explicit black, and after acceptance they retain last-good bytes rather than black on a bad reference.

## Audio Routing Contract

- The shared runtime audio rate is 44.1 kHz.
- Internal Synth, FM, Plucked, Drum, and Sampler instruments must enter the realtime engine before audio output.
- Instrument `Route=direct` bypasses FX bus processing and pans directly into the main mix.
- Instrument `Route=fx_bus_n` enters the selected FX bus, runs its slot FX in order, then pans into the main mix.
- MIDI instruments emit external MIDI/control data and are not an internal audio source unless an audio return path is explicitly added.
- MIDI-only instrument notes and CCs use the `midi_events` path and must not call host internal-audio musical event handling.
- `PlaybackRuntime` copies internal instruments' notes to MIDI out only for the channels in `RuntimeConfig::midi_out_internal_channels`. See [MIDI copies of internal notes](#midi-copies-of-internal-notes).
- `PlaybackRuntime` remembers the last 64 notes it sent to MIDI out, at the instant each goes on the wire, and drops an incoming MIDI In note identical to one of them within 30 ms as that device's echo before it reaches the runner.
- MIDI Drum slots are routed by the runner straight to the `midi_events` path: a Link note-on on a stamped tile becomes that tile's note and velocity on the slot's channel, with a timed note-off.
- Sample browser preview is musical audio and must route through the selected instrument slot, pan, volume, FX bus, and master output path.
- The default sample library is the `samples/` folder of WAV files; builds copy it as-is, and there is no sample manifest. Pi and desktop audio preparation leave a missing, undecodable or too-long sample's slot empty and report the affected paths with `RuntimeStoreResult::SamplesMissing`; the runtime owns the resulting `N/A-` slots and the `Samples missing` popup.
- Raspberry, Orange, and desktop audio recording tap the canonical final stereo mix once; recording ingress carries absolute session audio-frame offsets and never performs filesystem I/O in an audio callback.
- Runtime audio config commands carry `sound.voiceStealingMode`; host adapters forward it to the realtime audio policy.
- Raspberry and Orange startup read `runtimeConfig.sound.optimizeFor` as the authoritative local DSP mode. See [Perf. Mode buffers](#perf-mode-buffers).
- `gridBrightness` is applied by core LED frame rendering; `displayBrightness` leaves native OLED frame bytes at full intensity and picks SSD1351 master contrast step `value - 1` through `oled_frame::oled_master_contrast_step`, which the Pi render worker sends and the desktop OLED view emulates; `buttonBrightness` is applied by desktop and Pi only to the native unscaled `neoKeyLeds` values with the shared basis-point dim rule.

### MIDI copies of internal notes

`PlaybackRuntime` copies internal instruments' notes to MIDI out only for the channels in `RuntimeConfig::midi_out_internal_channels`: every unmuted slot under System > MIDI > Send MIDI `all`, none under `midi`; their CCs always go out. It remembers the held copies and sends their note-offs when their channel stops being copied (Send MIDI switch or Play Mix mute). Drum and Sampler `TileHit`s carry the GM note the runner picked (`midi_note`), and the runtime copies a struck hit as a fixed 100 ms note.

### Perf. Mode buffers

Raspberry and Orange startup read `runtimeConfig.sound.optimizeFor` as the authoritative local DSP mode. Raspberry Latency constructs the inline Jack path with output 256, ALSA period 64, internal block 128, and zero lookahead; Raspberry Capacity constructs the routing-tree Jack path with output 256, ALSA period 64, internal block 256, and 256-frame lookahead. Orange Latency constructs the inline Jack path with output 128, ALSA period 32, internal block 32, and zero lookahead; Orange Capacity requires Jack and constructs the routing-tree Jack path with output 256, ALSA period 64, internal block 128, and 128-frame lookahead. Desktop remains inline.

## Grid Coordinate Contract

- Core logic uses a world-space grid origin at lower-left: `(0,0)` is bottom-left, `y` increases upward.
- UI/hardware-facing layers may use screen-space coordinates (top-left origin), but conversion is only allowed at boundaries.
- `platform-core` owns the fixed 8×8 pure logical row-major and logical/display projection helpers. `playback-runtime` delegates its existing display-index API to those helpers.
- `packages/device-contracts/src/gridDomain.ts` is the deliberate TypeScript boundary mirror for desktop input/rendering; it is checked against the exhaustive `resources/grid-projection-v1.json` fixture.
- The HAL keeps fixed NeoTrellis quadrant/key wiring separate from the native projection owner. Its four-device addresses, physical key ordering, and GRB output are adapter facts, not generic geometry.
- In code, grid coordinate conversion must go through these centralized boundary helpers rather than ad-hoc math; the checked fixture is verification data, not a runtime table.
