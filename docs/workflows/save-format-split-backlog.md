# System settings and musical patch ownership

**Status:** the split landed on `main` on 2026-10-01 and runs on both
development boards. The latest published release (v0.8.6) predates it, so
stores written by v0.8.6 or earlier still hold the old mixed `default.json`.
Desktop startup refuses that file, and an incomplete split store, and asks for
supervised conversion or repair rather than guessing. Once the next release
ships, fold the table below into
[`menu-and-controls-spec.md`](../menu-and-controls-spec.md) and delete this
page.

The behavior of Save/Load, Auto Save, backups, and pending-save refusals is
specified in [`menu-tree/system.md`](../menu-tree/system.md) and
[`runtime-boundaries.md`](../runtime-boundaries.md#shared-runtime-contract).
This page keeps only the ownership decisions, because some of them do not
follow the menu headings.

## Files

| File | Owner | Raspberry | Orange |
| --- | --- | --- | --- |
| `system.json` | System settings | `/home/pi/presets/system.json` | `/var/lib/octessera/presets/system.json` |
| `default.patch.json` | Default musical patch | `/home/pi/presets/` | `/var/lib/octessera/presets/` |

Desktop seeds both files at first launch. `crates/playback-runtime` owns
classification, validation, and save/load intent; the desktop and Pi adapters
own paths and atomic writes. The two writes are independent, not a two-file
transaction. Shipped defaults still come from `config/defaults/`; Orange's
staged `usr/share/octessera/defaults/pi-default.json` is image-build metadata,
not an active default.

## Who owns what

| Control or data | Owner | Note |
| --- | --- | --- |
| Build worlds, saved grids and Looper events; Link mappings, scale, probability, tempo and swing; Shape instruments, mixer and FX; persistent Play FX and XY | Musical patch | Patches, named presets and `.oct` exports keep safe relative sample references even when media is missing. Held notes, transport phase and live gestures are never saved. |
| System output route, global MIDI ports and sync, USB roles, audio buffer/optimization, master volume, display/HDMI, Auto Save/Backups policy, recording limits | System settings | Device paths and ports stay local. A MIDI instrument's channel and note settings still travel with the patch. |
| System → Notes (note length, velocity scale/curve) | Musical patch | Menu-location exception. |
| System → Audio → Polyphony (`voiceStealingMode`) | Musical patch | Menu-location exception; moving it would change musical behavior. |
| Link → Paused Events | System settings | Menu-location exception. |
| Link → Aux Mappings, including Shift | Split by target | Musical bindings travel with the patch; hardware/System bindings stay local. Two documents claiming the same bank, control and Turn/Click side are rejected, not resolved by load order. |
| Shape sample-browser favourites | System settings | Bookmarks are local; assigned sample IDs are musical. |
| Active layer, behavior and Play-page selection | Musical patch | The saved initial focus, not live transport state. |

Save/Load, Panic, reboot, recording, and setup are actions, not saved fields.
