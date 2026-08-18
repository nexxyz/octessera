# Runtime Boundaries

This contributor contract keeps UI, host adapters, native runtime logic, core behavior logic, and audio rendering in separate layers.

Authoritative menu/control behavior spec: `docs/menu-and-controls-spec.md`.

## Layer Responsibilities

- UI layer (`apps/desktop/src/`)
  - renders runtime snapshot data
  - captures user interaction and emits `DeviceInput`
  - contains no transport/menu/audio/interpretation logic

- Runtime orchestration layer (`crates/playback-runtime`, `apps/desktop/src-tauri/src/runtime_worker.rs`)
  - owns lifecycle (`start`/`stop`)
  - schedules transport pulses and realtime status through Rust runtime code
  - owns native menu state, config payloads, snapshots, platform effects, and `NativeRunner`
  - owns schema-v2 global modulation state, legacy migration, keyed Link LFO menu/binding paths, canonical global Play XY serialization, and transient global-LFO endpoint composition; it emits resolved audio commands while the realtime engine owns rendering
  - owns the schema-v2 canonical device audio contract: atomic `AudioOutputSet` values for Jack, USB, and HDMI output preferences, strict legacy `usb.audioOut` migration, whole-set validation, and next-boot apply confirmation; adapters open the selected exact physical routes under this native contract
  - `PlaybackRuntime::dispatch` is the canonical host-message/result observation path; desktop and Pi loops schedule work and render its presented output
  - consumes typed runtime-config changes published by `NativeRunner` during canonical dispatch; hosts must not derive playback scheduling config from snapshots
  - maps typed adapter failure facts to recovery policy and owns the best-effort stop-and-silence operation
  - requires every host adapter to implement internal-audio silence and external-MIDI panic explicitly; safety cannot fall through to a successful no-op
  - classifies worker emission and persistence faults as retain/retry outcomes instead of safety-stop failures
  - applies native core behavior transitions through `platform-core`
  - publishes snapshots, including authoritative transient indicator fields and unscaled semantic NeoKey colors, plus platform effects, audio commands, MIDI events, runtime status, and native-owned modal frames
  - PlaybackRuntime owns revisioned OLED production after runtime-error enrichment: typed presentation conversion, two-buffer render/compare cache, monotonic positive frame revision, pending publication, and normalized host metrics. Every emitted snapshot has a positive `oledFrameRevision`; a changed `oled_frame` precedes the referencing snapshot, and unchanged frames publish no pixel payload. Absent or malformed presentation input suppresses the snapshot before the first valid frame, and thereafter applies non-OLED state while retaining the last-good OLED revision and emitting a typed retain-last-good fault/status. Raspberry and Orange normal runtime consumers publish only an accepted frame paired to the snapshot revision: no accepted pair is explicit black, while a missing/future/stale/malformed/conflicting reference publishes immutable accepted bytes as `RetainedLastGood` with a sticky typed fault. The initial acknowledged handoff still requires an exact accepted matching pair. Physical OLED revision advances only after a successful write. Desktop remains a separately gated native consumer; boot, fault, shutdown, suspend, and ownership lifecycle frames remain adapter-owned.
  - owns monotonic wall-clock deadlines for the 45 ms event dot and 90 ms beat/measure flash; hosts request timed expiry snapshots and only scale native NeoKey RGB values
  - owns setup-portal menu confirmation, playback stop/reset and note cleanup, the typed `RuntimePlatformEffect::SetupPortalOpen` effect, and all typed setup status/modal presentation
  - owns the portable Data Backup/Restore bundle contract: canonical patch migration and validation, the allowlisted preference delta, manifest/hash metadata, and typed transfer status; it does not perform transfer or filesystem I/O
  - delegates its display-ordered grid index API to the pure fixed-grid projection helpers in `platform-core`; it does not own a second projection table or geometry rule
  - The Orange production service keeps this native ownership through an Orange-only adapter: `octessera.service` runs the native runtime as the locked `octessera-runtime` account, while the separate `octessera` account is for interactive setup and administration. The adapter owns OLED, NeoTrellis, NeoKey, four encoder, store, sample, and MIDI device I/O; Seesaw uses polling and encoder GPIO uses the HAL's gpiocdev v2 edge backend. Internal synth/sample audio routes through `AudioService` and the realtime engine at 44.1 kHz; MIDI leaves through the native host adapter. Every non-empty Jack/USB/HDMI output set is valid. A selected Jack route is required and a Jack fault blocks readiness only when Jack is selected. Recognized disconnected selected UAC2 or HDMI routes may wait and recover; a selected route fault blocks readiness, and no route is used as a fallback for another selected or unavailable route. Readiness follows selected-route status, initialized control-surface devices, and the first rendered snapshot. The service grants FIFO priority 70 through `LimitRTPRIO=70`; it adds no `CAP_SYS_NICE` or ambient capability. Orange USB gadget composition is an image-side adapter decision from `/var/lib/octessera/presets/default.json`: `audioOutputs.usb` controls UAC2 and `usb.midiOutEnabled` controls MIDI, with no gadget, MIDI-only, UAC2-only, and combined states. The root-owned socket-activated apply lane accepts only the fixed reboot request after validating that persisted config; it is not a general privilege broker. Orange Check, Apply, and Rollback use the separate root-owned guarded updater lane and only the profile-qualified runtime-updater asset; profile or asset failures stop without a Raspberry, manual-ZIP, or image fallback. The updater does not replace Armbian, kernel, device-tree, or image assets.
  - Selected Orange audio routes use the exact Jack (`hw:CARD=octesseradac,DEV=0`), UAC2 (`hw:CARD=UAC2Gadget,DEV=0`), and HDMI (`hw:CARD=HDMI,DEV=0`) PCMs. Simultaneous physical outputs use independent, unsynchronized clocks and can drift or echo; this phase provides no sample alignment. The observed Orange HDMI connector path is `/sys/class/drm/card0-HDMI-A-1`. A live Raspberry Pi Zero 2 W observation on kernel `6.12.93+rpt-rpi-v8` found the exact connector paths `/sys/class/drm/card0-HDMI-A-1/{status,edid}`; Raspberry runtime code pins that card0 identity and does not scan or fall back to card1. This establishes connector identity only, not connected HDMI audio or audible qualification.
  - Orange offline DSP profiling is a native `apps/pi-zero` diagnostic branch before hardware startup. It exercises the existing realtime-engine scenarios and reports snapshots; it does not implement playback, control, device, or TypeScript fallback behavior. Orange profiling requires the explicit `--profile-dsp` argument.
  - Orange runtime failure policy is three attempts in a 30-second systemd start-limit window: the initial start plus two `Restart=on-failure` retries at five-second intervals. After that, the unit stays `start-limit-hit` until an operator runs `sudo systemctl reset-failed octessera.service` and then `sudo systemctl start octessera.service`.
  - owns MIDI input/output through host adapters only; Tauri/midir and Pi MIDI device access stay outside canonical runtime crates

