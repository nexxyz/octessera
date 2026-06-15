# Backlog — Cell Symphony

> Central requirement backlog. Status: `open` | `in-progress`.
> Each phase must be completed and manually tested before moving to the next.

## Native Runtime Migration Regression Plan

Status: `in-progress`

The native Rust runtime/menu migration must be checked against documentation, auxiliary resources, and the old TypeScript implementation before it is considered parity-complete. Canonical references:

- `docs/menu-and-controls-spec.md`
- `docs/runtime-boundaries.md`
- `packages/platform-core/resources/menu-help-texts.tsv`
- `packages/platform-core/src/menuTree.ts`
- `packages/platform-core/src/menuNodes.ts`
- `packages/platform-core/src/menuView.ts`
- `packages/platform-core/src/menuInput.ts`
- `packages/platform-core/src/inputGrid.ts`
- `packages/platform-core/src/inputInternal.ts`
- `packages/platform-core/src/runtimeHelpers.ts`
- `packages/platform-core/src/actions.ts`
- `packages/platform-core/src/storeResultHandlers.ts`
- `packages/platform-core/src/synthPresets.ts`
- `packages/platform-core/tests/*`
- `crates/playback-runtime/src/native_runner.rs`
- `crates/playback-runtime/src/native_menu.rs`
- `apps/desktop/src-tauri/src/audio_config.rs`
- `apps/desktop/src-tauri/src/host_adapter.rs`
- `apps/desktop/src/ui/App.tsx`

No basic-functionality fallbacks are allowed during this migration. Missing menu rows, bad native wiring, incorrect capability data, incorrect desktop bridge mappings, and runtime/core mistakes must fail visibly and be fixed at the source. Fallbacks are only acceptable for external compatibility and availability cases, such as older saved configs with missing or renamed fields, disconnected MIDI devices, unavailable sample files, or missing saved resources; these should trigger user-visible toast/status feedback where practical.

### Reported Bugs

- FIXED: The trigger indicator behaved erratically; native snapshots now hold event-dot state briefly and reset it deterministically on stop.
- FIXED: The play indicator now uses old OLED semantics: red on full-note/measure boundaries and green on non-full quarter boundaries.
- FIXED: The Sense/Scan menu now includes Empty Action and Empty Instrument for scanned-empty mappings.
- FIXED: Instrument selections in Sense now include a `none` entry and route that target to no emitted note/audio event.
- FIXED: Scanning now changes the native interpretation tick strategy and renders a scan progress row/column overlay.
- FIXED: Synth preset loading now applies the full old preset synth payload instead of only changing gain.
- FIXED: The synth filter section now exposes the resonance parameter.
- FIXED: Long breadcrumb paths are clipped to a single safe line and the desktop OLED fallback prevents title wrapping overlap.
- FIXED: Long sample/file menu labels are clipped in native snapshots and the desktop OLED fallback prevents body-row wrapping overlap.
- FIXED: FX bus labels now show assigned bus names such as `delay+reverb` or `(none)` instead of always displaying a generic label.
- FIXED: Sample slot labels are one-based in the UI while preserving zero-based persisted/runtime indexes.
- FIXED: FX Bus and Global FX menus now use separate effect option lists matching mono bus vs stereo global FX intent.
- FIXED: Scanning with sequencer routed to sampler now uses native scan interpretation and Sense target mapping so scanned events can reach sampler channels.
- FIXED: Dance overlays now dim and layer over existing behavior cells instead of erasing ghost-cell context.
- FIXED: Back exits sample assignment mode and restores the previous grid view path.
- FIXED: The scan indicator now uses a dim white additive overlay and preserves set/live cell colors instead of replacing them with bright blue.
- FIXED: Switching the edited grid part no longer resets the current same-behavior engine state, so the previously playing part state is preserved.
- FIXED: Scan Direction now changes the effective scan order for scan interpretation and scan progress overlay.
- FIXED: Sample previews emitted from file selection now reach the desktop audio queue through platform-effect audio commands.
- FIXED: Scan Sections now affect both scan interpretation and scan progress overlay lane width/height.
- FIXED: Scanning sequencer patterns now advance through scanned rows/notes instead of repeatedly triggering only the first sample on full-note boundaries.
- FIXED: Multiple configured native parts now advance together during transport ticks instead of only the selected/edit part playing.
- FIXED: The OLED body is constrained above a reserved bottom status row for toaster/status indicators.
- FIXED: Native menu/config edits now emit deferred default-save effects when Auto Save is enabled.
- FIXED: The transport play/pause/stop glyph is now the red/green quarter/measure indicator, while the separate trigger dot flashes white for emitted triggers; flash state is held deterministically across snapshots.
- FIXED: Regular menus now keep the full six-row OLED body while still reserving the bottom status/toast row; file rows remain single-line clipped.
- FIXED: Scanning sequencer patterns now honor Sense `scanUnit`, so quarter/eighth scanning advances before full-note step-rate boundaries.
- FIXED: Native sequencer grid state is serialized and rehydrated for active and inactive parts, so patterns survive startup/default-load.
- FIXED: Native save/default-save results and deferred autosave effects now light the `S` indicator and show a scrolling toast/status message.
- FIXED: Native toaster/status messages are exposed in snapshots and support horizontal scrolling for long messages.
- FIXED: Mapped samplers now trigger from sequencers by remapping sampler-assigned native note output to sample-slot notes before desktop audio dispatch.
- FIXED: The desktop save indicator now expires after a short flash window instead of staying visible permanently.
- FIXED: Bound aux encoder turns now produce native toaster/status messages.
- FIXED: Desktop OLED fallback spacing has slightly more row gap and selected-value padding.
- FIXED: Sequencer cells save/load through autosave/startup for active native sequencer grid state.
- FIXED: Native menu snapshots expose parameter bar metadata, and the desktop OLED fallback renders bars when the display mode allows them.
- FIXED: Panning parameters display as `L15` through `C` through `R15`.
- FIXED: Regular submenus no longer append transport status lines such as `Stopped 120 BPM`; transport status stays in the dedicated transport fields/indicators.
- FIXED: Native scale menus expose the legacy scale IDs, including major and minor pentatonic, and display friendly scale labels while preserving persisted IDs.
- FIXED: Native menus now use seven body rows above the reserved status/toast area.
- FIXED: The active-layer temporary trigger-off feature is available through Fn+Play and now shows trigger on/off toasts.
- FIXED: Part behavior config is stored per part, so switching or saving multiple same-behavior parts does not reuse/null another part's behavior config.
- FIXED: Repeated autosaves increment a flash serial so the desktop save indicator restarts for later saves even when the flash flag remains active.
- FIXED: Native numeric bar metadata now follows the legacy bar/marker classification for volume, pan, FX, synth/sample, system, L2 axis, and behavior parameters.
- FIXED: Instrument mixer pan/volume menu rows are initialized from current runtime state, so pan edits no longer bounce between stale default values and center.
- FIXED: Native contextual help resolves specific entries from `packages/platform-core/resources/menu-help-texts.tsv`; generic fallback help was removed and menu-help lint now fails on kind fallback coverage.
- FIXED: Saved/default payload apply now restores the active part `L1 > Step Rate`, so autosaved `1/16` survives restart/load.
- FIXED: Pitch note parameters display with the legacy note-name format such as `C4 (60)` while storing MIDI numbers.
- FIXED: Back/menu navigation exits live Dance overlays cleanly, and Fn/menu navigation no longer traps the user on a Dance page.
- FIXED: Dance momentary FX now use the legacy select-then-map flow: `FX Page` is flat, `Map to Grid` stores selected config per cell, and grid press/release triggers the stored cell config.
- FIXED: Wrapped note mapping now wraps within the concrete in-range scale-note list, so wrapped notes stay on the selected scale even when the starting note is off-scale.
- FIXED: Fn part navigation now works while a Dance overlay is active, uses display-space part rows, and exits the live Dance overlay after selecting a part.
- FIXED: Fn part navigation now uses grid Y directly instead of an inverted Y axis.
- FIXED: The original "disable triggers for a part" shortcut is `Fn+Play` (`Fn+Space` on desktop); the Play button label remains just `Space` plus play/pause symbols.
- FIXED: `Fn+Play` muting now writes the active part's real L2 trigger probability mode to `zero` and restores it, instead of only changing the Dance trigger-gate overlay state.
- FIXED: Major pentatonic and other scales now use distinct lowest/starting/highest note semantics and apply configured X/Y pitch steps into both interpretation axes and wrapped scale-note mapping.
- FIXED: `L4: Dance` now shows only the selected Dance page controls, flattened directly under the Dance menu.
- FIXED: Native behavior parameter help keys now use canonical `parts.*.l1.behaviorConfig.*` paths, so Life behavior params resolve TSV help text.
- FIXED: Help popup scroll clamping is covered for behavior-param help and existing contextual help scroll tests.

