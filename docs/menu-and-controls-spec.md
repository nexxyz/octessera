# Menu and Controls Spec (Authoritative)

This is the entry point for the canonical menu/control spec. The full menu tree is split into `docs/menu-tree-spec.md`; that file is part of this authoritative spec and must stay in sync with native menu changes.

Context-help copy source: `resources/menu-help-texts.tsv` (required header row). Each row provides a title plus two short text fields. Keep one idea per text field; the runtime may join and wrap them for the target display.
Platform capability source: `resources/platform-capabilities.json`; generated TypeScript and Rust constants must stay in sync with it. Display palette source: `resources/display-palette.json`; generated TypeScript, CSS, and Rust constants must stay in sync with it.

## Cheat Sheet

| Combo | Function | Notes |
|---|---|---|
| Shift + Space | Emergency Stop | Internal sync: panic + stop/reset.
| Shift + Space (external sync) | Resync arm | External sync: does not emergency-stop transport.
| Shift + Back | Clear active layer | Re-initializes current active layer behavior state.
| Shift + Fn | Combined modifier | Acts as its own logical button; Fn and Shift are inactive while both physical buttons are held.
| Combined modifier + Main press | Context help | Opens help for highlighted menu entry.
| Fn + Main encoder turn right | Single step | While paused/stopped, advances exactly one behavior generation and remains paused/stopped; while playing, shows `Pause first`. Turning left is consumed with no action.
| Fn + Space | Reset stop | Stops, resets transport position, and sends MIDI panic; takes priority over sample preview.
| Combined modifier + Space | Reserved | No-op.
| Fn + leftmost grid column | Navigate layers (1..8) | Mirrors `Build > Layer`.
| Combined modifier + leftmost grid column | Layer trigger gate toggle | Toggles that layer between `0%` and its previous trigger mode without changing active layer.
| Fn held + leftmost column LEDs | Navigation indicators | Cyan = navigation/current layer focus, green = configured layer, gray/black = inactive or non-navigable.
| Fn + rightmost grid column | Navigate Play pages | Opens `Play` and enables Play page if currently off; exits Play if already active.
| Combined modifier held LEDs | Layer-only indicators | Shows the left/layer side of Fn navigation only; Play page column is hidden/reserved.
| Sample assign + Shift + cell | Row assign step | Applies current selected-cell assign step to the whole row.
| Sample assign + combined modifier + cell | Column assign step | Applies current selected-cell assign step to the whole column.
| Fn + Aux press | Alternate aux binding | Binds the focused bindable value as that aux Turn target, or focused action as its `!` press target.

## Control Mapping

| Control | Simulator Key | Function |
|---|---|---|
| Main encoder turn | ← → | Move cursor / adjust values |
| Main encoder press | Enter | Enter group / enter/exit edit / trigger action |
| Back button | Backspace / Esc | Go back / exit edit / clear grid (with Shift) |
| Space button | Space | Play / Pause |
| Shift + Space | Shift+Space | Emergency stop (panic + reset scan origin) |
| Fn + Space | Ctrl+Space | Reset stop (panic + reset scan origin) |
| Shift + Fn + Space | Shift+Ctrl+Space | Reserved no-op |
| Shift + Back | Shift+Backspace / Shift+Esc | Clear grid (re-initialize behavior) |
| Aux encoder 1-3 turn | (simulated) | Adjust bound turn mapping |
| Aux encoder 1-3 press | (simulated) | Trigger bound press mapping |
| Fn + Aux encoder press | Fn + (simulated) | Alternate action: bind current value as Turn target or current action as `!` press target |
| Shift + Aux encoder turn/press | Shift + (simulated) | Use shifted aux binding bank |
| Shift + Fn + Aux encoder press | Shift+Ctrl + (simulated) | Bind current value/action into shifted aux binding bank |
| Shift + Fn | Shift+Ctrl | Combined modifier; acts as its own logical button and disables Fn/Shift functions while both are held |
| Combined modifier + Main press | Shift+Ctrl+Enter | Context help for highlighted entry |
| Fn + Main encoder turn right | Ctrl+→ | Single behavior/world generation step while paused/stopped |
| Fn + leftmost grid column | Ctrl + leftmost grid column | Navigate active layer (1..8); hold Fn to see layer indicators |
| Fn + rightmost grid column | Ctrl + rightmost grid column | Navigate/activate Play Play page; hold Fn to see page indicators |
| Shift + Fn + leftmost grid column | Shift+Ctrl + leftmost grid column | Toggle that layer's trigger gate without changing active layer |
| Sample assign mode + Shift + cell press | Shift + cell | Apply current assign toggle/level step to entire row |
| Sample assign mode + combined modifier + cell press | Shift+Ctrl + cell | Apply current assign toggle/level step to entire column |

Simulator grid drag behavior follows the active behavior's declared interaction mode. Paint behaviors drag-toggle/draw cells for editing; momentary behaviors such as Keys release the previous cell when the pointer enters another cell, matching a single finger sliding across grid buttons.