- Core logic layer (`crates/platform-core`)
  - deterministic behavior execution, grid state, interpretation, mapping, transforms, and native layer engine logic
  - generated platform capability constants from `resources/platform-capabilities.json`
  - checked-in generated display palette constants from `resources/display-palette.json`, copied into `OUT_DIR` by the `platform-core` build script, so runtime, Pi, and desktop adapters share color values without moving UI policy into the core
  - owns the fixed 8×8 logical row-major index and logical/display cell and index projection helpers; the lower-left world-space rule is pure native logic, not a runtime projection table
  - no UI framework code
  - no platform-specific I/O
  - no desktop, Pi, Tauri, Node runner, storage, MIDI-device, filesystem, or hardware adapter code

- Output adapters (`apps/desktop/src-tauri/src/`)
  - desktop audio sink maps native events/audio commands to the realtime engine and rodio source
  - MIDI input/output uses Tauri-side midir adapters
  - storage and sample-browser filesystem access remain host adapter responsibilities
  - desktop and Pi retain sample path resolution/containment, caching, preparation, preview/bank orchestration, and typed error adaptation
  - Pi setup adapters own the short-lived local Data Backup/Restore HTTP service, transfer-code authentication, archive streaming/staging, media filesystem I/O, physical restore confirmation forwarding, and the guarded atomic restore transaction. The service runs only with `Configure WiFi`; desktop returns typed setup `unsupported` and starts no transfer server.
  - Raspberry Pi device-update effects are executed by the host updater, which owns profile-qualified asset selection, checksum/manifest validation, candidate health guarding, and automatic fallback; `NativeRunner` owns menu/action semantics and confirmation. Orange device-update effects are executed by the Orange root-owned broker and guarded updater, which own profile-qualified selection of `octessera-<version>-orange-pi-zero-2w-runtime-updater-aarch64.zip` plus `SHA256SUMS-orange-pi-zero-2w-runtime-updater.txt`, checksum/manifest validation, candidate health guarding, and runtime rollback. Both paths fail closed on board or asset mismatch; neither path selects another board's asset or turns a manual/full-image install into an OTA fallback.
  - returns typed failure facts and carries runtime request/revision identity through asynchronous platform/audio-prep jobs; it does not choose recovery policy