### Old-TypeScript Parity Audit Deltas

- FIXED: Native trigger probability modes and maps now filter interpreted intents before mapping like old `filterTriggerGatedIntents()` did for both input transitions and transport ticks.
- FIXED: Fresh native Sense defaults now mirror old `createInitialState()` behavior without overwriting the user's current default save preset: scan axis `columns`, scan unit `1/16`, pitch `36..74` with start `60`, scale `major_pentatonic`, root `D`, `clamp`, X pitch step `0`, Y pitch step `1`, probability maps defaulting to `full`, and inactive parts with event triggers disabled.
- FIXED: Explicit factory reset is separate from fresh startup and no longer loads or overwrites the user's default save preset; it applies a native in-memory factory payload with old factory part/instrument intent.
- FIXED: Per-part `L1 > Step Rate` is native for immediate-mode execution and save/load; menu edits update the active part and payloads round-trip `parts[*].l1.stepRate`.
- FIXED: Native input-transition interpretation now respects `eventEnabled`; paused input remains enabled by default to match old `createInitialState().inputEventsWhilePaused = true`.
- FIXED: Native mapping/modulation now covers old velocity lanes, filter cutoff/resonance CC lanes, parameter-mod slots, and Dance XY target bindings for native menu-owned runtime config keys.
- FIXED: Native Dance X/Y now persists and applies `xyTouch`, `xyRelease`, target bindings, invert flags, and sample-hold/reset-center release behavior for supported native runtime config keys.
- FIXED: Native Dance FX now supports active-FX state, max-concurrent limiting, same-type replacement, same-config assignment toggle/clear behavior, legacy `momentary-fx:x:y` IDs, and active/limited LED state.
- FIXED: Native sample assignment now supports old velocity-level assignment cycling (`high` -> `medium` -> `low` -> off), configured level values, payload round-trip, and dirty marking for autosave.
- FIXED: Native autosave dirty coverage now includes menu payload diffs, sample assignment, Dance mix/pan/XY edits, and runtime modulation writes to native-owned config fields.
- FIXED: Native config payload/load shape now includes and rehydrates legacy nested `runtimeConfig.sound`, selected UI fields, per-part step rates, sample velocity-level fields, `xyTouch`, `xyRelease`, `paramMods`, `parts[*].xy` target/invert fields, sample nested defaults, and FX slot param objects for native-owned config.
- FIXED: Native persisted-config migration/sanitization now covers old pan-position scaling, legacy `l1.triggerGates` to probability-map migration, Dance FX param/type/target sanitization, sample slot/assignment normalization, aux turn-key validation, and XY binding validation.
- FIXED: Dance help TSV/lint targets now use the flattened selected-page native Dance paths and native Dance FX/XY keys instead of old `FX Page`/`Trigger Gate`/`X/Y Pad` grouped paths.
- FIXED: Assignment-mode input priority now matches old TS for grid assignment modes: FX/sample/probability assignment wins over Fn navigation.
- FIXED: Momentary FX IDs now use the old `momentary-fx:x:y` format; host/audio treats the value as an opaque start/stop key.
- FIXED: Native sampler assignment now matches old `applyNoteBehavior()` for assigned and unmapped sampler notes: assigned `note_off` events remap to the assigned sample-slot note, and unmapped sampler `note_on`/`note_off` events are suppressed instead of reaching sample slot 0 by default.
- FIXED: Native transport now applies runtime modulation for inactive playing parts as old `advanceEngineByPulses()` does inside the per-part loop, not only for the active part.
- FIXED: Native parameter-target rows now use a shared picker for paramMods, Dance XY, and Aux turn bindings instead of placeholder target rows.
- FIXED: Native `L2 > Aux Mappings` now exposes turn target picker semantics and click action picker semantics for behavior actions, sample assignment actions, Dance FX mapping, reset, and unbind.
- FIXED: Native Shift+grid param-mod assignment now matches old `applyParamModMapping()` slot targeting and mapped -> inverted -> cleared cycling, with the old lane overlay colors.
- FIXED: Native now persists and exposes `inputEventsWhilePaused` instead of only hardcoding the old default behavior.
- FIXED: Native config payloads now honor old `l1.saveGridState` / `l1.savedState` semantics rather than always writing `saveGridState: false` with native `behaviorState` fields.
- FIXED: Native now preserves persisted manual names and `autoName` side effects for parts, instruments, and buses, and exposes native text-entry rows for editing those names.
- FIXED: Native sample/MIDI voice payload compatibility now includes sample `baseVelocity`, legacy `midiEngine` shape, and stateful sample `ampEnv`/`filter`/`filterEnv` round-trip instead of placeholder objects.
- FIXED: Native System Sound/MIDI menu fields `sound.voiceStealingMode`, `midi.clockOutEnabled`, `midi.clockInEnabled`, and `midi.respondToStartStop` now persist/apply through the runner instead of being display-only/default values.
- FIXED: Native behavior changes now remap behavior-config paramMods and aux turn/click bindings through old analogue groups and clear behavior actions when the target behavior has no primary action.
- FIXED: Native help coverage now walks representative native menu configurations and requires every exposed target to resolve to a specific TSV row; the TSV lint also passes without fallback-style or ambiguous rows.
- FIXED: Native L2 value-lane `Curve` now persists, loads, applies through menu edits, and round-trips in config payloads.
- FIXED: Native scanning/detail rows, pitch/value-lane detail rows, and sampler velocity-level rows now follow the old conditional visibility rules.
- FIXED: Native synth and sampler cutoff menus now display/edit the old compact `0..255` scale while preserving runtime Hz storage, and synth oscillator detune is restored to the old `-50..50` menu range.
- FIXED: Native FX parameter menu display now shows human units for frequency, time, dB, percent, normalized values, and reverb decay instead of raw internal scaled values.

