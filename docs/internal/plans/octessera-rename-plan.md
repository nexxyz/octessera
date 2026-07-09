# Octessera Rename Plan

Planning note only. Do not execute this rename until explicitly approved.

Current condition: the repository is still named and wired as Cell Symphony. The
rename should start from a clean rollback point, then proceed in small phases
that preserve existing saves, config, services, and hardware deployment paths.

## Rollback Marker

Before any rename work:

1. Make the working tree clean, or explicitly decide that uncommitted files are
   excluded from the rollback marker.
2. Create an annotated tag at the approved starting commit, for example
   `pre-octessera-rename`.
3. Record the exact commit and tag in the rename branch notes.

A git tag marks a commit, not uncommitted working-tree files. If generated CAD,
release artifacts, or other local changes must be part of the rollback point,
commit them first or create a separate backup before tagging.

## Naming Decisions To Confirm

- Product, device, and app name: `Octessera`.
- Repository name: likely `octessera`.
- User-facing binaries: likely `octessera-pi`, `octessera-desktop`, and
  `Octessera.exe`.
- Pi systemd services: likely `octessera.service`,
  `octessera-boot-splash.service`, `octessera-oled-shutdown.service`, and
  `octessera-performance-governor.service`.
- Decide whether to rename internal crates/packages now or later. Default:
  later, unless a user-facing artifact cannot be renamed cleanly without it.
- Preserve existing `cellSymphony` saves, config, service upgrades, and store directories through migration or aliases.

Recommended approach: rename user-facing identity first, with compatibility migration. Rename internal crates/packages only in a later pass if the extra churn is worth it.

## Current Cell Symphony Inventory

Major current identifiers include:

- root package name: `cellsymphony`;
- pnpm packages: `@cellsymphony/desktop`,
  `@cellsymphony/device-contracts`;
- Rust crates/binaries: `cellsymphony-desktop`, `cellsymphony-pi`,
  `cellsymphony-hal`;
- desktop executable and metadata: `CellSymphony.exe`, Tauri product name,
  desktop package metadata, and app titles;
- Pi install paths and services: `/opt/cellsymphony`,
  `/usr/local/bin/cellsymphony-pi`, `cellsymphony.service`, and related boot,
  shutdown, and performance services;
- Pi image stage path: `tools/pi-image/stage4-cellsymphony`;
- PCB source and fabrication names: `hardware/pcb/cellSymphony.*` and
  `release-artifacts/pcb/gerber/cellSymphony-*`;
- docs, scripts, tests, runtime status text, OLED/splash text, and sample/help
  references.

The rename is broader than product copy. It touches packaging, deployment,
service migration, generated artifacts, build scripts, tests, and hardware docs.

## Execution Phases

1. Finish or shelve unrelated work, especially generated enclosure and release
   artifacts.
2. Commit and push all current work.
3. Create the rollback tag.
4. Create a rename branch.
5. Add compatibility tests before changing names:
   - old default/config payloads still load;
   - old desktop/Pi store directories migrate or alias safely;
   - existing presets and saves remain valid;
   - service upgrade stops/disables old services and enables new services;
   - deploy scripts still work against a Pi with old Cell Symphony paths.
6. Add path, store, and service migration while old names still exist.
7. Rename user-facing product text and desktop/Pi artifacts.
8. Rename docs, assets, CAD, PCB source names, and regenerated fabrication
   outputs.
9. Optionally rename internal crates/packages in a separate phase.
10. Run full validation, rebuild desktop/Pi artifacts, and smoke-test Pi
    deployment.

## User-Facing Text And Assets

Rename product-facing strings:

- boot splash and OLED text;
- desktop window title and app metadata;
- menu/help/error text where the product name appears;
- release artifact names;
- README, screenshots, and docs images;
- logos/icons/splash images if new Octessera artwork exists.

Keep compatibility wording where needed, for example: “Migrated from Cell Symphony config.”

## Packages, Binaries, And Services

Potential renames:

- `cellsymphony-pi` → `octessera-pi`;
- `cellsymphony-desktop` → `octessera-desktop`;
- `cellsymphony-hal` → `octessera-hal` only if internal crate/package rename is
  included;
- `@cellsymphony/desktop` → `@octessera/desktop`;
- `@cellsymphony/device-contracts` → `@octessera/device-contracts`;
- `cellsymphony.service` → `octessera.service`;
- `cellsymphony-boot-splash.service` → `octessera-boot-splash.service`;
- `cellsymphony-oled-shutdown.service` → `octessera-oled-shutdown.service`;
- `cellsymphony-performance-governor.service` → `octessera-performance-governor.service`;
- `/usr/local/bin/cellsymphony-pi` → `/usr/local/bin/octessera-pi`.

Migration must stop/disable old services and install/enable new services without requiring a manual recovery step. Keep aliases or compatibility handling for at least one release if practical.

## Files, Directories, And Config Paths

Audit and migrate:

- desktop config/store paths;
- Pi store/config paths;
- deploy scripts and release packaging;
- Pi image builder paths under `tools/pi-image/stage4-cellsymphony`;
- `/opt/cellsymphony` installation layout;
- logs and journal identifiers;
- portable executable path: `apps/desktop/dist-desktop/CellSymphony.exe` → `Octessera.exe`.

Startup migration should copy or migrate old stores to the new location when the new store is absent. Leave old stores untouched unless the user explicitly opts into cleanup.

## Hardware, PCB, And Enclosure

Audit and update:

- KiCad schematic title blocks;
- PCB silkscreen text and board labels;
- fabrication output filenames: Gerbers, drill files, pick/place, BOM;
- CAD/enclosure text, embossing, faceplate, and rear labels;
- assembly, wiring, and bring-up documentation.

Regenerate affected fabrication/export artifacts after source CAD/PCB updates.

For enclosure work, keep the CadQuery generator as the source of truth and export
STEP/STL/3MF from it. Do not hand-edit generated mesh artifacts as the source of
the rename.

## Documentation

Rename throughout:

- README;
- hardware build docs;
- menu/control specs;
- runtime boundary docs;
- deployment and Pi setup docs;
- troubleshooting docs;
- release process docs.

Keep a short migration note: “Octessera was formerly Cell Symphony. Existing Cell Symphony saves/configs are migrated automatically.”

## Validation

Run:

- `corepack pnpm run typecheck`;
- `corepack pnpm -r test`;
- `corepack pnpm -r lint`;
- `corepack pnpm -r format:check`;
- `cargo fmt --all --check`;
- relevant Rust tests and clippy;
- Pi cross-build;
- desktop portable executable build;
- Pi deploy smoke test.

Also verify:

- no user-facing `Cell Symphony` strings remain except intentional migration or
  history references;
- old Cell Symphony config/store locations are still readable;
- Pi upgrade from old `cellsymphony.service` does not leave the runtime stopped
  or double-started;
- release artifacts use Octessera names after source regeneration;
- package lockfiles and Cargo lockfile changes are intentional.