- Realtime audio engine (`crates/realtime-engine`, `crates/rodio-engine-source`)
  - owns all internal musical audio rendering, instrument route/pan, FX bus sends, FX bus processing, sidechain ducking, and final stereo mix
  - generates synth slot/sample/pan constants from `resources/platform-capabilities.json`
  - `crates/rodio-engine-source` owns only shared non-realtime file opening and WAV decoding into `SampleBuffer`
  - `EngineSource` receives prepared sample buffers and control events; shared file opening/decoding remains strictly outside the audio callback
  - receives an explicit `AllNotesOff` internal command for clearing synth, sample, and preview voices; internal safety does not use MIDI CC 120/123
  - is the only path for synth/sample instrument audio before device output
  - shared JSON audio configuration normalization and FX shape/type validation live in `realtime-engine`; desktop and Pi retain sample preparation, preview/bank orchestration, and typed error adaptation
  - desktop and Pi return the same typed audio-command/config failures, preserve revision identity for full-config preparation, and route `SamplePreview` through the selected realtime instrument path

## Setup Portal Boundary

- `crates/playback-runtime` owns the menu/effect/status presentation. It does not execute setup, authorize a request, or resume playback after setup.
- For this seam, the fixed Pi adapters create only the non-authorizing 32-hex-character request-token marker at `/run/octessera/setup-portal.request`, then read strict, sanitized status and receipt envelopes from `/run/octessera-setup-status`. They do not read root-private control state, nonce material, credentials, or setup secrets.
- Root-owned `octessera-setup-request.service`, `octessera-setup.service`, `octessera-setup-sidecar`, `octessera-wifi-connect`, and the setup status helpers own request claiming, portal serving, Wi-Fi/hostname/SSH/login mutation, timeout, cleanup, and receipts.
- Desktop returns typed `unsupported` setup status. No TypeScript behavior, `sudo`, `systemctl`, secret handling, root-private state access, network discovery, or fallback path belongs in the runtime or desktop UI.
- During an active Pi setup session, the Pi adapter also binds the local transfer service to the setup hotspot and returns its URL and session code through `RuntimeSetupPortalStatus`; the native runtime presents them on the OLED. The service is local and short-lived, and is not a cloud or desktop transfer path.
- Data Backup/Restore archive validation and preference projection stay in `crates/playback-runtime`; Pi host code owns archive/media I/O and applies a physically confirmed, staged restore. Credentials, binaries/images, and hardware identity remain outside the portable bundle.

## Device Apply Boundary

- Orange's `/run/octessera-device-apply/reboot.sock` is a root-owned, group-limited systemd socket for exactly two requests: `reboot\n` and `poweroff\n`; it remains one fixed socket for compatibility and does not discover or select commands.
- `octessera.service` requires and starts `octessera-device-apply-reboot.socket` before the runtime; the socket is enabled separately through `sockets.target` and waits only for `local-fs.target`. Musical-default provisioning remains an independent runtime prerequisite, not a socket prerequisite. This dependency only repairs socket activation ordering and does not change the helper protocol or security boundary.
- The socket service validates the regular `/var/lib/octessera/presets/default.json` file, its expected runtime ownership/mode, and the canonical audio/gadget schema only for `reboot\n`, then invokes only `/usr/bin/systemctl reboot` or `/usr/bin/systemctl poweroff` for the matching exact request before returning `accepted\n`; malformed, unknown, extra-byte, timeout, or failed requests return `rejected\n` without invoking another command.
- Native Rust owns the confirmed save/apply/shutdown semantic and sends these fixed requests through the adapter lane after external MIDI panic and internal audio silence. The image helper owns only the fixed request validation, reboot config validation, and power command execution; it does not accept command arguments, config payloads, or arbitrary commands. Orange has no `octessera-runtime` sudoers entry.
- Orange runtime updates use the separate root-owned `/run/octessera-update/update.sock` broker. It accepts only `check\n`, `apply\n`, and `rollback\n`, invokes the guarded updater, and returns bounded results; it does not expose a general command or privilege broker. The updater changes only the managed runtime release and binary link. Full Armbian, kernel, device-tree, and image replacement remains a manual image operation, and the standalone manual runtime ZIP is not an OTA asset.