### Open Native Parity Deltas From Deep Audit

- OPEN [critical]: Pi Zero still uses `NodeRunnerProcess` / TS core runner instead of the Rust `NativeRunner`; migrate Pi to the same native runtime boundary as desktop.
- OPEN [critical]: Pi host adapter ignores platform effects, audio commands, and MIDI output; wire store/default/preset effects, sample browser/preview, Dance FX/audio commands, and MIDI-out.
- OPEN [critical]: Pi loop does not render runtime snapshots to OLED/LEDs and only sends NeoKey press events, not releases; wire OLED, NeoTrellis/NeoKey LEDs, and button release semantics.
- FIXED: Desktop runtime no longer imports or constructs `@cellsymphony/platform-core-runner`; non-Tauri/test use must inject a runner or native dispatch explicitly.
- FIXED: Native preset Library `Save As` uses text draft name and emits store-save, Rename supports pick/New Name/Apply with save-then-delete cleanup, and empty lists show `(none)` refresh rows.
- FIXED: Native toast/status flow now covers preset save/load/delete/rename, Save Current with no loaded preset, default save/load, factory load, synth preset load, and MIDI panic; destructive/load/save/panic actions now use native modal confirmations.
- FIXED: Native Voice synth menu exposes oscillator octave/level/detune/pulse-width, amp/filter envelopes, env amount, key tracking, velocity sensitivity controls, and applies them to synth config payloads.
- FIXED: Native synth oscillator Wave, filter Type, and filter Cutoff rows now initialize from current synth state and apply back to synth config.
- FIXED: Native Sampler menu exposes Velocity Levels, high/medium/low levels, Base Velocity, sample filter/filter envelope, amp velocity sensitivity, and amp envelope controls, and applies them to sample config payloads.
- FIXED: Native Instrument `Note Behavior` is initialized from instrument state and propagated into `NativePartEngine.note_behaviors` for loaded and edited configs.
- FIXED: Native MIDI instrument Channel now has `midi.channel` state/payload, loads legacy `midiEngine.channel`, edits through the menu, and remaps emitted MIDI note/CC channels.
- FIXED: System Sound edits now sync runner global sound into active and inactive `NativePartEngine` configs immediately.
- FIXED: FX bus/global FX params are stateful in native; native preserves custom params from presets/defaults, exposes parameter menu rows, resets params on type changes, and serializes edited params.
- FIXED: Instrument Clone and Reset actions are exposed in native Voice, update native instrument slots, and round-trip through aux action serialization.
- FIXED: Native runtime snapshot/audio config shape now includes the editable synth/sample/mixer/FX fields needed by desktop, and desktop audio conversion preserves FX slot params.
- FIXED: Native `screenSleepSeconds` now tracks last interaction, exposes splash/off display state, and wakes on device input.
- FIXED: Fresh in-memory native defaults now match old `createInitialState()` for `masterVolume` (`73`), `autoSaveDefault` (`false`), and default note length (`120ms`).
- FIXED: Native text field OLED formatting no longer appends `@cursor`; cursor state remains internal while editing.
- FIXED: Native L1 part name row label now matches the legacy/spec `Part Name` label.
- FIXED: `docs/native-test-parity.md` partial rows for `ant`, `bounce`, `shapes`, and `behavior-api` are resolved by existing native deterministic coverage plus explicit legacy-only classification for random placement and TS registration side-effect cases.
- FIXED: Stale Life behavior help/help-popup backlog BUG markers were rechecked with targeted native regressions.
- VERIFY [high]: Run Pi/Linux target build/clippy and hardware smoke; current verification was desktop/Windows-scoped and does not prove Pi runtime parity.

