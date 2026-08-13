# Dependency license inventory

This directory contains generated dependency license text and inventories. It
complements, and does not replace, the notices for non-code assets such as
samples. It is an engineering record, not legal advice.

## Pins and inputs

- Cargo workspace lockfile: `Cargo.lock`, SHA-256
  `0225ccdf24258b816cc152df5571601f91e2c380afc06a44d9bf2157e87d6bae`.
- `cargo-about` **0.9.1**. Install with:
  `cargo install cargo-about --version 0.9.1 --locked --features cli`.
- Cargo configuration: `about.toml`; reviewed template:
  `tools/legal/cargo_about.hbs`.
- Hand-reviewed exact policy: `licenses/cargo/reviewed-dependency-policy.json`.
- Pinned SPDX references: MPL-2.0 SHA-256
  `66a3107d5ad6a058aab753eaac2047ccb2ed0e39465dd0fe5844da3e300d5172` and
  Apache-2.0 SHA-256
  `c71d239df91726fc519c6eb72d318ec65820627232b2f796219e87dcf35d0ab4`;
  MIT, BSD-3-Clause, Zlib, and LLVM-exception references are pinned in the
  same directory and verified by the generator.
- pnpm: **9.12.0**, pinned by the root `package.json` `packageManager` field.
- pnpm lockfile: `pnpm-lock.yaml`, SHA-256
  `f3f796dfca32246fa747360c417018104adb04dbf4ad2a6c873de532c7b7cd47`.
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

The Cargo `SOURCE_INDEX.json` is informational. It records the exact lockfile
package identities, target-profile context, source URLs, checksums, and the
MPL/manifest-no-file flags used to identify packages requiring source-availability
review before public binary release. It makes no completeness claim and does
not require local `.crate` archives.

When a local Cargo registry source lacks Cargo's `.cargo-checksum.json` sidecar,
generation derives the equivalent checksum payload from the exact cached
`.crate` bytes and validates the unpacked files against it. The archive itself
is not copied into this repository.

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

Normal CI runs the lightweight `python -m unittest discover -s tools/legal
-p 'test_*.py'` suite. The generated dependency inventory verifier and its
`cargo-about` audit remain pre-release/manual checks; CI does not install
`cargo-about` for the unit tests.

The Cargo command below is a policy audit, not a way to hide unresolved
licenses:

```text
cargo about generate --workspace --all-features --frozen --fail --format json -o licenses/cargo/cargo-about-review.json
```

The Cargo output is `cargo-lock-overinclusive`: it covers every lockfile
identity and is not a claim that all 509 packages ship in every target profile.
`SOURCE_INDEX.json` records release target source obligations separately.
`cargo-about` remains advisory; the exact policy checker, not its allowlist,
authorizes the ten MPL records and two r-efi Apache alternatives.

## Review status

The Cargo inventory records 497 permissive packages, ten reviewed MPL packages,
and two reviewed r-efi alternatives. It records 68
`manifest-license-no-file` packages as informational and 73 packages requiring
source-availability review before public binary release. All seven first-party
crates inherit the root `LICENSE`; that custom license is validated but
excluded from the third-party index. There are zero custom/unknown packages
and zero unresolved policy decisions.

The locally vendored `third_party/cpal-0.15.3` is included explicitly. It is
marked `modified-local-vendoring`, never as an unmodified upstream package, and
its original `LICENSE` and `PROVENANCE.md` are preserved and referenced in the
Cargo inventory.
