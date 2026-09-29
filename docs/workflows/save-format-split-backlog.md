# Next release: separate System settings and musical patches

**Target:** the release after the current preset work, tentatively 0.8.8. This is a plan, not a change to the current save format.

Today, `default.json` mixes a musical patch with settings for the particular device. Native `playback-runtime` already projects a portable `octessera.patch` and a device configuration, but that split does not exactly follow the menu headings. Agree on the exceptions below before changing persistence.

## Ownership decisions

| Control or data | Proposed owner | Decision to settle |
| --- | --- | --- |
| Build worlds, saved grids and Looper events; Link mappings, scale, probability, tempo and swing; Shape instruments, mixer and FX; persistent Play FX and XY | Musical patch | Keep relative `samples/...` assignments and validate them against the shipped library. Do not save held notes, transport phase or live gestures. |
| System output route, global MIDI ports and sync, USB roles, audio buffer/optimization, master output volume, display/HDMI, sample-browser favourites, autosave policy and recording limits | System settings | Keep device paths and ports local. A MIDI instrument's channel and note settings still belong to the patch. |
| System → Notes: note length and velocity scale/curve | **Musical patch recommended** | They change the music and are portable today. Move their menu controls to a musical section if the heading rule must be literal; otherwise document the System-menu exception. |
| System → Audio: polyphony/voice-stealing policy | **System recommended** | It also changes note allocation. Decide whether the patch should specify it before moving the field. |
| Link → Paused Events | **Musical patch recommended** | It changes this patch's response to input. Alternatively move the control to System and keep its present device ownership. |
| Link → Aux Mappings, including Shift | Split by target/action, as today | Musical parameter/action bindings travel with the patch; hardware/System bindings stay local. Decide how a conflicting assignment to the same control is shown and resolved. |
| Shape sample-browser favourites | System | Bookmarks are local; assigned sample IDs are musical. This is a menu-location exception unless the bookmark control moves. |
| Active layer, behavior and Play-page selection | Open | Distinguish a useful initial patch focus from a transient UI cursor. Neither is live transport state. |

Save/Load, Panic, reboot, recording and setup **actions** are not themselves saved fields. Menu placement must not silently turn a portable library preset into a device configuration. Resolve the open choices and update the menu/control spec before implementing them.

## Two files, one native owner

Proposed files in the existing store root: `system.json` for System settings and `default.patch.json` for the default musical patch. Keep named library presets musical-only. `crates/playback-runtime` owns classification, validation, save/load intent and native menu behavior; the desktop and Pi adapters own paths and atomic writes. Do not add a TypeScript runtime path or a generic synchronization service.

- **Save/Load System** changes only System settings. Loading restart-sensitive hardware settings follows the existing Apply/confirmation path; reading a file must not silently reboot the device.
- **Save/Load Default Patch** changes only music. Named preset Load/Save keeps the same musical boundary. Loading a patch uses the existing held-note/transport cleanup and never changes ports or brightness.
- Keep patch autosave and rolling patch backups initially. Start with explicit System Save; decide separately whether System needs autosave or backup history. A System save must not acknowledge unsaved music, and a queued save must not overwrite a newly loaded document.
- Each file uses the existing atomic write. They are independent operations, not a new two-file transaction protocol.

Keep `config/defaults/base.json` and its platform overrides as the authored defaults for the first version. Stage the Pi `system.json` from the native-owned projection **before** provisioning and early USB/audio boot helpers run; they cannot wait for the music runtime to seed it. Those helpers must read the staged System file instead of mixed `default.json`. Desktop can seed both files at first app launch. Do not duplicate the ownership rules in the generator.

## Migration and acceptance

There are no external user saves to support. Convert only our own current desktop, Raspberry and Orange defaults under supervision: stop the app/service, retain an exact mixed-file backup, normalize through the native owner, write both new files, read them back, and prove recomposition preserves the approved music and device settings. Install both before starting the new runtime; retire the mixed file from active use. If conversion fails, restore the old file and old runtime. Do not maintain an indefinite fallback chain.

Before release, test native two-file round trips and isolation in both directions; mixed Aux assignments; complete patch replacement; missing/corrupt input and stale queued saves; first-run seeding, device Apply, recovery and backups; and the same menu/help semantics on Raspberry, Orange and desktop. Update `docs/menu-and-controls-spec.md`, the split menu trees if placement changes, `resources/menu-help-texts.tsv`, `docs/runtime-boundaries.md`, and user-facing backup instructions. Rebuild the desktop app and perform focused checks on both physical boards.