Regression coverage added in this pass:

- `scan_progress_overlay_is_dim_white_and_preserves_live_cell_color`
- `switching_active_part_preserves_current_part_engine_state`
- `reverse_scan_direction_starts_from_last_lane`
- `scan_sections_limit_overlay_to_current_section_lane`
- `scan_interpretation_advances_with_engine_ticks`
- `reverse_scan_row_starts_from_last_row`
- `sectioned_scan_row_limits_output_to_current_section`
- `platform_effect_audio_command_reaches_audio_queue`
- `sense_scan_menu_exposes_none_and_scanned_empty_targets`
- `synth_preset_load_changes_full_synth_payload_and_filter_resonance_is_editable`
- `sample_slot_menu_is_one_based_but_payload_remains_zero_based_and_back_exits_assign`
- `scanning_sequencer_pattern_emits_different_rows_over_scan_steps`
- `transport_tick_advances_multiple_configured_parts`
- `native_menu_edit_emits_deferred_auto_save_when_enabled`
- `native_snapshot_reserves_bottom_oled_row_for_status`
- `regular_menu_snapshot_keeps_seven_body_rows_above_reserved_status`
- `scan_unit_advances_scanning_before_full_note_step_rate`
- `sequencer_grid_state_is_serialized_and_rehydrated_for_all_parts`
- `save_default_result_lights_auto_save_indicator_and_toast_scrolls`
- `sequencer_sampler_assignment_remaps_notes_to_sample_slots`
- `deferred_autosave_restores_active_sequencer_grid_on_startup`
- `bound_aux_turn_shows_status_toast`
- `native_menu_snapshot_includes_bar_values_and_pan_formatting`
- `save_flash_visible_expires_after_duration`
- `submenu_snapshot_does_not_append_transport_status_line`
- `scale_menu_uses_legacy_scale_ids_and_display_labels`
- `mixer_menu_uses_current_volume_and_pan_values`
- `instrument_pan_menu_edit_moves_monotonically_from_current_value`
- `active_part_trigger_toggle_suppresses_and_restores_with_toast`
- `repeated_autosaves_increment_flash_serial`
- `native_menu_help_targets_resolve_to_specific_tsv_rows`
- `contextual_help_includes_midi_output_guidance`
- `saved_step_rate_rehydrates_from_default_payload`
- `pitch_note_params_use_legacy_note_name_display`
- `back_exits_active_dance_overlay_and_menu_context`
- `dance_fx_page_is_flat_and_shows_selected_type_params`
- `dance_fx_map_to_grid_stores_config_and_payload_round_trips`
- `wrapped_notes_stay_on_scale_when_starting_note_is_off_scale`
- `negative_wrapped_notes_stay_on_selected_scale`
- `fn_left_column_selects_parts_while_in_dance_overlay_and_exits_overlay`
- `sense_pitch_mapping_uses_lowest_starting_highest_and_both_axis_steps`
- `l4_spec_rows_show_only_selected_dance_page_controls`
- `fresh_native_runner_uses_old_initial_sense_defaults`
- `trigger_probability_zero_suppresses_input_transition_events`
- `trigger_probability_custom_zero_cell_suppresses_transport_events`
- `assignment_mode_wins_over_fn_part_navigation_and_autosaves`
- `dance_mix_grid_edit_autosaves_persistent_volume_change`
- `per_part_step_rates_round_trip_and_drive_immediate_parts`
- `event_enabled_false_suppresses_input_transition_events`
- `sample_assignment_cycles_velocity_levels_when_enabled`
- `sample_assignment_velocity_level_uses_configured_values`
- `sampler_assignment_remaps_note_off_and_suppresses_unmapped_notes`
- `config_payload_includes_complete_sample_and_fx_param_shapes`
- `fx_slot_groups_show_selected_effect_params`
- `fx_params_edit_into_config_payload`
- `snapshot_settings_include_complete_audio_config_shapes`
- `synth_payload_includes_master_fx_slots`
- `legacy_nested_sound_and_ui_fields_rehydrate_from_payload`
- `input_events_while_paused_false_suppresses_paused_grid_events`
- `external_midi_realtime_respects_clock_in_and_start_stop_settings`
- `dance_fx_same_config_assignment_toggles_cell_clear`
- `dance_fx_press_replaces_same_type_and_limits_concurrency`
- `dance_fx_overlay_marks_active_and_limited_cells`
- `legacy_trigger_gates_migrate_to_probability_map`
- `legacy_eight_position_pan_payload_scales_to_native_pan_range`
- `dance_xy_touch_persists_and_release_behavior_matches_config`
- `factory_load_applies_native_factory_without_loading_user_default`
- `sense_velocity_and_filter_lanes_modulate_mapped_events`
- `sense_value_lanes_round_trip_in_payload`
- `param_mod_binding_updates_native_runtime_config`
- `shift_grid_param_mod_mapping_cycles_slots`
- `shift_grid_param_mod_overlay_marks_lanes_and_combined_cells`
- `dance_xy_binding_updates_native_runtime_config`
- `menu_binding_actions_update_param_xy_and_aux_targets`
- `behavior_change_remaps_behavior_param_mods_and_aux_bindings`
- `inactive_part_transport_tick_applies_param_modulation`
- `part_and_bus_names_round_trip_with_auto_name_flags`
- `native_text_row_edits_part_name_and_clears_auto_name`
- `sample_slots_and_assignments_are_sanitized_on_load`
- `dance_fx_payload_sanitizes_type_target_and_params`
- `invalid_aux_and_xy_bindings_are_dropped_on_load`
- `config_payload_includes_complete_sample_and_fx_param_shapes`
- `save_grid_state_controls_saved_state_payload_and_restore`
- `conditional_rows_follow_scan_lane_and_sampler_state`

