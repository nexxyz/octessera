# Octessera Rename Status

The Octessera rename is executed on the `octessera-rename` branch. Rollback is
the annotated tag `pre-octessera-rename`.

Naming mismatches should fail hard. Do not add compatibility aliases, fallback
imports, duplicate package names, old binary wrappers, old service aliases, or
project-owned path fallbacks.

## Final Names

- Product, device, and app name: `Octessera`.
- Repository name: `octessera`.
- User-facing binaries: `octessera-pi`, `octessera-desktop`, and
  `Octessera.exe`.
- Pi systemd services: `octessera.service`, `octessera-boot-splash.service`,
  `octessera-oled-shutdown.service`, and
  `octessera-performance-governor.service`.
- pnpm packages: `@octessera/desktop` and `@octessera/device-contracts`.
- Rust packages/crates: `octessera-desktop`, `octessera-pi`, and
  `octessera-hal`.
- Pi install path: `/opt/octessera`.
- Pi image stage: `tools/pi-image/stage4-octessera`.
- PCB source and fabrication names: `hardware/pcb/octessera.*` and
  `release-artifacts/pcb/gerber/octessera-*`.

## Completed Phases

1. User-facing identity:
   - product strings, desktop metadata, OLED/splash/status/help text, user docs,
     release asset names, and visible desktop artifact names;
   - Tauri identifier changed to `xyz.nex.octessera`, intentionally changing OS
     app identity/app-data resolution with no fallback to prior app data.
2. Internal-facing identity:
   - pnpm package names/imports, Cargo package and binary names, package filters,
     runtime labels, tests, lockfiles, temp prefixes, MIDI labels, and project
     environment variable names.
3. Repository, deployment, and artifact cleanup:
   - Pi service names, install paths, deploy/build scripts, Pi image stage paths,
     Cross targets, desktop sample defaults, hardware docs, CAD/PCB source names,
     and generated release artifact filenames.

## Data Policy

No startup fallback search paths were added for prior store/config/sample
locations. If data preservation is required later, implement it as a separately
reviewed one-time migration with tests, not as a runtime alias.

## Validation Gates

Run before merging or releasing:

- `corepack pnpm run typecheck`;
- `corepack pnpm -r test`;
- `corepack pnpm -r lint`;
- `corepack pnpm -r format:check`;
- `corepack pnpm run quality:audit`;
- `cargo fmt --all --check`;
- relevant Rust tests and clippy using Octessera package names;
- Pi cross-build;
- desktop portable executable build;
- Pi deploy smoke test when hardware is available.

Also verify:

- no prior project names remain in tracked content or filenames;
- no fallback imports, package aliases, service aliases, old binary wrappers, or
  project-owned old path fallbacks remain;
- release artifacts use Octessera names after source regeneration;
- package lockfile and Cargo lockfile changes are intentional.
