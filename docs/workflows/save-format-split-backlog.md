# Next release: separate System settings and musical patches

**Status:** on `main` and running on both development boards. No published release carries it yet, and stores from 0.7.x still need the supervised conversion described below.

Before the source cutover, `default.json` mixed a musical patch with settings for the particular device. Native `playback-runtime` projects a portable `octessera.patch` and device configuration. The file split preserves their existing ownership, including exceptions to the menu headings.

## Ownership decisions

| Control or data | Owner for this split | Constraint |
| --- | --- | --- |
| Build worlds, saved grids and Looper events; Link mappings, scale, probability, tempo and swing; Shape instruments, mixer and FX; persistent Play FX and XY | Musical patch | Local Default, backup and recovery patches retain safe relative local/SD sample references, even when media is unavailable. Portable named presets and `.oct` exports require canonical shipped-library sample IDs. Do not save held notes, transport phase or live gestures. |
| System output route, global MIDI ports and sync, USB roles, audio buffer/optimization, master output volume, display/HDMI, sample-browser favourites, Auto Save/Backups policy and recording limits | System settings | Keep device paths and ports local. A MIDI instrument's channel and note settings still belong to the patch. |
| System → Notes: note length and velocity scale/curve | Musical patch | Keep the existing portable behavior and document the System-menu exception. |
| System → Audio: polyphony/voice-stealing policy | Musical patch | Keep today's patch ownership; moving it to System would be a separate musical behavior change. |
| Link → Paused Events | System | Keep today's device-local ownership despite its Link-menu location. |
| Link → Aux Mappings, including Shift | Split by target/action, as today | Musical parameter/action bindings travel with the patch; hardware/System bindings stay local. Reject two documents claiming the same bank, control and Turn/Click side rather than choosing by load order. |
| Shape sample-browser favourites | System | Bookmarks are local; assigned sample IDs are musical. This is a menu-location exception unless the bookmark control moves. |
| Active layer, behavior and Play-page selection | Musical patch | Preserve their current saved initial focus, not live transport phase or held gestures. |

Save/Load, Panic, reboot, recording and setup **actions** are not themselves saved fields. Menu placement must not silently turn a portable library preset into a device configuration. `Notes`, `voiceStealingMode`, and initial layer/behavior/Play-page focus remain Patch-owned despite their System menu placement; Paused Events remains System-owned despite its Link placement. Aux ownership remains split by target/action and mixed aux storage remains per-side.

## Two files, one native owner

The files in the existing store root are `system.json` for System settings and `default.patch.json` for the default musical patch. Keep named library presets musical-only. `crates/playback-runtime` owns classification, validation, save/load intent and native menu behavior; the desktop and Pi adapters own paths and atomic writes. Do not add a TypeScript runtime path or a generic synchronization service.

- **Save/Load System** changes only System settings. A changed ordinary System preference edit saves once when exited through Main or Back; unchanged or reverted edits do not save. There are no per-turn writes, and these edits do not dirty or immediately save the Patch. If a write is pending, only the latest completed snapshot is retained for a follow-up rather than promising a physical write for each overlapping edit. Restart-sensitive values are persisted only after confirmed Apply; applying a setting must not discard unsaved music. The explicit Save System and Load System actions remain available. Reboot and Shutdown refuse with `System save pending, try again` while a System write or follow-up remains pending, preserving Patch-only recovery on allowed shutdown.
- **Save/Load Patch** changes only music. Named preset Load/Save keeps the same musical boundary. Confirmed Patch/Named Load stops/resets and panics music while retaining local System settings and ports. A known pending patch write or Auto Save-eligible dirty edit refuses load before confirmation with `Save pending, try again`; the check runs again after confirmation. With Auto Save off and no pending write, confirmation may discard dirty patch edits.
- Patch autosave is coalesced at about 150ms. Rolling backups and recovery are patch-only. A changed Auto Save preference saves to System on editor exit; when enabled, later eligible Patch edits use Patch autosave. Changing the Auto Save/Backups preference does not dirty or immediately save the Patch.
- Each file uses the existing atomic write. They are independent operations, not a new two-file transaction protocol.

Keep `config/defaults/base.json` and its platform overrides as the authored defaults. Pi's active settings paths are `/home/pi/presets/system.json` (Raspberry) and `/var/lib/octessera/presets/system.json` (Orange); early USB/audio boot helpers use the active System file. Orange's staged `usr/share/octessera/defaults/pi-default.json` is immutable image-build metadata, not an active default. Desktop seeds the split files at first app launch. Do not duplicate the ownership rules in the generator.

## Migration and acceptance

Legacy-only, partial, or corrupt split stores require supervised conversion or repair. This note is not instructions to launch the preview against existing real AppData or to flash or migrate a board.

Source acceptance covers native two-file round trips and isolation in both directions; mixed Aux assignments; complete patch replacement; missing/corrupt input and stale queued saves; first-run seeding, device Apply, recovery and backups; and the same menu semantics on Raspberry, Orange and desktop. Native menu/help text synchronization is owned with the runtime change. Physical deployment or migration is not implied by this source contract.