## OLED Boot Handoff Boundary

- The final systemd paths on the fixed Raspberry Pi Zero 2 W and Orange Pi 2W use the same source-defined OLED boot sweep from `resources/oled/boot-sweep-v1.json`: the mounted SSD1351 controller origin decreases by 303 px while the physical panel motion is left-to-right. In canonical bottom-to-top coordinates the top-row origin is 127 px less X than the bottom-row origin (`bottom_origin - row_y`); this produces the physical-panel slash, with the current magenta, green, yellow, and cyan palette, 8 px bands, and 4 px separators. The sweep uses 30 frames over a 1,200,000,000 ns cycle (25 fps); the clean logo+wordmark frame follows it and continuous boot loops retain that frame for a responsive 2,000,000,000 ns inter-loop rest. This is a source and test contract, not a claim of live visual qualification.
- Raspberry and Orange root-installed systemd animators are the sole loops and run concurrently with service loading after their fixed OLED devices are ready. Raspberry explicitly wants and waits for `systemd-udev-settle.service` before opening its GPIO/SPI devices; it does not depend on unavailable device units or polling. Either board may remain blank until systemd starts its animator. Native startup requests release, waits for the shared OLED lock, adopts the already-initialized display without reset, and stops the animation immediately before publishing the acknowledged first normal menu frame. Sleep and shutdown are separate OLED lifecycle paths, not boot-handoff states.
- Native runtime owns the confirmed instrument-menu lifecycle presentation. `Going to sleep`, `Shutting down`, and `Rebooting` are exact native toasts over the shared static `SPLASH_SLEEP_SHUTDOWN` background and existing toaster geometry. Before ordinary Reboot or Shutdown power submission, native force-publishes and acknowledges the latest snapshot, zeros the grid and NeoKey LEDs, preserves the OLED pixels/on state while detaching its handles, and only then invokes the board adapter power path. This contract does not claim presentation for arbitrary administrative `systemctl poweroff` or `systemctl reboot` commands.
- Orange's boot-loop handoff has a monotonic 30-second deadline that begins immediately after `handoff.start()`, before OLED initialization or adoption. Matching stop requests win at the deadline and release without black/off; timeout, signal, and post-ownership failures attempt an exact 32768-byte black RGB565 frame, then display-off `0xAE` independently, close once, and publish one attachable `failed` state. A failed handoff keeps the current boot ID and matching valid request ID so native recovery can attach it. Signal handlers only set a flag and do no I/O.
- Orange NeoTrellis operation requires the exact validated bus, wiring, and addresses; adapters do not provide an alternate bus, address, or hardware fallback.
- The menu `OLED Sleep` setting is display-only UI behavior. Linux suspend is a separate Orange `sleep.target` ownership transaction. Production installs `octessera-orange-oled-suspend.service` through `RequiredBy=sleep.target`, creating the hard `sleep.target.requires` relationship rather than a `.wants` relationship; a failed preparation therefore blocks suspend. The runtime quiesces and preserving-detaches its OLED handles, a strict AF_UNIX helper renders the suspend/resume frames, and the runtime reacquires the lock and hardware without changing the established `first_menu_rendered` handoff phase. Audio, MIDI, transport, and LED contracts remain unchanged.
- The board runtime owns `/run/octessera-boot`, including `oled.lock`, `status.json`, and `stop.request`. The lock is exclusive: the animator, native runtime, and lifecycle utilities may not write the OLED concurrently. `status.json` uses the strict phases `animating`, `release_requested`, `released`, `native_owned`, `first_menu_rendered`, and `failed`; the normal path reaches `first_menu_rendered`, while matching failure state remains attachable for recovery.
- First-menu readiness requires an actual OLED write acknowledgement. Every non-empty output set is valid. A selected Jack route is required only when selected, and a selected route fault blocks readiness; recognized disconnected selected USB or HDMI routes may wait. No route falls back to another route. A snapshot being queued, or a runtime service merely starting, is not readiness.
- Normal Raspberry and Orange output apply their board-specific clockwise RGB565 framebuffer transform (`source (x, y) -> transmitted (127 - y, x)`) in the HAL before writing accepted native RGB565BE bytes. Orange uses a reusable 32 KiB transform buffer, preserves RGB565 byte pairs, and keeps SSD1351 remap `0x74`. The Python splash is a separate renderer with the same physical orientation; its SSD1351 command byte is sent with D/C low and command data with D/C high, matching the Rust transport framing.
- Raspberry's selected initramfs writes one clean logo+wordmark frame and does not sweep, loop, publish a marker, or adopt the OLED. Its root-installed systemd service remains the sole animator and runs concurrently with service loading; Orange's selected initramfs likewise writes one static RGB565 frame with its fixed Python closure, while its root-installed renderer, lifecycle modules, assets, and `octessera-orange-boot-splash.service` remain the only loop. These are adapter differences around one shared handoff contract.
- Boot source, service, and selected-initramfs changes are constructor-required for both boards. Trusted `v0.7.5` runtime/setup parent respins are boot-neutral and cannot claim this layer; no full constructor or production image has been built for this handoff yet.