Help popup behavior:

- Main encoder turn scrolls help text
- Main encoder press closes help

## Transport States

- Play: `▶` (green flash on full-note/measure boundaries, yellow flash on other beat boundaries)
- Pause: `⏸`
- Stop (emergency): `■`

## Menu Tree

The full native menu tree lives in [`menu-tree-spec.md`](menu-tree-spec.md). Keep that file in sync with native menu/control changes.

## OLED Display

- 128×128 pixel, simulated in desktop app
- 20 characters × 8 lines of text (5×7 font, 16px line height)
- Top line: title bar (colored by section)
- Canonical display palette names are independent from menu sections: `GREEN` `#63D23F` for `Build`, `RED` `#DD82CD` for `Link`, `BLUE` `#35CFF2` for `Shape`, `YELLOW` `#FFD447` for `Play`, `GRAY` `#C9CED6` for `System`, plus white `#FFFFFF` and black `#000000`. Runtime, Pi, and desktop UI colors use this palette unless a behavior deliberately owns its own palette.
- Body lines 2-8: menu items use a `> ` marker and inverted highlight on the selected row, and `* ` when editing; while browsing, selected value rows stay compact on one row (for example `> Cutoff 127`) instead of adding a separate value row
- Native menu snapshots include rendered-row scroll metadata (`scrollOffset`, `totalRows`, `visibleRows`) for the current body window. Desktop renders this as a 1-2 px scrollbar inside the OLED body only when total rendered rows exceed visible body rows; it does not consume text columns and is omitted for splash/help/confirm overlays unless menu metadata is present.
- Context help for every submenu, parameter, and action must resolve to a specific row from `resources/menu-help-texts.tsv`; generic fallback help is not allowed and native tests must fail on missing coverage.
- Platform-sized menu/runtime limits such as layer count, instrument count, sample slots, bus count, global FX slots, Play-FX concurrency, scan section counts, OLED size, and pan position count come from `resources/platform-capabilities.json`.
- Splash graphics use provided logo assets: regular logo for startup/wakeup, sepia logo for sleep/shutdown.
- Bottom-right corner: transport icon (`▶` / `⏸` / `■`), hidden while a footer toast is active
- Transport color: stop is magenta, pause is cyan, play is white at rest and flashes green on full-note/measure boundaries or yellow on other beat boundaries. The NeoKey Play button uses the same stopped/paused/playing flash semantics, but its playing rest state stays dim green rather than white.
- Event dot: briefly shown when notes fire, hidden while a footer toast is active; turns magenta when recent voice stealing occurred
- Top-right audio load indicator: hidden when idle, yellow when DSP load is moderate or recent voice stealing occurred, magenta when DSP load is heavy
- Toast text: displayed at bottom for feedback messages

Value editing semantics:

