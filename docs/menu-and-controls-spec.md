# Menu and Controls Spec (Authoritative)

This is the contract that the native runtime, the two Pi boards, and the desktop simulator are built
and tested against. It runs long because a hardware detent and a simulator keypress have to do the
same thing, down to the toast text.

## How the spec is split

| File | Owns |
|---|---|
| This file | Controls and modifiers, value editing, OLED and LED presentation, overlay priority, aux bindings, modulation, save/load, and the native behavior contract |
| [`menu-tree-spec.md`](menu-tree-spec.md) | Root menu, tree notation, and the index of section trees |
| [`menu-tree/build.md`](menu-tree/build.md) | Build tree, behavior categories, per-behavior config rows, behavior notes |
| [`menu-tree/link.md`](menu-tree/link.md) | Link tree, event mapping and timing, LFOs, Arp, X/Y axis slots, Aux Mappings |
| [`menu-tree/shape.md`](menu-tree/shape.md) | Instruments, FX buses, global FX, routing, sample and drum assignment |
| [`menu-tree/play.md`](menu-tree/play.md) | Play pages, their grid layouts, Play FX, XY |
| [`menu-tree/system.md`](menu-tree/system.md) | System tree per platform and every System setting and action |

All seven files are one spec. Each statement lives in one file and the others link to it. Any
control, menu, runtime, or parity-affecting change updates the file that owns the statement in the
same commit.

Data sources the spec relies on:

- Help copy: `resources/menu-help-texts.tsv`, with the required header row
  `id path key kind title line1 line2`. Each row matches a menu entry and supplies a title plus two
  short text fields. Keep one idea per field; the runtime joins and wraps them for the display.
  Every submenu, parameter, action, and non-file selector must resolve to an entry, and native tests
  fail on missing coverage. Behavior selector leaves use behavior-specific help, behavior category
  rows use the stable `behavior.category.*` keys, parameter-picker leaves resolve to the target
  parameter's help, and dynamic file rows (sample browser files and folders) keep their `sample.*`
  action help.
- Platform dimensions and limits: `resources/platform-capabilities.json` (layer count, instrument
  count, sample slots, bus count, global FX slots, aux encoder count, Play FX concurrency, scan
  section counts, OLED size, pan positions). Generated TypeScript and Rust constants must match it.