## Dependency Rules

- UI may import type contracts and render snapshots only.
- UI must not call native core, transport, audio, MIDI, or storage bridges directly.
- Runtime may import native core and output/input adapters.
- Core crates must stay platform-agnostic.
- `crates/platform-core` and `crates/playback-runtime` must not depend on Tauri, HAL, Pi hardware crates, Node runner processes, storage implementations, or host filesystem/sample-browser adapters.
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
- Host -> runner messages are limited to `device_input`, `transport_pulse_step`, split MIDI realtime wire messages (`midi_realtime_clock`, `midi_realtime_start`, `midi_realtime_continue`, `midi_realtime_stop`), and `runtime_result`.
- Runner -> host messages include `snapshot`, optional `oled_frame`, `platform_effects`, `musical_events`, `midi_events`, `audio_commands`, and `runtime_status`. Every `snapshot` leaving `PlaybackRuntime` carries a positive OLED revision. `oled_frame` is changed-frame-only and precedes its matching snapshot; Raspberry and Orange pair accepted native frame bytes to normal snapshot publication or retained-last-good bytes during a typed cache fault; desktop remains a separate native consumer.
- Shared fixtures for this seam live in `SHARED_RUNTIME_CONTRACT_FIXTURES` so both hosts can validate the same contract examples.
- `transport_pulse_step` is the deterministic PPQN advancement boundary; hosts must not substitute wall-clock timer semantics above this seam.
- External MIDI realtime (`clock`, `start`, `continue`, `stop`) remains explicit at the boundary and is not inferred from UI/runtime scheduling code. Desktop MIDI input is routed natively from the host adapter into the runtime worker; UI code must not observe raw MIDI bytes for display or transport state.
- `runtime_result` carries host-side outcomes for storage, MIDI port enumeration/selection, sample-browser operations, device-update status, and asynchronously identified sanitized system-info requests back into the shared runner.
- `SystemInfoRequest` is a typed platform request; adapters return typed `SystemInfoResult` or identified `SystemInfoError` values. The native runner owns loading/error/unavailable presentation, row formatting, clipping, scrolling, and dismissal. Desktop UI renders only the resulting snapshot/OLED frame.
- `NativeRunner` may emit an internal typed runtime-config change during dispatch; `PlaybackRuntime` consumes it before returning presented host messages. It is not a host adapter responsibility and is never reconstructed from snapshot fields.
- Central modulation processing is the single native path for held tick/XY sources and global LFO output: behavior ticks and active XY captures update held sources, global LFOs advance only at 24 PPQN, and the process sums persistent, held, and LFO contributions, clamps once, and applies once per target endpoint. An active LFO step processes only dirty LFO endpoints plus other contributors sharing those keys; unrelated held grid/XY sources are retained without being resolved, reapplied, or cloned. Ordinary menu/Aux base edits rebase and recompose only the edited key/endpoint, preserving held sources so clearing a source restores the new base; changed persistent targets from one tick share one revision and delayed autosave request. Config/patch transactions reset candidate modulation state, install every persistent owner, then resample active XY so its captured base is the loaded owner value. Enabled targeted LFO phases advance and wrap even at depth zero; no other PPQN or wall-clock path may advance an LFO or reapply a held source.
- Background audio preparation returns identified typed success/failure results through `runtime_result`; prep failures retain the last good runtime/audio state.
- Sample-bank preparation is atomic on both hosts: every configured sample path must resolve and decode before a new bank is queued; unresolved or undecodable samples return typed `sample` failures and leave the previous banks and signatures in place. Sample preview resolution/decoding runs on the audio-prep worker, never on the runtime or host-adapter thread, and reports success only after the prepared preview reaches the audio queue.
- Pi audio preparation treats superseded revisions as cancellation: no stale-prep fault is returned or latched.
- Identified asynchronous results retain their request ID/revision through the runner round trip; `PlaybackRuntime` observes each result once and clears only the matching fault.
- Emission and persistence faults clear only after the corresponding native emission or identified save/recovery acknowledgement succeeds. Native save confirmation/toast feedback is emitted after that acknowledgement, not when a save request is queued.
- `octessera.patch` schema version 2 is the portable preset contract: it carries musical patch state and sampler paths, while device-local settings and device/system aux bindings remain host-local; musical aux bindings travel with the patch. Portable parity evidence is equality of the patch projection plus verified sample-path loadability, not equality of the full device config or a physical-board FAT result. Full default, recovery, backup, and confirmed device-apply payloads remain local configs.
- Stop-and-silence recovery independently attempts runner transport stop, internal synth/sample silence, and external MIDI panic on both hosts.
- `snapshot` is the runtime display/input-facing state payload; `musical_events`, `midi_events`, `platform_effects`, and `audio_commands` are the resolved outputs that Rust schedules or dispatches.
- `oled_frame` carries a positive revisioned 128×128 `rgb565be` Base64 payload on the JSON bridge; typed native pixels remain immutable bytes before serialization. Desktop, Raspberry, and Orange adapter caches stage candidates and promote only on matching snapshot references, validate dimensions, format, Base64, exact byte length, revision idempotency/conflicts, and expose sticky typed cache faults. Same-revision byte conflicts remove candidates or poison accepted revisions until a strictly newer valid candidate matches a snapshot. Raspberry and Orange normal OLED publication consumes accepted native bytes with no semantic renderer fallback; before acceptance they publish explicit black, and after acceptance they retain last-good bytes rather than black on a bad reference.