- Number/enum/bool rows enter edit mode on main press
- Navigation memory is limited to `System`, `System > Sound`, and `System > UI`. It is native, ephemeral, cleared on menu rebuild, and does not apply to any other menu, dynamic list, sample browser, preset list, MIDI port list, parameter picker, help, confirm dialog, or assignment overlay.
- `System > Sound > Output Buffer` persists Pi CPAL/ALSA output buffer frames as `runtimeConfig.sound.audioOutputBufferFrames` with choices `64/128/256/512/1024/2048`, default `256`. Changing it shows `Restart device to apply`; leaving the edited row opens the standard `Confirm Reboot` dialog. Audio is not reopened live. Internal engine block frames and synth-slot worker count are platform capabilities, not menu/runtime settings. On Pi startup, `OCTESSERA_AUDIO_OUTPUT_BUFFER_FRAMES`, `OCTESSERA_AUDIO_BLOCK_FRAMES`, and `OCTESSERA_SYNTH_SLOT_WORKERS` remain higher-priority development/profiling overrides for their respective settings.
- `System > USB` persists `runtimeConfig.usb.audioOut` (`jack|usb|both`, default `jack`) and `runtimeConfig.usb.midiOutEnabled` (default `false`). Pi preserves the selected audio output mode across boot and USB host unplug/replug: `jack` opens only jack, `usb` retries USB gadget audio without silently becoming `jack` or `both`, and `both` keeps jack playing while USB gadget audio appears or disappears. Changes are restart-applied and show `USB: Save & Reboot`. `Save & Reboot` opens a confirmation with `Cancel` and `Save & Reboot`; the confirm path emits `RuntimePlatformEffect::UsbApplyReboot { payload: config_payload() }` so the platform saves the payload before applying/rebooting. `Start SD2 Xfer` and `Stop SD2 Xfer` are transient actions for the OLED microSD card (`SD2`), not the Pi boot microSD (`SD1`) and not config toggles. Start confirms that USB audio/MIDI disconnect and the host owns OLED SD2, stops playback without auto-resume, opens a blocking `SD2 Transfer` popup, emits `RuntimePlatformEffect::UsbSdTransferStart`, and ignores all device input except Back or main encoder click while the popup is open. If no USB host is connected, Pi leaves the mass-storage gadget waiting for a host and the popup remains cancellable. Pressing Back or main encoder click from the popup emits `RuntimePlatformEffect::UsbSdTransferStop` and keeps transport stopped; the user should eject the drive on the host first. Pi rejects start while USB audio out, USB MIDI out, or recording is active, sends MIDI panic/all-notes-off, and switches the gadget to writable mass storage. Stop confirms the user should eject on the host first, then emits `RuntimePlatformEffect::UsbSdTransferStop` and restores normal USB audio/MIDI gadget setup. General MIDI enable/input/clock/sync remains under `System > MIDI`.
- `System > Recording` persists settings only: `runtimeConfig.recording.maxMinutes` defaults to `10` and clamps to `1..120`. `Start Audio` emits `RuntimePlatformEffect::RecordingStartAudio { max_minutes }`; `Stop` emits `RuntimePlatformEffect::RecordingStop`. On Pi, Phase 1 records the final internal stereo output as 44.1 kHz/16-bit WAV files under `/home/pi/recordings/`. It does not capture external MIDI instrument audio, OLED frames, display SD storage, USB audio input, or MIDI input.
- Browsing selected values are shown on the selected label row; edit mode uses a separate value-focused row for clarity.
- Breadcrumbs use full labels for the current submenu and short labels for ancestors, e.g. `/S/FX/Bus 1` and, one level deeper, `/S/FX/B1/Slot 1`. Top-level ancestors use `Build`/`B`, `Link`/`L`, `Shape`/`S`, `Play`/`P`; layer ancestors use `Layer N`/`LN`; FX bus ancestors use `Bus N`/`BN`. Overlong breadcrumbs are front-ellipsized with `...` so the current location remains visible. Section color follows the canonical section path, not the truncated display text.
- Rows that lead to a submenu or selector render with a trailing `>` marker. `Build > Layer > Behavior: <id>` is a synthetic browser-style selector, not an editable enum. It groups behavior rows under `[Human]`, `[Rhythm]`, `[Musical]`, then alphabetically under `[Cellular]`, `[Fields]`, `[Geometry]`, `[Growth]`, and `[Motion]`, uses `..` rows for parent navigation, and writes the selected native behavior ID to that layer's persisted `behaviorId` field. Human includes direct play plus `arp` and `weave`; Rhythm includes `polyrhythm`, `breaks`, `fills`, `clave`, `groove`, and `euclid`; Musical includes `ostinato`, `motif`, `canon`, `chords`, `contour`, `cadence`, and `phrase`. Selecting a behavior uses a targeted native Build refresh for that layer and does not rebuild the full menu tree. `glider` is no longer a behavior ID; its glider injection controls are part of `life`. `forest_fire` is the canonical Forest Fire behavior ID, with no `forest` alias. `bubbles` belongs to Motion; its current, drift, and buoyancy rows use eighth-cell units per tick, and `Add Bubble` spawns one bottom-origin rising bubble immediately.
- Forest Fire renders trees and burning cells as visible, but event interpretation follows the behavior's trigger types: tree-to-fire and manual ignition emit activate triggers, burned-out cells emit deactivate triggers, visible non-burning trees are stable, and unrelated empty cells emit no event.
- `crystal_growth` is the canonical Crystal Growth behavior ID, with no `crystal` or `crystals` alias. It belongs to Growth before `dla`. `cross` grows through cardinal neighbors only; `diagonal` grows through diagonal neighbors only; `snowflake` grows through cardinal neighbors plus parity-selected diagonals: even `(x + y)` uses NW/SE, odd uses NE/SW. Grid press seeds or refreshes the exact lower-left world-space cell without toggling it off or changing its phase; scheduled/action seeding chooses deterministic cells.
- `lightning` is the canonical Lightning behavior ID, with no aliases. It belongs to Fields before `raindrops`. Target edges use lower-left world space: north is `y=max`, south is `y=0`, east is `x=max`, and west is `x=0`; automatic strikes seed from the opposite edge. On the connection tick, all visible lightning cells emit activate once, then remain stable during decay and deactivate when cleared.
- `kuramoto` is the canonical Kuramoto behavior ID, with no aliases. It belongs to Fields before `lightning`. Cells are visible only near the sync/wrap window; phase wraps emit activate, stable sync-window cells stay quiet, grid press sets the exact lower-left world-space cell just before wrap and emits activate, and `Desync Pulse` perturbs phases without immediate activate events.
- `wave` is the canonical Wave behavior ID, with no aliases. It belongs to Fields near `raindrops` but uses oscillating displacement/velocity rather than ripple rings or diffusion. Grid press applies an impulse to the exact lower-left world-space cell and emits activate; threshold crossings activate/deactivate as the wave propagates and damps. `Impulse Interval` and `Spawn Step` schedule deterministic small impulses so the default patch keeps breathing.
- `gravity` is the canonical Gravity behavior ID, with no aliases. It belongs to Motion after `bubbles`. It is falling granular sand only: gravity directions use lower-left world space, movement into a new cell activates the destination and deactivates the origin, unchanged settled grains are stable, and near-saturated/stalled defaults drain a few settled grains instead of staying full.
- `boids` is the canonical Boids behavior ID, with no aliases. It belongs to Motion after `bubbles` and `gravity`. It renders many quantized flocking agents; cell entry activates, vacated cells deactivate only when no boid remains, and scatter changes velocity without immediate activation.
- `orbit` is the canonical Orbit behavior ID, with no aliases. It belongs to Motion after `boids`. Particles orbit one moving attractor; particle cell entry activates, vacated cells deactivate only when no particle remains, and attractor-only cells render stable except press/reset forced accents.
- `sand_ripples` is the canonical Sand Ripples behavior ID, with no aliases. It belongs to Motion after `orbit`. It models wind-driven grain transport and migrating crests, with no water flow or gravity avalanche behavior; saturated defaults shed tiny deterministic gaps so dunes keep moving.
- `lava_lamp` is the canonical Lava Lamp behavior ID, with no aliases. It belongs to Motion between `boids` and `orbit`. It renders soft moving blobs/metaball-like fields that merge and split; it is not flocking, orbiting, or passive diffusion.
- `ink` is the canonical Ink behavior ID, with no aliases. It belongs to Fields before `ising`. Ink diffuses toward cardinal-neighbor average and fades; direct drops force activate, passive threshold crossings activate/deactivate, and low pigment remains quiet. `Drop Interval` and `Spawn Step` schedule deterministic small drops for default liveness.
- `ising` is the canonical Ising behavior ID, with no aliases. It belongs to Fields between `ink` and `kuramoto`. It models binary magnetic domains with temperature, noise, and field bias; spin flips to +1 activate, flips to -1 deactivate, unchanged +1 cells remain stable, and -1 cells are quiet.
- `reaction_diffusion` is the canonical Reaction-Diffusion behavior ID, with no aliases. It belongs to Fields between `raindrops` and `wave`. It uses two-chemical Gray-Scott-style integer pattern formation; B concentration drives visibility, upward threshold crossings activate, downward visibility crossings deactivate, and grid presses splash chemicals into the exact lower-left world-space cell plus cardinal neighbors. `Seed Interval` and `Spawn Step` schedule deterministic small chemical splashes for default liveness.
- `rivers` is the canonical Rivers behavior ID, with no aliases. It belongs to Fields between `reaction_diffusion` and `wave`. It models water flow over height with erosion/deposition, cardinal non-wrapping downhill movement, and visible water threshold triggers.
- `cracks` is the canonical Cracks behavior ID, with no aliases. It belongs to Growth before `crystal_growth`. Crack tips propagate through stressed cells, new tips activate, stress-only cells are stable/quiet, and shatter/replace removes visible pane cells in bounded staged passes with deactivate triggers.
- `coral` is the canonical Coral behavior ID, with no aliases. It belongs to Growth before `cracks`. Exposed cardinal colony surfaces grow, adjacent opposing colonies become skeletons instead of directly converting, breakaway clearing deactivates removed cells, and full defaults thin a few cells deterministically instead of staying solid.
- `physarum` is the canonical Physarum behavior ID, with no aliases. It belongs to Growth after `dla`. Bounded agents follow trail and food, deposit evaporating memory, food is stable/quiet, and seed slime forces new agent-cell activate accents.
- `vines` is the canonical Vines behavior ID, with no aliases. It belongs to Growth after `physarum`. Directional tendril tips seek light and open space, branches reserve empty cells without wrapping, pruning deactivates removed vines, full defaults shed a few old cells deterministically, and direct planting uses exact lower-left world-space cells.
- `fractal_explorer` is the canonical Fractal Explorer behavior ID, with no aliases. It belongs to Geometry before `shapes`. It dynamically samples Mandelbrot/Julia regions with drift and zoom; class increases activate, class disappearance deactivates, and grid press recenters the exact world-space cell with a one-shot forced activate. Its mode action key is `toggleFractalMode`.
- `maze_growth` is the canonical Maze Growth behavior ID, with no aliases. It belongs to Geometry between `fractal_explorer` and `shapes`. It carves one-cell corridors from frontiers, moves walkers over visible cells, never wraps neighbors, and collapse/removal deactivates visible cells.
- `predator_prey` is the canonical Predator–Prey behavior ID, with no aliases. Grass persistence and regrowth are visible, quiet `Stable` background events; animals emit activate on entry, birth, and reseed, deactivate on move-out or death, and stable while persisting. Predator eating a herbivore adds a one-tick trigger-only cardinal burst without mutating neighbor cells, so visible grass cells may briefly emit activate as predator-event accents. Saturated defaults reopen a few cells instead of staying full.
- `cyclic` is the canonical Cyclic behavior ID, with no aliases. It belongs to Cellular between `brain` and `forest_fire`. Cells chase the next discrete state through a clipped Moore neighborhood; advancement emits activate except wrap-to-zero, which emits deactivate. Zero cells are inactive and quiet.
- Bool behaves like a 2-option enum (`off`/`on`) and changes on encoder turn, not immediate row press
- Named target selectors (instrument slot, layer index, mixer route) display their computed names via `formatDisplayValue()` (e.g. `I1: synth`, `L3: rain`, `fxb2`)
- Behavior `none` hides Build Step Rate, dynamic behavior config rows, and Reset while preserving stored values. Instrument Type `none` hides Note Mode, engine-specific params, mixer/MIDI rows, and Slot Actions while preserving stored config.
- Parameter target pickers mirror the main menu root order (`Build`, `Link`, `Shape`, `Play`, `System`) so modulation, Aux, and XY target browsing use the same mental model as normal navigation. Within `Build`, Behavior targets are generated per layer: layers with behavior `none` expose no behavior targets; real behavior layers expose their own Step Rate as `layers.N.algorithmStep` and config fields/actions as `layers.N.worlds.behaviorConfig.*`.
- Numeric XY and layer X/Y parameter-mod bindings may store optional user `Range Min`/`Range Max` values. These constrain modulation output while preserving the target capability `min`/`max` metadata. Enum and bool bindings ignore user ranges.
- Each layer has one `Link > L* > LFO` sine modulator stored as `layers.N.linkLfo` with `enabled`, numeric `target`, `period`, and `depthPct`. It is disabled with no target by default, runs only while transport is playing, advances one full sine cycle per selected 24 PPQN note period, freezes when paused, and resets phase on stop/reset. Link LFO targets are restricted to live audio-facing numeric controls; transient LFO values are sent as live audio commands, are restored to the saved base value on stop/reset/config changes, and are not saved into target values. High-rate unsafe FX timing/buffer params such as delay time mode/note/ms and mod-delay base/depth ms are not Link LFO targets.
- Each layer has `Link > L* > Arp` stored as `layers.N.pulses.arp` with `mode`, `source`, `stepIntervalSteps`, `noteLengthMs`, `gatePct`, and `octaveSpread`. Defaults are `none`, `simultaneous`, `1`, `120`, `80`, and `0`. Sources are `simultaneous` routed note-on batches and playback-runtime tracked `held` notes; unsupported sources normalize to `simultaneous`. `none` preserves the normal Link path. Other modes emit finite note-ons using `noteLengthMs * gatePct / 100` and do not create held notes; matching note-offs update held membership and do not cut off arp-owned finite notes. `stepIntervalSteps` is clamped to 1..16, `noteLengthMs` to 10..2000, `gatePct` to 1..100, and `octaveSpread` to 0..3.
- Link event mappings (`activate`, `stable`, `deactivate`, `scanned`, `scanned_empty`) have per-target Delay and Retrig controls. Delay is counted in that layer's link ticks; Retrig is extra repeats after the original at delay+1, delay+2, and so on. Trigger probability is evaluated once before scheduling.
- Musical timing selectors use the 24 PPQN vocabulary `1/32T`, `1/32`, `1/16T`, `1/16`, `1/8T`, `1/8`, `1/4T`, `1/4`, `1/2T`, `1/2`, `1/1T`, `1/1`. Straight `1/64` is intentionally not exposed because it is 1.5 pulses at 24 PPQN; triplet values are exact.
- When `Number Style` is `bar` or `bar+numbers`, bounded sound/control/behavior number items keep the numeric value on the selected text row and render the smooth geometric bar on the next body row, so the value is not shortened to make room for the bar
- Bar display applies automatically to FX params, synth/sample shaping controls, editable mixer volume/pan, FX bus volume/pan, Play FX controls, system sound/UI controls, Link axis controls, and behavior controls such as spawn interval/count, threshold, lifespan, and radius
- DLA has `Cell Life` (`0..256`, default `96`) so old aggregate cells age out and the cluster keeps renewing instead of filling the grid forever. `0` disables DLA aging/removal. If aging removes the whole cluster, DLA reseeds its small starter cluster.
- Selector-like numeric rows stay plain text, including MIDI channels, instrument/sample slots, layer selectors, and MIDI note ranges
- Structural selector edits apply immediately while the row is in edit mode through key-specific fast paths. This covers instrument type, instrument route, FX bus slot type, and master FX slot type. Behavior selection applies immediately when a behavior action row is pressed. Dynamic parameter rows also apply immediately while editing.
- FX buses expose three ordered mono-chain slots: `Slot 1`, `Slot 2`, and `Slot 3`, with keys under `mixer.buses.N.slot1.*`, `slot2.*`, and `slot3.*`. When bus config is missing, the menu displays shipped defaults (`Slot 1: Delay`, `Slot 2: Duck`, `Slot 3: None`) rather than selecting the first option by accident. Old runtime/default configs that omit `slot3` load it as `none`; saved configs include explicit `slot3`. Global/master FX remains two stereo slots and does not expand with bus slot count.
- The Pi active bus FX warning budget is 12 active bus slots, matching the current 4 buses × 3 slots maximum after Pi DSP profiling. The warning budget excludes the two global/master FX slots and does not reject saved patches.
- Bus Delay FX exposes `Mix %`, `Spread %`, `Time Mode`, `Time Note`, and `Time ms`. Editing `Time Note` switches to note mode and materializes `timeMs` from the current BPM; later BPM edits retime note-mode bus Delays. Editing `Time ms` switches to ms mode and remains manual. Runtime/audio commands carry `timeMs` only, while `timeMode` and `timeNote` persist as patch metadata and are excluded from modulation, Aux, and XY binding targets. `Spread %` is 0..100 and widens only the final FX bus output; instruments, sampler voices, bus sends, and the FX slot chain remain mono. Delay Mix 0 with Spread 100 produces no widening.
- Bus input and Slot 1→2→3 processing remain mono. Bus `Volume`, `Pan Pos`, and delay `Spread %` apply only at the final bus output stage before summing into the main mix.
- Bar value text uses compact units where useful: `%`, `ms`/`s`, `Hz`, `bpm`, `dB`, semitones/cents, and pan as `L15`/`C`/`R15`; ambiguous internal `0..1` ranges display as `0..100`
- `Link > Swing` is a global groove amount. `0%` is straight timing. Swing delays internal off-beat step/scan progression and catches up before the next beat; external MIDI clock output remains straight.

