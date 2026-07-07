# Octessera Rename Plan

Planning note only. Do not execute this rename until explicitly approved.

## Naming Decisions To Confirm

- Product, device, and app name: `Octessera`.
- Repository name: likely `octessera`.
- User-facing binaries: `octessera-pi`, `octessera-desktop`, `Octessera.exe`.
- Decide whether to rename internal crates/packages now or later.
- Preserve existing `cellSymphony` saves, config, service upgrades, and store directories through migration or aliases.

Recommended approach: rename user-facing identity first, with compatibility migration. Rename internal crates/packages only in a later pass if the extra churn is worth it.

## Execution Phases

1. Commit and push all current work.
2. Create a rename branch.
3. Add compatibility tests before changing names:
   - old default/config payloads still load;
   - old desktop/Pi store directories migrate or alias safely;
   - existing presets and saves remain valid;
   - service upgrade does not strand the old service.
4. Rename user-facing product text and artifacts.
5. Add runtime/store/service migration.
6. Rename docs, assets, CAD, and PCB markings.
7. Optionally rename internal crates/packages in a separate phase.
8. Run full validation, rebuild desktop/Pi artifacts, and smoke-test Pi deployment.

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
- `@cellsymphony/desktop` → `@octessera/desktop`;
- `@cellsymphony/device-contracts` → `@octessera/device-contracts`;
- `cellsymphony.service` → `octessera.service`;
- `/usr/local/bin/cellsymphony-pi` → `/usr/local/bin/octessera-pi`.

Migration must stop/disable the old service and install/enable the new service without requiring a manual recovery step.

## Files, Directories, And Config Paths

Audit and migrate:

- desktop config/store paths;
- Pi store/config paths;
- deploy scripts and release packaging;
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

Also verify no user-facing `Cell Symphony` strings remain except intentional migration/history references.