## Audio Routing Contract

- The shared runtime audio rate is 44.1 kHz.
- Internal synth and sample instruments must enter the realtime engine before audio output.
- Instrument `Route=direct` bypasses FX bus processing and pans directly into the main mix.
- Instrument `Route=fx_bus_n` enters the selected FX bus, runs its slot FX in order, then pans into the main mix.
- MIDI instruments emit external MIDI/control data and are not an internal audio source unless an audio return path is explicitly added.
- MIDI-only instrument notes and CCs use the `midi_events` path and must not call host internal-audio musical event handling.
- Sample browser preview is musical audio and must route through the selected instrument slot, pan, volume, FX bus, and master output path.
- The default sample artifact inventory has 320 attribution rows: 318 WAV rows are loadable through the sampler browser/decoder, and two AIFF rows remain inventory metadata outside that WAV-only load path. Artifact staging may carry the complete inventory without making all rows sampler-loadable.
- Runtime audio config commands carry `sound.voiceStealingMode`; host adapters forward it to the realtime audio policy.
- `gridBrightness` is applied by core LED frame rendering; `displayBrightness` is owned by `PlaybackRuntime`'s OLED presentation conversion and carried into native frame bytes; `buttonBrightness` is applied by desktop and Pi only to the native unscaled `neoKeyLeds` values with the shared basis-point dim rule.

## Grid Coordinate Contract

- Core logic uses a world-space grid origin at lower-left: `(0,0)` is bottom-left, `y` increases upward.
- UI/hardware-facing layers may use screen-space coordinates (top-left origin), but conversion is only allowed at boundaries.
- `platform-core` owns the fixed 8×8 pure logical row-major and logical/display projection helpers. `playback-runtime` delegates its existing display-index API to those helpers.
- `packages/device-contracts/src/gridDomain.ts` is the deliberate TypeScript boundary mirror for desktop input/rendering; it is checked against the exhaustive `resources/grid-projection-v1.json` fixture.
- The HAL keeps fixed NeoTrellis quadrant/key wiring separate from the native projection owner. Its four-device addresses, physical key ordering, and GRB output are adapter facts, not generic geometry.
- In code, grid coordinate conversion must go through these centralized boundary helpers rather than ad-hoc math; the checked fixture is verification data, not a runtime table.
