# Dependency license inventory

This directory contains generated dependency license text and inventories. It
complements the notices for non-code assets such as samples.

## Pins and inputs

- `cargo-about` **0.9.1**. Install with:
  `cargo install cargo-about --version 0.9.1 --locked --features cli`.
- Cargo configuration: `about.toml`; template:
  `tools/legal/cargo_about.hbs`.
- License policy: `licenses/cargo/reviewed-dependency-policy.json`.
- Pinned SPDX references: MPL-2.0 SHA-256
  `66a3107d5ad6a058aab753eaac2047ccb2ed0e39465dd0fe5844da3e300d5172` and
  Apache-2.0 SHA-256
  `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`;
  MIT, BSD-3-Clause, Zlib, and LLVM-exception references are pinned in the
  same directory and verified by the generator.
- pnpm: **9.12.0**, pinned by the root `package.json` `packageManager` field.
- Production package discovery command:
  `corepack pnpm licenses list --prod --json`.

The current production set is `@tauri-apps/api` 2.11.0, `react` 18.3.1,
`react-dom` 18.3.1, `scheduler` 0.23.2, `loose-envify` 1.4.0, and
`js-tokens` 4.0.0. The checked pnpm inventory copies each package's installed
license/notice-family files and records their SHA-256 values.

## Regenerate and check

From the repository root:

```text
python tools/legal/dependency_license_generate.py
python tools/legal/dependency_license_generate.py --check
python tools/legal/dependency_license_check.py
```

The generator is the only command that writes the generated files. The
verification entrypoint is read-only and compares a freshly rendered byte
representation with the checked files. It also runs locked offline Cargo
metadata, the pinned pnpm license command, lockfile/package-content checks,
symlink and path checks, and both SHA manifests.

The Cargo `SOURCE_INDEX.json` records exact lockfile package identities,
target-profile context, source URLs, checksums, and package source/license flags.

Cargo generation resolves `cargo metadata --locked --offline --all-features` so
the inventory covers every package identity present in `Cargo.lock`, including
platform-optional packages.

Additional pinned-tool checks:

```text
cargo metadata --locked --format-version 1
corepack pnpm licenses list --prod --json
python -m py_compile tools/legal/dependency_license_generate.py tools/legal/pnpm_dependency_license_generate.py tools/legal/cargo_dependency_license_support.py tools/legal/cargo_dependency_license_render.py tools/legal/dependency_license_check.py tools/legal/test_dependency_license.py
python -m unittest tools/legal/test_dependency_license.py
```

The Cargo inventory render command is:

```text
cargo about generate --workspace --all-features --frozen --fail --format json -o licenses/cargo/cargo-about-review.json
```

The generated Cargo inventory covers every lockfile identity resolved by the
workspace metadata command above. `SOURCE_INDEX.json` records the corresponding
source references.

The locally vendored `third_party/cpal-0.15.3` is included explicitly. It is
marked `modified-local-vendoring`, never as an unmodified upstream package, and
its original `LICENSE` and `PROVENANCE.md` are preserved and referenced in the
Cargo inventory.