Action row markers:

- `!` prefix means the row is an action item
- Plain action rows reduce one leading display space so the action text aligns visually with ordinary menu item text despite the `!` marker. Auto-mapped rows keep the normal alignment because `1-` and `1!` prefixes are equal width.

## Grid LED Behavior (NeoKey per-key RGB)

Each cell in the 8×8 grid is mapped to an LED with color based on its behavior palette and `CellTriggerType`. Every behavior provides inactive, active, and stable colors. Defaults are inactive black, active yellow, and stable green. Inactive black is preferred unless a behavior needs a different off-state color.

| Condition | Color |
|---|---|
| Cell off | Behavior inactive color |
| `activate` | Behavior active color |
| `stable` | Behavior stable color |
| `deactivate` | Gray |
| `scanned` | Cyan (only if scan mode is "scanning") |

Brightness is scaled by the Grid Bright setting after the behavior palette is applied. Runtime snapshots also expose logical active-cell booleans so simulator paint controls do not infer cell state from RGB values.

Overrides:

- While Fn is held for navigation: non-navigation cells are fully off. The leftmost column shows navigation/current-layer focus cells in cyan, configured layers in green when Play is not active, and inactive/non-navigable cells in dim gray. The rightmost Play page column uses yellow for page cells, green for the active page, and dim gray for non-page cells. While Shift+Fn is held, only the left/layer column is shown and the Play page column is hidden/reserved. The Fn navigation overlay is suppressed while sample assignment, trigger probability assignment, or Sparks FX assignment overlays are active.
- While sample assignment mode is active: grid shows assignment overlay using magenta for high, yellow for medium, green for low, gray for other assigned cells, and black for unassigned dark cells.
- While any Play Page (`mix`, `pan`, `fx`, `trigger-gate`, `transpose`, `xy`) is active: grid shows the Play performance overlay instead of active behavior cells. Play Transpose uses the left column for eligible layer selection, Shift + left column to enable/disable all eligible layers, and columns 1..7 as a three-octave piano offset picker for synth and enabled MIDI note targets only. In the transpose picker, the selected offset is green, the unselected center key is white, and available offsets are dim blue. Held transposed notes are safely drained with exact routed note-offs when transpose routing is retargeted, disabled, stopped, or reset.
- In `Play`, `Mix`, `Pan`, `Trigger Gate`, and `Transpose` act as page-select rows: main encoder press selects and activates the page without entering an empty submenu. `FX` and `XY` remain normal enterable menu groups because they expose configuration rows.
- When Ghost Cells is on, inactive layers' active cells render as very dim green behind the active layer. Active layer cells and sample assignment overlays take priority.
- Active context changes use OLED toast/status feedback, for example `Layer: L3 rain` or `Play: fx`; these toasts do not change LED overlay priority. Modal help/confirm displays keep display priority over context feedback.
- Holding Shift, Fn, or Shift+Fn for more than one second without another mapped action shows a concise hint toast (`Shift: map/edit`, `Fn: nav/alt`, or `Help: Sh+Fn+Enter`). Startup uses the same chord wording: `Help: Sh+Fn+Enter`. Existing toasts, help/confirm dialogs, assignment overlays, and consumed mappings suppress the hint.