Latest Rust verification after this pass:

- `cargo fmt --all --check`: passed
- `cargo test -p platform-core`: passed, 54 tests
- `cargo test -p playback-runtime`: passed, 176 tests
- `cargo test -p cellsymphony-desktop`: passed, 10 tests
- `cargo clippy -p platform-core -p playback-runtime -p cellsymphony-desktop`: passed
- `corepack pnpm --filter @cellsymphony/platform-core lint:menu-help`: passed

### Execution Order

1. Done: Fix transport/trigger indicators and OLED clipping because they affect every manual test pass.
2. Done: Fix L2 Sense scan menu, scan execution/progress, `none` targets, scanned-empty mapping, and sampler scan routing.
3. Done: Fix synth preset payload application and missing synth filter controls.
4. Done: Fix sample browser/assignment UI regressions, including one-based sample labels and assignment exit behavior.
5. Done: Fix FX bus/global FX option separation and bus labels.
6. Done: Fix Dance ghost-cell layering and overlay priority.
7. Done: Cross-check native menu against the complete layout and wire every exposed option or remove it until it has native backing.
8. Done: Update docs/help/resources and run full regression checks.

## Native Test Parity Plan

Status: `in-progress`

Goal: every legacy TypeScript test that still represents live native runtime behavior must have a Rust/native counterpart. TypeScript packages remain references, but native correctness must be enforced in `crates/platform-core`, `crates/playback-runtime`, and `apps/desktop/src-tauri`.

Tracking document: `docs/native-test-parity.md`.

Rules:

- Port behavior and runtime semantics as Rust tests; do not rely on legacy TS tests to validate the shipped native path.
- Mark TS tests as `covered`, `partial`, `missing`, or `legacy-only` in the matrix before deleting or ignoring parity work.
- If a TS test covers compatibility with older configs, keep/port the compatibility behavior and require a visible status/toast where practical.
- If a TS test covered a TS-only runtime path that is no longer shipped, mark it `legacy-only` with a short reason.

Execution order:

1. Behavior package tests: `none`, `life`, `sequencer`, `keys`, `brain`, `ant`, `bounce`, `shapes`, `raindrops`, `dla`, `glider`.
2. Interpretation and mapping tests: scan column/row/section variants, whole-grid active, scanned-empty, validation/sanitization, clamp/wrap, empty scale errors.
3. Runtime/config payload tests: behavior state save/load, `saveGridState`, legacy route normalization, trigger gate migration, factory reset, old/missing fields.
4. Aux/input transition tests: grid press/release events, paused input behavior, aux stale binding toasts, unbind confirmation, mapping overlay delays.
5. Dance/sample/probability tests: assignment velocity levels, shift row and Fn+Shift column, probability map alignment, sampler row assignment under transport.
6. OLED/help/menu tests: seven body rows plus status row, marker bars, audio load indicators, generated help coverage, complete menu wiring.
7. Desktop bridge tests: sample preview, store result toasts, MIDI disconnected status, audio config mapping, OLED fallback assumptions.

Current first batch:

- Done: Create the coverage matrix.
- Done: Port missing deterministic behavior package tests into `crates/platform-core`.
- Done: Port missing interpretation/mapping tests into `crates/platform-core`.

First batch verification:

- `cargo test -p platform-core`: passed, 49 tests.

Next parity batches:

- Done: Port runtime/config payload tests for behavior state, old route normalization, old/missing config fields, and save/default flows.
- Done: Port aux/input transition tests for grid press/release events, non-interpreting behavior suppression, grid edit autosave, aux binding, and unbound aux toasts.
- Done: Port sample assignment, menu action wiring, OLED/help, and desktop bridge parity tests.
- Classified TS-only `xy-pad.test.ts` helper tests as `legacy-only` because those helpers are not part of the shipped native runtime path; native Dance XY grid interaction remains covered in runner tests.

Parity batch verification:

- `cargo test -p platform-core`: passed, 52 tests.
- `cargo test -p playback-runtime`: passed, 118 tests.
- `cargo test -p cellsymphony-desktop`: passed, 10 tests.
- `cargo clippy -p platform-core -p playback-runtime -p cellsymphony-desktop`: passed.
- `corepack pnpm run lint`: passed.
- `corepack pnpm run typecheck`: passed.
- `corepack pnpm run format:check`: passed.
- `corepack pnpm -r test`: passed.
- `corepack pnpm --filter @cellsymphony/desktop tauri:build`: passed.

### Coordinate And LED Foundation

- Compare native LED indexing against old `GRID_DOMAIN.toDisplayIndex` and `GRID_DOMAIN.indexOf` behavior.
- Implement one shared native display-index helper and use it for all LED writes and overlay rendering.
- Apply it to FN left-column part navigation, FN right-column Dance page navigation, Dance Mix, Dance Pan, Dance FX, Trigger Gate, XY, ghost cells, sample assignment overlay, and trigger-probability assignment overlay.
- Ensure FN right-column page rows match the spec: row 0 = `mix`, row 1 = `pan`, row 2 = `fx`, row 3 = `trigger-gate`, row 4 = `xy`.
- Ensure FN left-column part rows match old display/world orientation.
- Ensure non-navigation cells dim to 25% brightness while FN navigation is shown.

Regression tests:

- Native FN overlay highlights the active part at the same display cell as old TypeScript.
- Native FN overlay shows configured part indicators in the expected left-column cells.
- Native FN overlay shows Dance page indicators in the expected right-column cells, including `xy`.
- Pressing each FN Dance page cell selects the matching page.
- Trigger-gate row edits affect the expected part.
- Dance Mix press at a known cell updates the expected instrument volume.
- Dance Pan press at a known cell updates the expected instrument or bus pan.
- XY press at a known cell produces the expected normalized coordinate.

