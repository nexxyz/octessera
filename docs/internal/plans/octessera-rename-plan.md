# Octessera Rename Plan

Active plan. Execute only on the rename branch created from the rollback tag.

Current condition: Phase 1 user-facing identity has started, while internal
packages, crates, services, install paths, repository paths, and hardware source
filenames are still named and wired as Cell Symphony. The rename starts from the
clean rollback tag `pre-octessera-rename`, then proceeds in three large phases.
Naming mismatches should fail hard. Do not add aliases, fallback imports,
duplicate package names, old binary wrappers, old service aliases, or path
fallbacks for project-owned identifiers.

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

## Naming Decisions

- Product, device, and app name: `Octessera`.
- Repository name: likely `octessera`.
- User-facing binaries: likely `octessera-pi`, `octessera-desktop`, and
  `Octessera.exe`.
- Pi systemd services: likely `octessera.service`,
  `octessera-boot-splash.service`, `octessera-oled-shutdown.service`, and
  `octessera-performance-governor.service`.
- Rename internal crates/packages in phase 2, not as an optional later cleanup.
- Existing Cell Symphony saves/configs are not preserved through fallback search
  paths. If data preservation is required later, implement it as an explicit,
  one-time migration with tests, not as a runtime alias.

Recommended approach: rename user-facing identity first, then internal package
and crate identity, then deployment/repository/artifact surfaces.

## Current Cell Symphony Inventory

Major current identifiers include:

- root package name: `cellsymphony`;
- pnpm packages: `@cellsymphony/desktop`,
  `@cellsymphony/device-contracts`;
- Rust crates/binaries: `cellsymphony-desktop`, `cellsymphony-pi`,
  `cellsymphony-hal`;
- desktop package/source names and historical metadata; Phase 1 changes visible
  product metadata, app titles, and portable executable output to Octessera;
- Pi install paths and services: `/opt/cellsymphony`,
  `/usr/local/bin/cellsymphony-pi`, `cellsymphony.service`, and related boot,
  shutdown, and performance services;
- Pi image stage path: `tools/pi-image/stage4-cellsymphony`;
- PCB source and fabrication names: `hardware/pcb/cellSymphony.*` and
  `release-artifacts/pcb/gerber/cellSymphony-*`;
- docs, scripts, tests, runtime status text, OLED/splash text, and sample/help
  references.

The rename is broader than product copy. It touches packaging, deployment,
service installation, generated artifacts, build scripts, tests, and hardware
docs.

## Execution Phases

0. Preflight:
   - ensure `pre-octessera-rename` points to the approved rollback commit;
   - work on a rename branch;
   - capture an old-name grep inventory.
1. User-facing identity:
   - product strings, desktop title/product metadata, OLED/splash/status/help
     text, user docs, and visible desktop artifact names;
   - change the Tauri identifier to `xyz.nex.octessera` in this phase. This
     intentionally changes desktop OS app identity/app-data resolution with no
     fallback to old Cell Symphony app data;
   - do not rename pnpm packages, Rust crates, services, or install paths yet.
2. Internal-facing identity:
   - pnpm package names/imports, Cargo crate and binary names, package filters,
     runtime labels, tests, and lockfiles;
   - run `corepack pnpm install` immediately after package name changes.
3. Repository, deployment, and artifact cleanup:
   - Pi service names, install paths, deploy/build scripts, Pi image stage paths,
     Cross targets, hardware docs, CAD/PCB source names, and regenerated release
     artifacts.

## User-Facing Text And Assets

Rename product-facing strings:

- boot splash and OLED text;
- desktop window title and app metadata;
- menu/help/error text where the product name appears;
- release artifact names;
- README, screenshots, and docs images;
- logos/icons/splash images if new Octessera artwork exists.

Do not keep compatibility wording in product UI. Historical docs may say
“formerly Cell Symphony” only when useful to explain the rename.

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

Service changes must install and use only the Octessera unit names. Do not add
service aliases or old binary wrappers. If an old service is handled during a
deployment upgrade, it should be explicitly stopped/disabled/removed rather than
kept as a compatibility path.

## Files, Directories, And Config Paths

Audit and rename:

- desktop config/store paths;
- Pi store/config paths;
- deploy scripts and release packaging;
- Pi image builder paths under `tools/pi-image/stage4-cellsymphony`;
- `/opt/cellsymphony` installation layout;
- logs and journal identifiers;
- portable executable path: `apps/desktop/dist-desktop/CellSymphony.exe` → `Octessera.exe`.

Do not add startup fallback search paths for old stores/config directories. If
old user data must be carried forward, add a separately reviewed one-time
migration step with tests.

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

Keep a short historical note only where useful: “Octessera was formerly Cell
Symphony.” Do not claim automatic save/config migration unless a one-time
migration is explicitly implemented and tested.

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
- no fallback imports, package aliases, service aliases, old binary wrappers, or
  project-owned old path fallbacks remain;
- Pi deployment uses Octessera service and install paths without double-starting
  an old Cell Symphony service;
- release artifacts use Octessera names after source regeneration;
- package lockfiles and Cargo lockfile changes are intentional.