## Sectioned Scanning

- `Sections=1` preserves current scan behavior: `columns` scans one full column per step; `rows` scans one full row per step.
- `Sections=2`, `4`, or `8` split the perpendicular axis into that many lanes and scan each lane in sequence.
- For `rows` with `Sections=2`, each lane is 4 rows tall; the scan ray moves left-to-right across lane 1, then lane 2. Total steps: `gridWidth * sections`.
- For `columns` with `Sections=2`, each lane is 4 columns wide; the scan ray moves bottom-to-top/top-to-bottom by row across each lane. Total steps: `gridHeight * sections`.
- Stop/emergency reset scan index to origin.
- `Restart Section` on Pitch Steps makes pitch stepping local to the lane for the matching scan orientation: X restart applies to column sections; Y restart applies to row sections.
- Note mapping builds the concrete notes in `Low Note..High Note` that match `Scale` and `Root`, chooses the nearest scale note to `Start Note` as the zero-degree index, and applies X/Y pitch steps before clamp/wrap. `wrap` wraps within that concrete scale-note list, so wrapped notes must remain in scale.

## Auto-Save

- Location: System > Saves > Default > Auto Save
- Location: System > Saves > Default > Backups
- When enabled: native menu edits and aux-bound value changes emit deferred `store_save_default` effects; fast audio-facing edits update state/audio immediately and coalesce `ConfigPayload` generation for about 150ms so storage writes the latest settled value instead of saving every intermediate encoder step
- Disabled by default
- Toggling Auto Save on triggers an immediate save when you exit that menu row
- Explicit Save Default is always immediate and cancels any pending deferred default save
- Backups are enabled by default. When any persistent config changes, runtime may emit `store_save_backup` at most once every five minutes; hosts keep the latest 20 `bak-{timestamp}.json` files.
- Confirmed shutdown/reboot emits `store_save_recovery`; Pi writes the latest recovery payload synchronously before setting the power request.
- Loading default, preset, or factory config stops transport, resets position, and sends MIDI panic/equivalent note clearing before applying the loaded config.
- `System > Saves > Load Empty` opens `Confirm Load Empty`. Cancel is a no-op. Confirm stops playback with the same MIDI panic/note-safety path, loads an empty `none`-behavior patch, regenerates the preset draft name, marks config dirty for autosave, and preserves device/user preferences: brightness, ghost cells, numeric display, sleep/dim timers, master volume, autosave/backup settings, MIDI setup/status/sync settings, sync source, audio output buffer frames, sample favourites, input-events-while-paused, aux auto-map enabled, and the available preset name list.