### Dance BPM Setting

- Check old `menuTree.ts`, `menu-and-controls-spec.md`, and runtime state for canonical BPM field and menu placement.
- Add a BPM control under native `L4: Dance` if old behavior exposed it there.
- Ensure the control writes the canonical transport/runtime BPM field, not a Dance-only shadow value.
- Ensure transport tick timing/status uses the edited BPM.

Regression tests:

- `L4: Dance > BPM` is visible.
- Editing BPM updates native transport/runtime status.
- BPM persists in config payload if old TS persisted it.
- Transport pulse behavior uses the edited BPM value.

### Dance Mix, Pan, Trigger Gate, XY, And Ghost Cells

- Match old `runtimeHelpers.danceModeToLeds` behavior.
- Dance Mix must show all instrument columns up to capability count and dim `none` or inactive instruments instead of omitting them.
- Dance Pan must show two-cell pan markers and route-colored bus markers for bus-routed instruments.
- Trigger Gate must use old row/column layout: columns 0..2 set per-row modes, columns 5..7 row 0 set all-parts modes, gap columns stay dark.
- XY must map full 8x8 grid to normalized 0..1 coordinates with no margins.
- Ghost cells must follow old priority: sample assignment overlay > Dance overlay > active part cells > inactive-part ghost cells > off/cursor background.
- When Dance is active, behavior cells should be ghosted/underlaid according to the spec, not lost or drawn full-bright in the wrong layer.

Regression tests:

- Dance Mix LEDs include all slots and dim inactive/none slots.
- Dance Pan direct and bus-routed markers appear at expected cells and colors.
- Trigger Gate selected mode cells are bright and non-selected mode cells are dim.
- All-parts Trigger Gate buttons affect all parts.
- Ghost cells render inactive part active cells as dim green.
- Dance overlay priority does not erase required ghost-cell context.

### Transport And OLED Polish

- Match spec transport icons: play, pause, stopped.
- Stop state must show stopped icon, not pause icon.
- Long breadcrumb/title paths must never wrap over body rows. Use old abbreviation rules, clipping, or scrolling.
- Preserve old selected row formatting and menu body row height.

Regression tests:

- Native stopped snapshot exposes stopped icon.
- Desktop OLED fallback renders stopped icon.
- Deep breadcrumb path remains single-line-safe and does not overlap first body row.
- Long sample filenames and nested paths do not overlap menu rows.

### Synth Preset Loading

- Port or share old `SYNTH_PRESETS` data for native use.
- Add `L3 > Voice > Instruments > I# > Synth > Preset > Load > <preset>`.
- Implement load action matching old `synth_preset_load` semantics.
- Apply the selected preset to the chosen instrument slot's synth payload.
- Ensure desktop audio config receives the changed synth config.

Regression tests:

- Synth preset menu exposes known preset names.
- Loading a preset changes the selected instrument synth payload.
- Loading a preset does not affect other instrument slots.
- Desktop audio config receives the selected preset values.

### Sample Browser Parity

- Match old `menuNodes.sampleBrowserNodes` and `actions.ts` behavior.
- Browser rows must be `..`, `[folder]`, file rows, and `(empty)` when no entries exist.
- `Choose Sample` opens/refreshes the browser and emits `sample_list_request` for the current directory.
- Folder rows enter subdirectories.
- `..` goes to parent directory.
- File rows pick the sample.
- Preview should use the old preview input behavior instead of a divergent menu row, unless old behavior explicitly had a row.
- Long file/folder names must be clipped or scrolled without OLED overlap.

Regression tests:

- Opening `Choose Sample` emits `SampleListRequest`.
- Sample list result creates `..`, `[folder]`, file rows, and `(empty)` correctly.
- Folder enter and parent up emit correct directories.
- File pick sets `instruments.N.sample.slots.M.path`.
- Preview emits `SamplePreview` through the selected instrument slot.
- Long names do not wrap/overlap.

### Sample Assignment Mode

- Add native `Assign` action under sampler menu.
- Add transient native sample assignment state: `{ instrumentSlot, sampleSlot }`.
- Implement grid single-cell sample assignment.
- Implement row assignment with Shift + cell.
- Implement column assignment with Fn + Shift + cell.
- Preserve one assignment per cell: new assignment replaces old assignment.
- Support velocity-level cycling when velocity levels are enabled: high -> medium -> low -> off.
- Render sample assignment overlay using old `sampleAssignmentToLeds` colors and display orientation.
- Include `sample.assignments`, `velocityLevelsEnabled`, `velocityLevels`, and `baseVelocity` in payloads.

Regression tests:

- Entering assign mode sets transient native state.
- Single-cell assignment persists in payload.
- Assigning another sample to the same cell replaces the old assignment.
- Velocity-level cycling matches old behavior.
- Row and column assignment shortcuts work.
- LED overlay matches selected slot, other-slot dim, and unassigned dark behavior.

### Instrument Routing And Correct Event Targets

- Add `instruments.N.mixer.route` state and menu: `direct`, `fx_bus_1`, `fx_bus_2`, `fx_bus_3`, `fx_bus_4`.
- Persist/load route in native config payloads.
- Feed route through desktop `AudioInstrumentMixerConfig` into realtime engine config.
- Update Dance Pan to resolve direct vs bus-routed pan target the old way.
- Audit native event generation from behavior cells through L2 Sense mappings to instrument slot output.
- Build per-part mapping from native `sense_parts`, not a single global/default mapping.
- Ensure activate/stable/deactivate/scanned mappings target the selected instrument slot and action.
- Ensure Synth and Sampler map to internal audio slot events while MIDI maps to external MIDI output.

Regression tests:

