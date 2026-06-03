# Engineering Quality, CI, and Test Requirements

## 1) Quality Goals

This document defines the minimum engineering quality baseline for Cell Symphony.

Primary goals:

- Deterministic core behavior for automata and mapping logic
- Stable realtime timing under normal load
- Reliable cross-platform desktop builds
- Reproducible project portability

## 2) Repository and Tooling Baseline

- Monorepo using `pnpm` workspaces
- TypeScript for shared/UI packages
- Rust for realtime engine and import backend
- Tauri for desktop packaging

Recommended baseline tools:

- JS/TS formatting: Prettier
- JS/TS linting: ESLint with TypeScript rules
- Rust formatting: rustfmt
- Rust linting: clippy (`-D warnings` in CI)

## 3) Test Strategy

### 3.1 Test layers

- Unit tests
  - CA state transitions
  - Mapping rules and quantization
  - Project serialization/deserialization
- Integration tests
  - Device-contract event flow (UI -> core/backend -> render models)
  - Sample import conversion (FLAC -> WAV metadata and file output)
  - MIDI event sequencing behavior
- End-to-end desktop smoke tests
  - App launch
  - Control mapping correctness (keyboard + clickable matrix)
  - Minimal audio/MIDI path sanity checks

### 3.2 Determinism requirements

- Core CA + mapping tests must validate repeatable output for fixed seeds/settings.
- Golden tests are recommended for representative CA-to-music scenarios.

### 3.3 Audio/timing requirements

- Scheduler drift/jitter must remain within defined thresholds during test runs.
- Add benchmark checks for dense-pattern scenarios.

## 4) Coverage and Quality Gates

Minimum CI gate requirements:

- All lint/format checks pass.
- All unit/integration tests pass.
- No TypeScript compile errors.
- Rust clippy and test suite pass.

Coverage targets (initial):

- `packages/platform-core`: >= 90% line coverage
- `packages/mapping-core`: >= 90% line coverage
- `packages/device-contracts`: >= 95% line coverage
- `packages/interpretation-core`: >= 90% line coverage
- `packages/behavior-api`: >= 90% line coverage
- Each behavior package (`packages/behaviors-*`): >= 85% line coverage
- UI and Rust coverage tracked and improved incrementally, with hard gate added after baseline maturity

Static quality requirements:

- No TODO/FIXME introduced without issue reference.
- No dead exported APIs in shared packages.
- No unchecked panics in non-test Rust paths without rationale.

## 5) GitHub Actions CI Pipeline Requirements

## 5.1 Trigger policy

- On pull requests to main branches
- On pushes to protected branches
- Optional nightly workflow for heavier integration/perf runs

### 5.2 Required jobs (parallel where possible)

1. `lint_ts`
   - Install deps
   - Run ESLint for TS/JS
2. `format_check_ts`
   - Run Prettier check
3. `typecheck_ts`
   - Run TypeScript project checks
4. `test_ts`
   - Run unit/integration tests for TS packages with coverage
5. `rust_fmt`
   - `cargo fmt --check`
6. `rust_lint`
   - `cargo clippy -- -D warnings`
7. `rust_test`
   - `cargo test --workspace`
8. `desktop_build_smoke`
   - Build Tauri app in CI mode (no release signing)
9. `artifact_validation`
   - Verify sample import outputs and project fixture portability checks

### 5.3 Matrix requirements

- Primary CI target OS: Windows and Linux
- Optional macOS build verification once packaging is enabled

### 5.4 Caching requirements

- pnpm cache
- cargo registry and target cache
- Node modules cache strategy compatible with lockfile changes

### 5.5 Artifact requirements

- Upload test reports (JUnit or equivalent)
- Upload coverage reports
- Upload build logs for failed desktop packaging jobs

## 6) Code Review and Merge Requirements

- PR must reference requirement or issue intent.
- At least one reviewer approval required.
- CI status checks required before merge.
- No direct pushes to protected main branch.

Recommended PR hygiene:

- Keep PRs scoped to one milestone concern when possible.
- Include test evidence and risk notes.
- Include screenshots/video for simulator UI behavior changes.

## 7) Security and Supply Chain Requirements

- Pin dependency versions through lockfiles.
- Enable dependency vulnerability scanning in GitHub.
- Block known-critical vulnerabilities in CI.
- Restrict workflow permissions to least privilege.

## 8) Performance and Reliability Requirements

- Define and track startup time budget for desktop app.
- Define and track max CPU budget for reference dense CA pattern.
- Detect xrun/dropout risk indicators where possible.
- Keep realtime thread work minimal and avoid blocking disk I/O on audio path.
- Any feature that produces internal musical audio must include engine-boundary tests proving the signal reaches the canonical route/pan/bus-FX mixer.

## 9) Definition of Done (Engineering)

A feature is done when:

- Requirements/spec references are updated.
- Tests are present and passing at appropriate layers.
- CI quality gates pass.
- Observability/diagnostic impact is considered.
- User-visible behavior is documented where relevant.