## Aux Encoder Binding

- Each aux encoder has two independent custom slots:
  - turn slot: bound to value parameters (number/enum/bool)
  - press slot: bound to actions
- Each aux encoder also has a separate shifted custom bank with the same turn/press slot shape. Shift + aux turn/press uses only the shifted bank; plain aux turn/press uses the normal bank plus auto-map fallback. `Link > Aux Mappings` labels these rows `Trn`, `Clk`, `S+Trn`, and `S+Clk`.
- Fn + aux press is an alternate action on a bindable item that binds/overwrites the relevant custom slot:
  - while editing a value item: binds Turn slot
  - while selecting an action item: binds `!` press slot
- Shift + Fn + aux press binds/overwrites the shifted custom bank instead of the normal bank.
- In the Fn-held aux overlay, plain labels are turn targets and `!Label` entries are press actions; `/` means both slots are present for that encoder.
- Regular aux press triggers the press slot action (if any)
- Regular aux turn adjusts the turn slot value (if any)
- Aux toasts use compact labels such as `Trn-1`, `Clk-1`, `S+Trn-1`, and `S+Clk-1`.
- `Auto Map` lives under `System > UI`. When enabled, context-sensitive auto mappings fill unbound aux slots for the active menu context; custom aux bindings keep precedence when present.
- Auto-map does not fill shifted aux slots; shifted aux bindings are custom-only and persist as `runtimeConfig.shiftAuxBindings`, mirroring `runtimeConfig.auxBindings`.
- In supported contexts, focused menu rows show auto-map indicators like `1-Cutoff` and `1!Assign`, preserving selection markers on focused rows such as `> 1!Assign`.
- If no slot is bound, toast shows labels like `Trn-1: No binding` or `S+Clk-1: No binding`
- Turn toasts show current value, e.g. `Trn-1: Spawn Count: 3`
- Shared route currently implemented:
  - `trigger.life.spawn_now` resolves per behavior (sequencer has no implementation)