- Route menu exposes direct and all FX buses.
- Route edits persist in payload and desktop audio config.
- Dance Pan writes bus pan for bus-routed instruments and instrument pan for direct instruments.
- Life part 1 mapped to `I1: synth` emits an event for instrument 1.
- Mapping the same part to `I2: sampler` emits an event for sampler slot 2.
- Mapping to MIDI emits MIDI output and not internal audio.
- `none` mappings emit no note/audio event.

### Sense Instrument Selector Labels

- Named target selectors must display computed labels through the old `formatDisplayValue` behavior.
- L2 Sense instrument slot pickers must show `I1: synth`, `I2: sampler`, etc., not raw numeric values.
- Persist the correct slot ID while displaying the computed instrument designation.

Regression tests:

- Sense mapping selectors display instrument labels.
- Selecting `I2: sampler` persists slot `1` or equivalent canonical ID.
- Loading a payload restores the correct selected display label.

### FX Option Coverage

- Audit `realtime-engine` for every supported bus/global FX kind.
- Audit old `momentaryFx.ts` for Dance momentary FX kinds: `none`, `stutter`, `freeze`, `filter_sweep`, `pitch_shift`.
- Expose all implemented bus/global FX kinds in native FX bus/global FX menus.
- Ensure desktop audio config accepts and forwards all selected kinds.

Regression tests:

- Every implemented engine FX kind appears in the native menu options where applicable.
- Selecting each kind persists in config payload.
- Desktop audio config accepts each kind without dropping or renaming it.

### Documentation And Auxiliary File Sync

- Update `docs/menu-and-controls-spec.md` if any current spec is stale.
- Update `docs/runtime-boundaries.md` if runtime/audio routing behavior changes.
- Keep `packages/platform-core/resources/menu-help-texts.tsv` in sync if menu/help targets change.
- Keep native contextual help aligned with new native menu rows.
- Re-run menu-help lint after any help/menu changes.

### Regression Audit Checklist

- Map old TS tests to native Rust equivalents:
- `logic-core.test.ts`: transport icons, menu input/edit, MIDI menu requests.
- `logic-ui.test.ts`: OLED display, Dance/Fn, ghost cells.
- `features-runtime.test.ts`: runtime routing, sample assignment, presets/defaults, Dance.
- `features-aux.test.ts`: aux binding, sample assignment, stale mappings.
- `menuHelp.test.ts`: help lookup behavior.
- `xy-pad.test.ts`: XY modulation and Dance grid behavior.
- Add missing native tests in `crates/playback-runtime/src/native_runner/tests.rs`, `crates/playback-runtime/src/native_menu/tests.rs`, `crates/platform-core/src/...`, and `apps/desktop/src-tauri/src/audio_config_tests.rs`.

### Required Final Verification

- `cargo fmt --all --check`
- `cargo test -p platform-core`
- `cargo test -p playback-runtime`
- `cargo test -p cellsymphony-desktop`
- `cargo clippy -p platform-core -p playback-runtime -p cellsymphony-desktop`
- `corepack pnpm run lint`
- `corepack pnpm run typecheck`
- `corepack pnpm run format:check`
- `corepack pnpm -r test`
- `corepack pnpm --filter @cellsymphony/desktop tauri:build`

Note: unscoped `cargo clippy` is expected to fail on Windows if it attempts to compile Pi-only `rppal`. Run full unscoped clippy on Linux/Pi target separately.

### Manual Smoke Checklist

- Stop/play transport icon.
- Deep breadcrumb path rendering.
- FN left/right grid navigation.
- Dance Mix, Pan, Trigger Gate, FX, and XY coordinate behavior.
- Ghost cells in and out of Dance.
- L2 Sense mapping labels.
- Map Life P1 activate to I1 synth and verify audible synth.
- Map Life P1 activate to I2 sampler and verify sampler output.
- Map Life P1 activate to MIDI instrument and verify MIDI output.
- Route I1 to FX bus and verify bus pan/FX.
- Load synth preset.
- Browse sample folders with `..`, long file names, preview, and pick.
- Enter sample assignment mode and assign cells.

### REQ-15 — Signal Path Visualization

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 4 |
| **Priority** | low |
| **Scope** | medium |
| **Depends on** | stable L2 Sense trigger mappings, L3 instrument mixer routing, FX bus routing, Global FX routing |
| **Source** | lines 109–118 |

OLED graphical display of the active configuration and musical signal path: `L1: Life` behavior -> `L2: Sense` interpretation/mapping -> `L3: Voice` instrument/audio routing -> `L4: Dance` live performance overlays -> master output.

**Configuration flow:**
- `L1: Life` defines the active part behavior, step rate, behavior-specific config, and part identity.
- Behavior output is the upstream source for the visualization: the diagram starts from each active part's selected behavior, not from raw audio.
- `L2: Sense` defines how behavior/grid state is interpreted into trigger intents, including scanning, event triggers, trigger probability, note mapping, and X/Y axis modulation.
- `L3: Voice` defines the destination instruments, their type and note behavior, instrument mixer routing, FX buses, and Global FX.
- `L4: Dance` defines live performance controls layered on top of the configured path, including mix, pan, trigger-gate, X/Y modulation, and momentary FX targets inserted into the audio path.
- The visualization must make it clear which parts of the graph come from stored configuration versus live performance overlay state.