- Display palette: `resources/display-palette.json`. Generated TypeScript, CSS, and Rust constants
  must match it. See [Palette](#palette).
- Shipped defaults: `config/defaults/base.json` with the `desktop.json` and `pi.json` overrides.

## Cheat Sheet

| Combo | Function | Notes |
|---|---|---|
| Shift + Space | Stop | Internal sync: stop/reset, silence internal audio, and clear held MIDI notes without broad panic. |
| Shift + Space (external sync) | Resync arm | Arms a one-shot resync at the next 96-PPQN (one-bar) boundary. Playback and the grid continue; at the boundary the transport origins reset to zero and the arm clears. |
| Shift + Back | Clear active layer | Re-initializes the active layer's behavior state. |
| Shift + Fn | Combined modifier | Acts as its own logical button. Fn and Shift functions are inactive while both physical buttons are held. |
| Combined modifier + Main press | Context help | Opens help for the highlighted menu entry. |
| Fn + Main press | Aux mappings modal | Opens the scrollable mappings modal from anywhere, stopped or playing. Holding Fn alone does not open it. |
| Fn + Main turn right, outside edit | Single step | While paused or stopped, advances exactly one behavior generation and stays paused/stopped. While playing, shows `Pause first`. Turning left is consumed with no action. Numeric edit takes priority over this gesture. |
| Main numeric edit turn | Coarse/fine value change | Plain Main is coarse; Fn+Main is fine. Shift+Main and Shift+Fn+Main stay coarse. Fn does not change enum, bool, or text edit turns. |
| Aux1–3 numeric turn | Coarse/fine bound value change | In the normal custom/auto-map target, a plain turn is coarse and an Fn turn is fine. In the shifted custom bank only (no fallback), Shift is coarse and Shift+Fn is fine. |
| Fn + Space | Reset stop | Stops/resets, silences internal audio, and clears held MIDI notes without broad panic. Takes priority over sample preview. |
| Combined modifier + Space | Reserved | No-op. |
| Fn + leftmost grid column | Select layer (1..8) | Makes that layer active and leaves any Play overlay. The menu position does not change. |
| Combined modifier + leftmost grid column | Layer trigger gate toggle | Toggles that layer between `0%` and its previous trigger mode without changing the active layer. Only this disable transition releases held Synth, FM, Plucked, and Sampler notes owned by that layer, plus its external MIDI notes; an already-ringing one-shot Drum hit finishes its decay. Ordinary Play Trigger Gate edits to `0%` only suppress future admissions. Other layers are unaffected, and re-enabling does not resurrect released notes. |
| Fn held | Navigation LEDs | Left column: blue = active layer (or, while a Play page is active, every configured layer), green = configured layer, dim gray = `none` layer. See [Grid overlays](#overlays-and-priority). |
| Fn + rightmost grid column | Select Play page | Selects and activates the page on that row (rows 0..6) and moves the menu to it. It never exits Play; Back or Fn + leftmost column leaves the Play overlay. |
| Combined modifier held | Layer-only LEDs | Shows only the left (layer) column of Fn navigation; the Play page column is hidden and reserved. |
| Sample/Drum assign + Shift + cell | Row stamp | Stamps the current selection on the whole world-space row. |
| Sample/Drum assign + combined modifier + cell | Column stamp | Stamps the current selection on the whole column. |
| Fn + Aux press | Alternate aux binding | Binds the focused bindable value as that aux Turn target, or the focused action as its `!` press target. |

## Control Mapping

| Control | Simulator key | Function |
|---|---|---|
| Main encoder turn | ← → | Move cursor / adjust values |
| Main encoder press | Enter | Enter group / enter or exit edit / trigger action |
| Back button | Backspace / Esc | Go back / exit edit / clear grid (with Shift) |
| Space button | Space | Play / Pause |
| Shift + Space | Shift+Space | Stop/reset with internal-audio silence and bounded held-note cleanup; under external sync, arms the next 96-PPQN boundary instead |
| Fn + Space | Ctrl+Space | Reset stop with internal-audio silence and bounded held-note cleanup |
| Shift + Fn + Space | Shift+Ctrl+Space | Reserved no-op |
| Shift + Back | Shift+Backspace / Shift+Esc | Clear grid (re-initialize behavior) |
| Aux encoder 1-3 turn | (simulated) | Adjust a numeric target coarsely; enum/bool targets keep ordinary discrete turns. Orange SW1/SW2/SW4 use the native GPIO event path |
| Aux encoder 1-3 press | (simulated) | Trigger the bound press mapping. Orange AUX2's switch line is unavailable only while UART0 TX is active and is enabled by the input-routing overlay |
| Fn + Aux encoder press | Fn + (simulated) | Alternate action: bind the current value as Turn target or the current action as `!` press target |
| Fn + Main encoder press | Fn+Enter | Open/close the Aux mappings modal; holding Fn alone does not open it |
| Shift + Aux encoder turn/press | Shift + (simulated) | Numeric turns are coarse; uses only the shifted aux binding bank |
| Fn + Aux encoder turn | Fn + (simulated) | Fine numeric turn on the normal custom/auto-map target |
| Shift + Fn + Aux encoder press | Shift+Ctrl + (simulated) | Bind the current value/action into the shifted aux binding bank |
| Shift + Fn + Aux encoder turn | Shift+Ctrl + (simulated) | Fine numeric turn using only the shifted custom bank |
| Shift + Fn | Shift+Ctrl | Combined modifier; acts as its own logical button and disables Fn/Shift functions while both are held |
| Combined modifier + Main press | Shift+Ctrl+Enter | Context help for the highlighted entry |
| Fn + Main encoder turn right outside editing | Ctrl+→ | Single behavior generation step while paused/stopped. While editing, numeric values turn finely and other edit types keep their ordinary turn |
| Fn + leftmost grid column | Ctrl + leftmost grid column | Select the active layer (1..8); hold Fn to see layer indicators |
| Fn + rightmost grid column | Ctrl + rightmost grid column | Select/activate a Play page; hold Fn to see page indicators |
| Shift + Fn + leftmost grid column | Shift+Ctrl + leftmost grid column | Toggle that layer's trigger gate without changing the active layer (see the Cheat Sheet for note release) |
| Sample/Drum assign + Shift + cell press | Shift + cell | Stamp the entire row |
| Sample/Drum assign + combined modifier + cell press | Shift+Ctrl + cell | Stamp the entire column |

### USB-host keyboard

The base keyboard controls mirror the simulator keys. USB-host capture also maps the three AUX
encoder rows:

| Keyboard key | Native control | Repeat |
|---|---|---|
| ← / ↑ | Main encoder turn `-1` | Accepted |
| → / ↓ | Main encoder turn `+1` | Accepted |
| Q / W / E | AUX1 turn `-1` / click / turn `+1` | Turns accepted; click suppressed |
| A / S / D | AUX2 turn `-1` / click / turn `+1` | Turns accepted; click suppressed |
| Y/Z / X / C | AUX3 turn `-1` / click / turn `+1` | Turns accepted; click suppressed |
| Enter | Main encoder press | Suppressed |
| Backspace / Escape | Back button | Suppressed |
| Space | Space button (`S`) | Suppressed |
| Shift | Shift modifier | Suppressed |
| Control | Fn modifier | Suppressed |

- Key presses and releases update the matching native button or modifier state. Key-repeat on
  arrow and AUX turn keys emits extra turns; repeats of Enter, the AUX click keys, Back, Space,
  Shift, and Control emit no extra edges.
- Releasing an AUX click key clears its held keyboard state without retriggering the click.
  Disconnect, disable, read failure, and shutdown use the same held-state cleanup.
- The AUX3 left key is the physical key right of left Shift. Linux reports that position as
  `KEY_Z` whether the keycap says `Y` or `Z`.
- Space is the hardware Space/S button; the letter `S` is the AUX2 click. Modifier combinations
  keep their meaning: Shift+Space is Stop (Resync arm under external sync), Control+Space is Reset
  stop, and Shift+Control+Enter opens context help.
- Raspberry captures only while the persisted USB role is `Host`; changing the role still uses the
  save-and-reboot flow. Orange captures on its fixed USB-A host connector, independent of USB0
  OTG/gadget behavior.
- Both boards capture only in the graphical HDMI modes `live-grid`, `plain-grid`,
  `active-behavior`, and `cycle-behaviors`. `Terminal` disables capture. While Bluetooth is On,
  Bluetooth keyboards are accepted under the same gate.
- Hotplug and HDMI-mode changes apply live. The first supported keyboard is selected and grabbed
  exclusively, a second keyboard is ignored while one is selected, and a disconnect or gate change
  releases held logical controls and the grab.

### Input dispatch

- Simulator grid drag follows the active behavior's interaction mode. Paint behaviors drag-toggle
  or draw cells; momentary behaviors such as Keys release the previous cell when the pointer enters
  another one, like a finger sliding across the grid buttons.
- Raspberry and Orange share one encoder dispatch: consecutive same-direction turns from one encoder
  coalesce and clamp to a `-127..127` delta, reversals stay ordered, and pending turns dispatch
  before an encoder press. Each input pickup drains at most 16 encoder events so other runtime work
  is not starved.

### Help popup and Aux mappings modal

Help popup:

- Combined modifier + Main press opens it. Main turn scrolls the text; Main press closes it.

Aux mappings modal:

- Fn + Main press opens it from anywhere, stopped or playing. Releasing Fn does not close it, and
  holding Fn alone does not open it.
- It lists all 12 mappings, with unbound entries shown as `-`:

  | Bank | Aux | Turn | Click |
  |---|---|---|---|
  | Normal | 1..3 | Mapping or `-` | Mapping or `-` |
  | Shifted | 1..3 | Mapping or `-` | Mapping or `-` |

- Normal mappings show the custom binding first, then auto-map. Shifted mappings come from the
  shifted custom bank only, with no fallback.
- Main turn scrolls. Aux turns and clicks do nothing while it is open and do not touch the
  underlying menu or bindings.
- Plain Main press, Fn + Main press again, or Back closes it. Shift + Fn + Main press opens context
  help instead. After closing, Main, Shift+Main, Fn fine turns, and Aux bindings work as usual.

## Transport

- Icons: Play `▶` (green flash on full-note/measure boundaries, yellow flash on other beats),
  Pause `⏸`, Stop `■`.
- Stop, accepted MIDI Start, and resync reset the transport origin, the generic scan cursor, the
  autonomous Build cadence, Pattern phase, and the Looper playback position. Evolved worlds,
  configuration, and recorded loops are kept; other behavior-owned state follows each behavior's
  reset contract.
- While transport runs, clearing or replacing a behavior reinitializes or restores its own state but
  keeps the transport-owned interpretation cursor, fractional accumulator, and page/section phase.
- Sequencer alone has origin-immediate static-grid semantics: page 1 step 1 sits on the transport
  origin, empty leading cells are timed rests, and later pages continue without restarting.
- Armed external resync: clocks strictly before the next 96-PPQN boundary process normally. The
  boundary clock is consumed by the reset; scanning Sequencer layers process their origin tick at
  reported PPQN 0, and non-Sequencer layers get no zero-elapsed tick. The next clock reports PPQN 1
  and does not repeat the origin tick.
- Pause and layer trigger-gate changes preserve phase, and Pause/Continue resumes the exact phase.
  A true stop/reset returns to origin.

## Seeded Randomness

- Randomness is unseeded unless a source opts in.
- `Build > Seed` (`randomSeed`, `1..9999`, default `1`) is one shared per-patch seed.
- `Build > Lx > Seeded` (`layers.N.build.seeded`, default off) makes that layer's behavior
  randomness, including hash-style choices and the Rhythm/Musical/weave pattern seed, follow a
  stream mixed from Seed and the layer index.
- `Link > Lx > Seeded` (`layers.N.link.seeded`, default off) does the same for that layer's trigger
  probability and random arp order. Unseeded Link layers share the unseeded streams.
- Seeded streams restart on every transport reset (stop, play-from-stop, accepted MIDI Start,
  resync), on patch load, and when Seed or the layer's Seeded switch changes. The same starting
  world, inputs, and timing then replay the same result on every platform.
- Restarting a stream never resets an evolved grid, so an exact replay starts from the same world
  (a loaded patch with saved grid state, or Reset).
- Instrument and FX audio-rate noise is never seeded.

## Menu Structure

- The full tree is in [`menu-tree-spec.md`](menu-tree-spec.md). The root order is Build, Link,
  Shape, Play, a blank spacer row, then System.
- Rows that open a submenu or selector render a trailing `>`.
- System groups list parameters and submenus before action rows, with two exceptions:
  `Maintenance` keeps the `Reset` group between `Hardware Test` and `Sys. Info`, and the dynamic
  `Rename` group lists preset choices before `New Name` and `Apply`. Other sections use the orders
  shown in their trees.
- Navigation memory applies only to `System`, `System > Notes`, `System > Audio`,
  `System > Audio > Engine`, and `System > Display`. It is native and ephemeral, is cleared on menu
  rebuild, and does not apply to any other menu, dynamic list, sample browser, preset list, MIDI port
  list, parameter picker, help, confirm dialog, or assignment overlay.
- Breadcrumbs show the current submenu label in full and its ancestors short, front-ellipsized to
  28 characters with `...` so the current location stays visible. Short forms: `Build`/`B`,
  `Link`/`L`, `Shape`/`S`, `Play`/`P`, `System`/`SYS`; ancestor labels starting `Layer N` or
  `Bus N` shorten to `LN`/`BN`. Real paths look like `/B/L1: life`, `/S/FX Buses`, and
  `/SYS/Maintenance/Reset`. Section color follows the canonical section path, not the truncated
  text.
- Named target selectors (instrument slot, layer index, mixer route) display computed names, for
  example `I1: synth`, `L3: rain`, `fxb2`, never raw IDs.
- The instrument selectors under `Link > Lx > Scanning` and `Link > Lx > Events` keep options as
  `none` plus `I#: name`. Unselected rows show only `I#`. The selected row uses exactly two body
  rows: its plain heading, then `> I#: name` (`>* I#: name` while editing). Long selected details
  scroll horizontally while browsing and editing.

### Parameter target pickers

Modulation slots, Aux Turn bindings, Play XY axes, and LFO targets share one menu-mirrored picker.

- Children: `(none)`, then `Current: <label>` when bound, then `Range Min` and `Range Max` when the
  binding is numeric and the picker is not an Aux Turn picker, then the target tree.
- The tree mirrors the root order (`Build`, `Link`, `Shape`, `Play`, `System`). Under Build, layers
  with behavior `none` expose no behavior targets; other layers expose their own Step Rate as
  `layers.N.algorithmStep` and their config fields/actions as `layers.N.build.behaviorConfig.*`.
- `Range Min`/`Range Max` constrain the output of every source (slots, XY, LFOs) while keeping the
  target's capability `min`/`max` metadata. Missing sides fall back to the target range; equal
  values produce a fixed value. Enum and bool bindings ignore user ranges.
- LFO target pickers list only numeric, live-safe targets and never another LFO's settings. The
  instrument-specific numeric targets are listed in [shape.md](menu-tree/shape.md#binding-targets).

## Value Editing

- Number, enum, and bool rows enter edit mode on Main press; Main or Back finishes the row. In the
  restart flow, Back means Cancel at the `Apply System` choice and Continue at the `Restart?`
  choice.
- Bool rows behave like a two-option enum, display `On`/`Off`, and change on encoder turn, not on
  row press. Text rows show a `*` prefix and an in-text cursor while editing.
- Structural selector edits apply immediately while the row is in edit mode, through key-specific
  fast paths: instrument type, instrument route, FX bus slot type, and master FX slot type. A
  behavior applies when its selector item is pressed. Dynamic parameter rows also apply while
  editing.

### Numeric steps

The same rule covers Main and Aux turns, coarse and fine (Fn). Saved values and ranges never
change; only the distance per detent does.

| Row kind | Coarse detent | Fine (Fn) detent |
|---|---|---|
| Linear (default) | `step × m`, where `m = (span + 10·step) / (20·step)` rounded down and clamped to `1..5` | one `step` |
| Times: every row whose key ends in `Ms`, except XY Smoothing | 1 ms below 20 ms, 5 ms to 100 ms, 25 ms to 1 s, 100 ms above | 0.1 ms below 20 ms where the value stores tenths (otherwise 1 ms), 1 ms to 100 ms, 5 ms to 1 s, 25 ms above |
| Frequencies and Q: keys ending in `Hz` except instrument `cutoffHz`, plus FX `Q` and `Mid Q` | multiply by 2^(1/24), about a quarter-tone | multiply by 2^(1/96) |
| Reverb `Decay` and delay `Feedback` (non-negative ranges) | distance to the top (1.0, or 100%) scales by 2^(1/4) | scales by 2^(1/16) |

- Time steps snap to the step size; frequency steps always move at least one stored unit. Curved
  rows ignore the row's linear `step`.
- Time rows include envelope Attack/Decay/Release on every instrument; Plucked Decay; Drum Decay,
  Attack, and Sweep ms; `Note Length`, Arp `Length ms`, MIDI `Duration`, bus FX delay/slice/
  compressor/duck times, mod-delay `Base ms` and `Depth ms` (stored in tenths), and the Play FX
  times (Freeze `Release Ms`, Filter Sweep `Sweep In`/`Sweep Out`, Pitch Shift `Slide In`/
  `Slide Out`).
- Frequency rows include Plucked `Body Hz`, FX `Rate Hz`, `Center`, EQ `Mid Freq Hz`, and the Play
  Stutter `Rate Hz`. Instrument filter cutoffs keep their logarithmic `0..255` index with linear
  steps.
- Bipolar mod-delay `Feedback` stays linear. `Play > XY > Smoothing` stays on plain 10 ms steps
  because saved smoothing is 0 or a multiple of 10 ms.

## OLED Display

### Frames and layout

- Native presentation publishes a positive-revision `oled_frame` (`128x128`, `rgb565be`) only when
  the bytes change, immediately before the semantic snapshot that references `oledFrameRevision`.
  PlaybackRuntime owns OLED brightness conversion and never emits a snapshot with revision `0`,
  including when presentation input is absent or malformed. Before the first valid frame an invalid
  snapshot is suppressed and a typed fault/status is emitted; after a valid frame, non-OLED
  snapshot state continues with the prior positive revision and a typed retain-last-good
  fault/status.
- Desktop consumes these runtime frames. Raspberry and Orange render OLED, LEDs, and HDMI from the
  native presentation scene on the render worker, with the runtime error state attached. No
  TypeScript or semantic Pi renderer fallback is permitted.
- On Raspberry and Orange, the first menu frame at boot, runtime-error and dismissal frames, the
  power terminal frame, and the Raspberry audio-fault frame are native presentation scenes. The
  first scene is physically written before native scenes, HDMI, or candidate readiness proceed. A
  changed snapshot revision schedules a scene even when the display generation is unchanged, so
  host-side state such as runtime errors reaches the screen. Boot splash, direct boot fault,
  shutdown splash, suspend, and ownership lifecycle frames remain adapter paths.
- Geometry: 5×7 glyphs on a 6 px advance. The top line is the title bar, colored by section. The
  menu body is 19 columns by 7 rows (`x=6`, `y=18`, 13 px rows). The toast line at the bottom is 17
  columns wide.
- Every snapshot `display` object carries the required `bodyLayout`:
  - `rows` keeps the menu/help/sample/restore/confirmation/System Info/concise-MIDI row
    semantics, including the 28-character/7-row normalization, bars, selected-line scrolling, and
    scrollbars.
  - `card` is used for the setup portal and Backup / Restore status and keeps every complete
    semantic source string in `display.lines`. Cards use `x=4`, `width=120`, `y=18`, `height=91`,
    `rowAdvance=13` (20 columns by 7 rows), reserve the final row for the selected action, and
    ignore row bars and scroll metadata. When card prose wraps or later lines do not fit, only the
    final visible prose row gets `...`. The native and Pi renderers share the fixed-font metrics and
    wrapping rules.
- Splash graphics use the shared static `SPLASH_SLEEP_SHUTDOWN` logo and wordmark for sleep and
  shutdown with the existing toast geometry. The sleep toast is exactly `Going to sleep`. Startup
  and wake keep the regular logo.

### Palette

Palette names are independent of menu sections. Runtime, Pi, and desktop UI colors use this
palette unless a behavior deliberately owns its own.

| Name | Hex | Section |
|---|---|---|
| `GREEN` | `#63D23F` | Build |
| `MAGENTA` | `#DD82CD` | Link |
| `BLUE` | `#35CFF2` | Shape |
| `YELLOW` | `#FFD447` | Play |
| `GRAY` | `#C9CED6` | System |
| `WHITE` | `#FFFFFF` | |
| `BLACK` | `#000000` | |

### Body rows

- Menu rows use a `> ` marker with an inverted highlight on the selected row, and `* ` while
  editing. While browsing, a selected value row stays compact on one line (for example
  `> Cutoff 127`); edit mode shows a separate value-focused row.
- `!` marks an action row. Plain action rows drop one leading display space so the text lines up
  with ordinary rows. Auto-mapped rows keep normal alignment because the `1-` and `1!` prefixes are
  the same width.
- Snapshots include rendered-row scroll metadata (`scrollOffset`, `totalRows`, `visibleRows`).
  Desktop draws a 1-2 px scrollbar inside the OLED body only when rendered rows exceed visible
  rows. It uses no text columns and is omitted for splash/help/confirm overlays unless menu metadata
  is present.
- When `Number Style` is `bar` or `bar+numbers`, bounded number rows keep the value on the selected
  text row and draw a compact bar on the next body row, so the value is not shortened. Bars use the
  row/menu color, keep a visible bounding box when empty, partial, or full, draw marker-style
  values as a tick inside the box, and keep contrast on highlighted rows.
- Bars apply to FX params, synth/FM/sample shaping controls, editable mixer and FX bus volume/pan,
  Play FX controls, System sound/UI controls, Link axis controls, and behavior controls such as
  spawn interval/count, threshold, lifespan, and radius. Selector-like numeric rows stay plain text:
  MIDI channels, instrument/sample slots, layer selectors, and MIDI note ranges.
- Bar value text uses compact units: `%`, `ms`/`s`, `Hz`, `bpm`, `dB`, semitones/cents, pan as
  `L15`/`C`/`R15`. Internal `0..1` ranges display as `0..100`.

### Display priority

1. On Pi, `display.off` keeps the OLED dark.
2. A top-level `runtimeError` takes the full frame over splash, menu, and footer toast, and hides
   transport and event indicators. Exception: the native MIDI input-list failure keeps the concise
   `MIDI INPUTS` / `MIDI unavailable` menu frame and toast.
3. Splash.
4. Confirmation dialogs.
5. The setup portal modal, then the Backup / Restore card.
6. System info and help.
7. The menu with its footer. Context toasts never change LED overlay priority.

### Runtime errors

- The error frame has seven fixed body rows in its own rectangle (`x=10`, `y=34`, 18 columns,
  12 px rows): `DOMAIN ` plus 11 identifier cells, `CODE ` plus 13, `OP ` plus 15, then `MSG ` plus
  up to 14 message cells and three continuation rows with a four-space prefix plus up to 14 cells.
  The compact `Back/Press: close` footer sits below them.
- Identifier underscores become spaces, controls and whitespace collapse, unsupported glyphs show as
  `?`, and missing identifiers show `unknown`. Messages keep supported path punctuation and
  underscores, use `needs attention` when missing, wrap words and hard-break long words across the
  four message rows, and end the last content row with `...` when truncated. Unused continuation
  rows stay empty. PlaybackRuntime owns this layout; Pi consumes the rows.
- `PlaybackRuntime::latched_errors` is the sole owner of presented runtime-error visibility. While
  its top error is visible, ordinary physical `DeviceInput` messages are rewritten internally and
  never forwarded to platform adapters; `requestSnapshot: false` is ignored on this path.
- Awake `DeviceInput::ButtonA { pressed: Some(true) }` (Back) or
  `DeviceInput::EncoderPress { id: Some("main") }` dismisses only the exact captured top error, then
  emits the underlying menu/modal/OLED snapshot. Dismissal does not restart transport or emit any
  action, effect, event, or command. Every other input (releases, turns, aux, grid, `Other`) is
  consumed without changing state. Wake still wins: the input that wakes the OLED is consumed as wake
  input and does not dismiss.
- During the error gate, Shift/Fn/combined-modifier presses and releases update only physical held
  state and execute no mapped controls. Dismissal does not clear a modifier that is still held, and
  a release received during the gate reconciles held state before the UI is restored.

### Status icons and transport indicators

- The transport icon (`▶` / `⏸` / `■`) sits bottom-right and hides while a footer toast is shown.
- Event-dot and transport-flash timing uses native monotonic deadlines: 45 ms for the event dot,
  90 ms for beat/measure flashes. A visible start, flash-kind change, or expiry forces one snapshot;
  same-state retriggers only extend the deadline.
- Recent voice stealing turns the event dot magenta. Aggregate audio-load and voice-steal status
  stay on the desktop audio-load bridge and do not drive the worker icon.
- The persistent-worker CPU icon is magenta at `(117,5)` and shows only while valid persistent
  worker metrics report `high_cpu_steady`; missing or inline metrics hide it. The configured
  threshold (`System > Audio > Engine > CPU Warn %`) applies immediately. Clearing needs
  utilization strictly below `threshold - 5` percentage points for five rendered seconds; equality
  stays in the hold band and resets the clear timer.
- A missed-quantum flash takes the same CPU slot with the inverted white/black CPU glyph and hides
  the magenta icon while active. The yellow save icon at `(107,5)` may coexist with either CPU-slot
  state.
- `DeadlineMiss` is native, nonterminal ownership recovery. A newly missed persistent quantum
  replays the previous final master quantum exactly once when its frame count matches, otherwise it
  emits silence; later refills during recovery are silent. `missedQuantumFlash` stays set for five
  emitted seconds (`sampleRate * 5`), clears on the exact emitted-frame crossing, and resets on a
  later miss. Structural worker failures (`DispatchFailed`, `CompletionFailed`, `WorkerExited`,
  `InvalidBlock`) stay sticky terminal states and skip this recovery path.
- None of these status changes alter OLED layout or coordinates.

### Toasts

- Toasts render on the single bottom line with a 17-column visible window. Longer messages scroll
  horizontally by one native offset per display snapshot attempt; the host supplies the 33 ms
  attempt cadence while scrolling. Short messages schedule no scrolling.
- Selected long menu rows use the same attempt pacing, advancing one character every four attempts
  with a three-space cycle gap. Short selected rows schedule no scrolling.
- Toast offsets reset when a toast is replaced. There is no `startedAtMs` wall-clock contract.
- Long Aux turn toasts scroll once to their rightmost position and hold there without wrapping, so
  the changing value stays visible (for example `...r Cutoff: 56`). A held toast stops scheduling
  scroll attempts.
- Context changes show toasts such as `Layer: L3 rain` or `Play: fx`. Help and confirm modals keep
  display priority over them.
- Holding Shift, Fn, or Shift+Fn for more than one second without another mapped action shows a hint
  toast: `Shift: map/edit`, `Fn: nav/alt`, or `Help: Sh+Fn+Enter`. Startup shows
  `Help: Sh+Fn+Enter` too. Existing toasts, help/confirm dialogs, assignment overlays, and consumed
  mappings suppress the hint.

### NeoKey snapshot contract

- The native snapshot owns NeoKey presentation. Back is magenta. Space is magenta when stopped, blue
  when paused, dim green while playing at rest, yellow on beat flashes, and green on measure
  flashes. Shift and Fn are blue while combined, yellow while held, and dim gray otherwise.
- `neoKeyLeds.back`, `.space`, `.shift`, and `.fn` are required native RGB values before brightness
  scaling. Desktop and Pi apply the same basis-point rule: normal scale is
  `buttonBrightness * 100`; dimmed scale is zero for zero brightness, otherwise
  `max(buttonBrightness * 8, 400)`; each channel is `(channel * scale + 5000) / 10000`.
- `eventDotOn`, `transportIcon`, and `transportFlash` are required top-level snapshot fields.
  `transportFlash` is not duplicated under `settings`.

## Grid

### Coordinate boundary

- The grid is a fixed 8×8. Native behavior/world coordinates use a lower-left origin: `(0,0)` is
  bottom-left and `y` increases upward.
- Display and hardware frames use top-left row-major order at the boundary. `platform-core` owns the
  pure logical row-major and logical/display projection helpers; `playback-runtime` delegates its
  display-index API to them.
- Desktop `GRID_DOMAIN` is a deliberate boundary mirror for input and rendering, checked against
  every cell in `resources/grid-projection-v1.json`. The fixture is verification data, not a runtime
  projection table.
- HAL NeoTrellis wiring stays fixed adapter data: four device quadrants, physical keys, device
  addresses, and GRB output are kept apart from the shared world/display projection.

### Cell colors

Each cell's LED color comes from its behavior palette and `CellTriggerType`. Every behavior provides
inactive, active, and stable colors; the defaults are inactive black, active yellow, and stable
green. Inactive black is preferred unless a behavior needs another off color.

| Condition | Color |
|---|---|
| Cell off | Behavior inactive color |
| `activate` | Behavior active color |
| `stable` | Behavior stable color |
| `deactivate` | `GRAY` |
| `scanned` | `BLUE` (only in scan mode `scanning`) |

Grid Bright scales the result after the palette is applied. Snapshots also expose logical
active-cell booleans so simulator paint controls never infer cell state from RGB.

### Overlays and priority

The grid is composed in this order; each step paints over the previous one.

1. Behavior cell colors.
2. Scan progress: on the active layer in scan mode `scanning`, the current scan line or lane
   segment gets a dim white lift.
3. The active Play page overlay (see [play.md](menu-tree/play.md)). `Map to Grid` switches to the
   `fx` page, so FX assignment uses the FX page overlay. The `drums` page blacks out the grid; the
   other pages dim behavior cells to a quarter and draw their markers on top.
4. Unless FX assignment is active: sample assignment, otherwise Drum assignment, otherwise
   probability assignment. Each blacks out the grid first.
   - Sample assignment: cells of the selected sample slot light magenta (High), yellow (Medium),
     green (Low), or gray (Base); cells of other sample slots light dim gray.
   - Drum assignment: the selected voice's cells are bright white, other assigned cells dim white,
     empty cells dark.
   - Probability map: black `0%`, magenta `low`, yellow `high`, green `100%`.
5. Param-mod lanes, only while Shift is held without Fn, no Play page or assignment is active, and
   the highlighted row is a bindable parameter. Rows `y=0..1` and columns `x=0..1` show dim gray
   lanes; X slot 1/2 paint row 0/1 and Y slot 1/2 paint column 0/1, green (magenta when inverted),
   bright for the highlighted target and dim otherwise. Cells `(0,0)` and `(1,1)` are white.
6. Fn navigation, while Fn or Shift+Fn is held and no assignment is active. All other cells go
   black. The left column is the layer column: without an active Play page the active layer is
   blue, configured layers green, and `none` layers dim gray; with an active Play page every
   configured layer is blue. The right column lists the Play pages by row, dim yellow, with the
   active page green; row 7 is dim gray. Shift+Fn shows only the layer column.

World coordinates keep the bottom-left origin and the normal LED display conversion in every
overlay.

Ghost Cells (`System > Display`): when on, inactive layers' active cells are meant to render very
dim green behind the active layer, below active-layer cells and sample assignment. The runtime
currently stores and exports this setting but does not render ghost cells.

### Sectioned scanning

- `Sections=1` scans one full column per step for `columns` and one full row per step for `rows`.
- `Sections=2`, `4`, or `8` split the grid into that many lanes across the scan direction and scan
  each lane in turn:
  - `columns`: lanes are `8 / Sections` rows tall, starting at the bottom. The column-segment ray
    moves left to right across lane 1, then lane 2, and so on. Total steps: `gridWidth * sections`.
  - `rows`: lanes are `8 / Sections` columns wide, starting at the left. The row-segment ray moves
    bottom to top through lane 1, then lane 2. Total steps: `gridHeight * sections`.
  - `reverse` runs the same step sequence backwards.
- Stop and emergency reset return the scan index to origin.
- `Restart Section` on Pitch Steps is meant to make pitch stepping local to the lane of the matching
  scan orientation (X for column sections, Y for row sections). The runtime currently stores and
  edits the switch but does not apply it to pitch mapping.

## Aux Encoder Binding

- Each aux encoder has a turn slot (value parameters: number/enum/bool) and a press slot (actions),
  plus a separate shifted custom bank of the same shape.
- Plain turn/press uses the normal bank with auto-map fallback; Shift turn/press uses only the
  shifted bank. Auto-map never fills shifted slots. Shifted bindings persist as
  `runtimeConfig.shiftAuxBindings`, mirroring `runtimeConfig.auxBindings`.
- Fn + aux press binds or overwrites the matching custom slot: while editing a value it binds the
  Turn slot; on an action row it binds the `!` press slot. Shift + Fn + aux press writes the shifted
  bank instead.
- A regular press triggers the press slot action; a regular turn adjusts the turn slot value. Enum
  turns clamp without wrapping; bool turns go `-1 => Off`, `+1 => On`.
- `activeBehavior` and `behaviorConfig.*` updates re-initialize behavior state.
- Aux value changes become autosave-eligible after the native 150 ms edit debounce. Pi autosaves
  coalesce the latest eligible revision for two seconds, whether transport is playing or stopped; a
  newer eligible revision restarts that deadline.
- The menu lives under [`Link > Aux Mappings`](menu-tree/link.md#aux-mappings).

### Auto-map

- `System > Display > Auto Map` turns it on. Context-sensitive mappings fill unbound normal slots
  for the focused menu context; custom bindings keep precedence.
- Focused rows show indicators such as `1-Cutoff` and `1!Assign`, keeping selection markers
  (`> 1!Assign`).
- Behavior rows map Step Rate to A1 and a behavior-specific set to A2 onward (for example Life:
  Count, then Interval with a Spawn press, then Glider with a Glider press). Behaviors without
  their own layout map only Step Rate.
- Instrument layouts: FM rows map Index, Cutoff, Res, Gain; Plucked rows map Decay, Bright, Pick,
  Gain; Drum Voice/Edit rows map Tune, Decay, Tone, Attack of the selected voice; every type's
  Filter maps Cutoff, Res, Env, Key; Volume maps Gain, Vel; Synth oscillators and the Synth, FM,
  and Plucked envelopes have their own layouts. Sampler Filter maps only the audible Cutoff and
  Res, and Sampler Amp Env/Filter Env have no mappings because those saved controls are not
  rendered. Other Sampler rows map Slot (with an Assign press), Base, Tune, Levels. Mixer maps
  Vol, Pan, Route.
- Layouts list up to four entries, but there are three aux encoders (`auxEncoderCount: 3`), so a
  fourth entry is never reachable from hardware.
- FM Index, Plucked Decay, and the selected Drum voice Tone emit typed controls without
  reinitializing the whole slot.
- No auto-map applies inside Link. In Play, Play FX params and `Map to Grid` use the Play FX layout
  (also listed in [`Play > FX > Aux Map`](menu-tree/play.md)).

### Aux toasts

- Compact labels: `Trn-1`, `Clk-1`, `S+Trn-1`, `S+Clk-1`.
- Unbound: `Trn-1: No binding`, `S+Clk-1: No binding`.
- Turn: current value, for example `Trn-1: Spawn Count: 3`.
- Repeated turns of the same target keep the visible toast: only the value changes in place and the
  1.2 s timeout restarts with each turn. A different target, or a turn after the toast expired or
  was replaced, starts a fresh toast. Fresh turn toasts for different targets are throttled to one
  per 500 ms, showing the latest queued one when the throttle ends.

### Inactive (stale) bindings

- Bindings are not removed when their target context changes. If a bound target is inactive, the
  input is ignored, a scoped `not active` toast is shown, and the binding stays for later.
- Turn examples:
  - FX param missing for the current slot type: `Trn-1: B1 Time ms not active`
  - Instrument type changed away from the bound subtree: `Trn-1: I1 Filter cutoff not active`
  - Layer scan field while `scanMode` is not `scanning`: `Trn-1: L1 Scan Direction not active`
  - Behavior config param missing from the current behavior's `configMenu()`:
    `Trn-1: L1 Spawn Count not active`
- Press bindings store the behavior action key. A press fires only if the active layer's behavior
  exposes the same key, otherwise `Clk-1: L1 Spawn Random not active`. A Sample Assign press whose
  instrument is no longer a Sampler is also `not active`.
- Scope prefixes are 1-indexed: `B<N>` bus, `I<N>` instrument, `L<N>` layer. Behavior config uses
  the active layer scope.

## Modulation

- Behavior ticks and held XY input update persistent source contributions without cloning or
  reapplying unchanged held targets. Global Link LFOs advance at 24 PPQN only.
- The native process combines persistent, held tick/XY, and LFO contributions, clamps once, and
  applies one resolved value per target endpoint. An active LFO step visits only dirty LFO target
  keys plus other contributors sharing them.
- Ordinary menu/Aux base edits rebase and recompose only the edited key/endpoint and keep held
  sources, so clearing a source restores the new base.
- Changed persistent grid/XY targets from one step share one revision and one delayed autosave
  payload; no immediate full payload or behavior-state serialization is emitted.
- Config/patch transactions reset candidate modulation state, install all persistent owners, and
  resample active XY afterwards so its captured base is the loaded value.
- Enabled targeted LFO phases advance and wrap even at depth zero. Other PPQN paths and wall-clock
  timing never advance LFOs or process unrelated held sources.
- Layer X/Y, Play XY, and LFO assignments are staged through the native claim validator. An
  exclusive target may have one layer/Play claim, and an LFO may never claim one. A rejected
  assignment leaves binding, contribution, revision, autosave state, and menu focus unchanged and
  shows `Mapping rejected: unsupported target`, `Mapping rejected: target type mismatch`,
  `Mapping rejected: LFO needs a live numeric target`, or
  `Mapping rejected: target already claimed`. Accepted assignments and removals refresh the visible
  `Current:` label.
- Link axis lanes ([link.md](menu-tree/link.md)):
  - Pitch is additive across axes (`X Steps + Y Steps`), signed `-16..16`, and uses scale-degree
    stepping, not post-quantize.
  - `Velocity` modulates outgoing `note_on` velocity. `Filter Cutoff` emits CC74 (lowpass cutoff)
    and `Filter Res` emits CC71 (lowpass resonance).
  - Each value lane applies its curve after `Grid Offs`: `linear` maps position `t` directly,
    `curve` uses the quadratic ease-in `t²`. Both clamp `t` to `0..1`, hit `From` and `To` exactly,
    and stay inside the inclusive range between them, including when `From > To`.
  - `Grid Offs` rotates axis indexing (offset 5 treats cell 5 as first, then wraps). Its bounds are
    `-(GRID_SIZE-1)..+(GRID_SIZE-1)`, so `-7..7`.

## Save and Load

- Ordinary System preferences, including Auto Save and Backups, are saved when a changed editor is
  exited with Main or Back. Unchanged or reverted edits do not save, turning does not write on every
  step, and System edits never dirty the musical patch. If a System write is pending when another
  edit completes, only the latest completed snapshot is kept for the follow-up; overlapping edits do
  not each guarantee a physical write.
- `System > Saves > System > Save System` is an optional explicit flush; `Load System` loads local
  device settings without stopping music or silently rebooting. Restart-sensitive settings save
  through the confirmed Apply flow ([system.md](menu-tree/system.md#restart-sensitive-settings)),
  which writes only System settings and keeps unsaved music.
- Reboot and Shutdown refuse with `System save pending, try again` while a System write or follow-up
  is pending. An allowed Shutdown keeps Patch-only recovery.
- `Auto Save` and `Backups` under `System > Saves > Default` govern the default patch in
  `default.patch.json`. Auto Save is off by default. When on, eligible musical edits emit deferred
  saves; fast audio-facing edits update state and audio immediately and coalesce payload generation
  for about 150 ms, so storage writes the latest settled value. Changing the Auto Save preference
  itself saves to System after a changed editor exit and does not write the Patch.
- Patch autosave, rolling backups, and recovery saves are patch-only. Backups are on by default: the
  first eligible dirty edit may trigger one immediately, later backups are at least five minutes
  apart, and hosts keep the latest 20 rolling patch backups.
- `Save Patch` writes the default patch. Confirmed `Load Patch` and named-preset loads stop/reset
  playback and panic/clear held notes while keeping local System settings and ports.
- A load refuses with `Save pending, try again` before confirmation when a patch write is pending or
  an Auto Save-eligible patch edit is dirty; the same check after confirmation catches a newly
  pending write. With Auto Save off and no pending write, the confirmation may discard unsaved patch
  edits on purpose.
- Patches, named presets, `.oct` musical exports, the local default patch, rolling backups, and
  recovery all keep any safe relative WAV sample path (no absolute paths, `..`, or backslashes),
  even when the media is missing. On another device the patch loads cleanly where the same files
  exist; elsewhere the affected slots show `N/A-<filename>` and can be remapped. The bundled library
  is the plain `samples/` folder; there is no sample manifest.
- Named presets are portable patch files. Loading one applies musical patch state only and keeps
  local settings such as brightness, MIDI ports/sync, USB, HDMI, recording settings, audio buffer,
  Auto Save/Backups, and sample favourites. Device/System aux bindings stay local; musical aux
  bindings travel with the patch.
- Patch-persistent state and transient state per section are listed in
  [build.md](menu-tree/build.md#layer-runtime-behavior) and [play.md](menu-tree/play.md#persistence).

## Config Persistence (ConfigPayload)

- Native `ConfigPayload` is produced and consumed by `crates/playback-runtime/src/native_runner.rs`.
- It stores the active behavior, per-layer behavior/config/state, Link settings, mapping,
  instruments, mixer, FX, Play settings, MIDI settings, UI settings, and persistence flags.
- Restore accepts current payloads and supported older saved shapes, sanitizes external
  compatibility data, then applies only native-owned runtime/core fields.
- Behavior state is restored when saved and compatible; a behavior change initializes the new
  behavior's state through the native behavior engine.
- Transport timing accumulators reset on restore so a loaded config starts from a deterministic
  position.

## Brightness

- OLED Bright is applied by PlaybackRuntime while producing native OLED frame bytes.
- Grid Bright scales matrix LED RGB intensity; Button Bright scales NeoKey LED intensity.
- The Dim Timer applies an extra dim, with its visible floor, while the OLED stays on.
- Once `OLED Sleep` turns `display.off` on, Pi replaces semantic grid and NeoKey output with a 10 Hz
  sparse dim-star animation of independently pulsing stars. It ignores `ledsDimmed`, stays within
  the sleep dim scale, and keeps a zero-brightness blackout dark.

## Native Behavior Contract

Native behaviors implement the Rust `BehaviorEngine` trait in `crates/platform-core/src/behavior.rs`
and are registered from `crates/platform-core/src/behaviors/`. Each behavior provides:

- a stable behavior id
- initial state from config
- input and tick transitions
- a render model for the grid
- serialization/deserialization of saved state
- optional config menu items
- optional immediate input-transition interpretation
- an optional grid interaction mode such as paint or momentary

Input events:

- `DeviceInput` carries `grid_press` and `grid_release`. Behaviors that ignore `grid_release` do
  nothing with it. `keys` uses press→activate and release→deactivate; `looper` uses the same live
  semantics and can overdub step-quantized press/release events into its loop.
- With immediate input-transition interpretation, `platform-core` runs grid changes from input
  through the same Link/mapping pipeline as a tick and produces immediate musical events. `keys` and
  `looper` use this for finger-drumming response.

### Trigger types

All behaviors use `CellTriggerType`: `activate`, `stable`, `deactivate`, `scanned`, or `none`.

| Type | Source | When |
|---|---|---|
| `activate` | Behavior | Cell becomes active (birth, shape hits cell, etc.) |
| `stable` | Behavior | Cell stays active (alive, inside shape interior, etc.) |
| `deactivate` | Behavior | Cell becomes inactive (death, shape leaves cell, etc.) |
| `scanned` | Scanning layer | Cell found active during a scan (scan mode `scanning` only) |

Scan mode `none` produces no `scanned` triggers. `State Notes` controls only non-scan state-note
events; `scanned` triggers stay active while scanning.