- Enum turning is clamped (no wrap)
- Bool turning is clamped with directional behavior (`-1 => Off`, `+1 => On`)
- `activeBehavior` and `behaviorConfig.*` updates re-initialize behavior state
- All aux value changes schedule the deferred auto-save when enabled

### Stale (Inactive) Binding Detection

- Bindings are **not** automatically removed when the target context changes
- If a bound target becomes inactive, the input is ignored and a scoped `not active` toast is shown
- The binding remains intact so the user can re-activate the target later

#### Turn (Stale Target)
- **FX param**: param does not exist for the current slot type, e.g. `Trn-1: B1 Time ms not active`
- **Instrument subtree**: instrument type changed away from the bound subtree, e.g. `Trn-1: I1 Filter cutoff not active`
- **Layer scan field**: `scanMode` is not `"scanning"`, e.g. `Trn-1: L1 Scan Direction not active`
- **Behavior config param**: param is not in the current behavior's `configMenu()`, e.g. `Trn-1: L1 Spawn Count not active`

#### Press (Stale Action)
- **Spawn route**: current behavior has no spawn action, e.g. `S1: L1 Spawn Now not active`
- **Concrete action**: action type is not in current behavior's `configMenu()`, e.g. `S1: L1 Spawn Random not active`

#### Scope Prefixes
- `B<N+1>` — bus number (1-indexed)
- `I<N+1>` — instrument number (1-indexed)
- `L<N+1>` — layer number (1-indexed)
- Global behavior config uses active layer scope `L<active+1>`