**Signal path logic:**
- Parts do not output audio directly. Each active part interprets behavior grid activity through `L2: Sense` into trigger intents.
- Part nodes should identify the active `L1: Life` behavior driving that part.
- Sense mappings route trigger intents into instrument slots by trigger kind:
- `activate`, `stable`, and `deactivate` mappings when `Event Triggers` is enabled.
- `scanned` and `scanned_empty` mappings when `Scan Mode=scanning`.
- `L2: Sense` note mapping and axis modulation affect the musical events sent into the destination instrument slot and should be represented as Sense-stage transformation between part behavior and instrument target.
- Mappings with `Action=none` do not create a visible route.
- Mappings targeting an instrument whose `Type=none` are hidden.
- Synth and sampler instruments are internal audio sources:
- `Route=direct`: instrument post-fader output is panned by instrument `Pan Pos` and summed into the main mix.
- `Route=fx_bus_n`: instrument post-fader output is sent exclusively to FX Bus N.
- FX buses run `Slot 1` then `Slot 2`; slots with `Type=none` are passthrough.
- FX bus output is panned by bus `Pan Pos` and summed into the main mix.
- Global FX runs after direct and bus outputs are summed, in `Slot 1..N` order.
- Global momentary FX is applied after Global FX.
- Master output applies after the full main mix/global FX/momentary FX chain.
- MIDI instruments emit external MIDI/control data and are shown as terminal external-MIDI routes, not as internal audio routes.

**Layout rules:**
- Display is for the 128x128 OLED.
- Show only active routes.
- Use the existing `L1`/`L2`/`L3`/`L4` section color scheme consistently across boxes, highlights, and connector accents.
- `L1: Life` color identifies part/behavior source nodes.
- `L2: Sense` color identifies interpretation, mapping, trigger-probability, note-mapping, and modulation nodes.
- `L3: Voice` color identifies instruments, mixer routing, FX buses, Global FX, and output nodes.
- `L4: Dance` color is white and identifies live overlay nodes such as trigger-gate state, X/Y modulation, mix/pan performance control, and momentary FX targets.
- When a node reflects both stored config and live overlay state, use the owning stage color for the box and the currently active overlay stage color as the highlight/accent.
- Hide parts with no enabled non-`none` Sense mappings.
- Hide instruments with `Type=none`.
- Hide FX buses that have no routed instruments.
- Hide FX slots with `Type=none`, but still show the bus if routed instruments pass through it.
- Always show Global FX only when at least one global slot has `Type != none`; otherwise show the summed main mix flowing directly to output.
- Auto-layout from top to bottom:
- `L1: Life` part/behavior source
- `L2: Sense` interpretation and mapping stage
- Parts
- Instrument slots
- Direct main mix or FX buses
- Global FX, when active
- Momentary FX, when active
- Output
- `L4: Dance` overlay indicators should be shown adjacent to the stage they affect rather than as a separate disconnected graph.
- Use boxes for entities and arrows for signal/event flow.
- When the diagram becomes crowded, abbreviate labels to compact IDs:
- `P1`, `P2`, etc. for parts
- `I1`, `I2`, etc. for instruments
- `B1`, `B2`, etc. for FX buses
- short FX IDs such as `rv`, `dl`, `dk`, `eq`, `cmp`, `sat`, `dst`
- `GFX` for Global FX
- `OUT` for master output
- Prefer hiding inactive entities before abbreviating active entities.
- If the full active graph still cannot fit at readable scale, collapse repeated parallel routes into grouped edges where possible.

**Navigation:**
- The diagram is navigable with the main encoder.
- Encoder turn moves highlight between visible boxes.
- Encoder press enters the highlighted entity's canonical menu:
- Behavior source -> `L1: Life > Pn`
- Part -> `L2: Sense > Pn`
- Sense transform node -> the relevant `L2: Sense > Pn` subgroup
- Instrument -> `L3: Voice > Instruments > Instrument n`
- FX Bus -> `L3: Voice > FX Buses > Bus n`
- FX Bus slot -> that bus slot's FX menu
- Global FX -> `L3: Voice > Global FX`
- Global FX slot -> that global slot's FX menu
- Dance overlay node -> relevant `L4: Dance` submenu or mapped target context
- Output -> relevant master/sound output menu
- Back exits the diagram view to the previous menu context.

**Acceptance:**
- Shows the full configuration flow from `L1: Life` behavior selection through `L2: Sense`, `L3: Voice`, and `L4: Dance` overlays.
- Shows active part-to-instrument event routes from L2 Sense mappings.
- Shows where `L2: Sense` note mapping, trigger probability, scanning/events, and modulation affect the route.
- Shows internal audio routes from synth/sampler instruments through direct output or FX buses.
- Shows FX bus slot order, bus pan/output, Global FX order, momentary FX position, and master output.
- Uses the established `L1`/`L2`/`L3`/`L4` section colors to visually distinguish source, interpretation, voice, and performance-overlay stages.
- MIDI instruments are represented as external MIDI terminal routes, not routed through internal audio FX.
- `none` mappings, `Type=none` instruments, unused buses, and `Type=none` FX slots are hidden.
- Crowded diagrams use compact IDs and grouping while remaining readable.
- Navigation can enter each visible entity's canonical config menu.
- Fits on the 128x128 OLED at readable scale for typical use-cases: 2-4 active parts, 2-4 active instruments, 1-2 active buses, and 0-2 active Global FX slots.

## Phase 5: Advanced / Hardware

*Hardware-specific features and tooling.*

---

### REQ-17 — Hardware Test Harness

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 5 |
| **Priority** | medium |
| **Scope** | medium |
| **Depends on** | — |
| **Source** | line 52 |

Tool launched on Raspberry Pi that guides through testing every button, grid element, encoder, and audio output — to verify hardware assembly per PCB design. *(Placeholder — details to be spe[...]

**Acceptance:**
- Step-by-step guided tests: "Press button A1", "Turn encoder 1 clockwise", etc.
- Grid: "Tap each cell", "Verify colour X at Y,Z".
- Audio: play back a test sample through output.
- Reports pass/fail per test.

---

### REQ-18 — Over-the-Air Updates

| Field | Value |
|-------|-------|
| **Status** | open |
| **Phase** | 5 |
| **Priority** | low |
| **Scope** | small |
| **Depends on** | — |
| **Source** | line 56 |

"Update from GitHub" function on hardware — fast, dynamic update to latest firmware/software. *(Placeholder — details to be specified at Phase 5.)*

**Acceptance:**
- Single action triggers check for updates from GitHub.
- Downloads and applies update automatically.
- Rollback on failure.
- Progress indication on OLED.