### Toast Scrolling

- Toast messages are rendered on a single OLED bottom line (max 17 chars visible)
- Messages longer than 17 chars scroll horizontally:
  - Hold at start: 700ms
  - Scroll at 120ms per character
  - Hold at end once the final window is reached
- `startedAtMs` tracks the original toast creation time; extending a visible toast preserves the scroll position

## Config Persistence (ConfigPayload)

- Native `ConfigPayload` is produced and consumed by `crates/playback-runtime/src/native_runner.rs`.
- It stores active behavior, per-layer behavior/config/state, Link settings, mapping, instruments, mixer, FX, Play settings, MIDI settings, UI settings, and persistence flags.
- Restore accepts current payloads and supported older saved shapes, sanitizes external compatibility data, then applies only native-owned runtime/core fields.
- Behavior state is restored when saved and compatible; behavior changes initialize the new behavior state through the native behavior engine.
- Transport timing accumulators are reset on restore so loaded configs start from a deterministic runtime position.

## Brightness Behavior

- OLED Bright scales OLED display intensity in host display adapters. Grid Bright and Button Bright scale their LEDs; the Dim Timer applies an additional sleep dim with a small visible floor so low brightness settings do not make dimmed LEDs look fully off.
- Grid Bright scales matrix LED RGB intensity.
- Button Bright scales NeoKey button LED intensity.

## Modulation Behavior

- Pitch modulation is additive across axes (`X Steps + Y Steps`).
- Axis pitch steps are signed (`-16..16`).
- Pitch note generation uses scale-degree stepping (not post-quantize).
- `Velocity` lane modulates outgoing `note_on` velocity.
- `Filter Cutoff` lane emits CC74 (mapped to lowpass cutoff).
- `Filter Res` lane emits CC71 (mapped to lowpass resonance).
- `Grid Offs` rotates axis indexing (offset=5 => cell 5 treated as first, then wraps).
- `Grid Offs` bounds are derived: `-(GRID_SIZE-1) .. +(GRID_SIZE-1)` → `-7..7`.

## Edit Marker

- Selected editable value line uses compact marker: `*Value`.
- In text edit mode: `*` prefix and cursor shown within the text.

## Native Behavior Contract

Native behaviors implement the Rust `BehaviorEngine` trait in `crates/platform-core/src/behavior.rs` and are registered from `crates/platform-core/src/behaviors/`.

Behavior engines provide:

- stable behavior id
- initial state from config
- input and tick transitions
- render model for the grid
- serialization/deserialization for saved state
- optional behavior config menu rows
- optional immediate input-transition interpretation
- optional grid interaction mode such as paint or momentary

All behaviors use `CellTriggerType`: `activate`, `stable`, `deactivate`, `scanned`, or `none`.

### Input Events

`DeviceInput` supports `grid_press` and `grid_release` events. Behaviors that do not handle `grid_release` simply ignore it. `keys` uses press→activate and release→deactivate semantics; `looper` uses the same live semantics and can overdub step-quantized press/release events into its loop.

Looper uses a `Punch In/Out` action instead of an editable mode row. Pressing it toggles between overdub and play, preserves the recorded loop and live playback state, and shows `Looper: Overdub` or `Looper: Play`.

When a behavior enables immediate input-transition interpretation, `platform-core` interprets grid changes from input through the same Link/mapping pipeline used during tick, producing immediate musical events. `keys` and `looper` use this to provide immediate finger-drumming response.

## 4 Trigger Types

| Type | Source | When |
|---|---|---|
| `activate` | Algorithm | Cell becomes active (birth, shape hits cell, etc.) |
| `stable` | Algorithm | Cell stays active (alive, inside shape interior, etc.) |
| `deactivate` | Algorithm | Cell becomes inactive (death, shape leaves cell, etc.) |
| `scanned` | Scanning layer | Cell found active during scan (only in "scanning" mode) |

Scan mode "none" generates NO `scanned` triggers. Only "scanning" mode (column/row) generates `scanned` triggers.
`State Notes` only controls non-scan state-note events; `scanned` triggers remain active while scanning.

## Maintenance Rule

Any control/menu/runtime behavior change must update this document in the same commit.
