# Pi DSP FX Profile

This note records the current Pi-side basis for the bus FX warning budget.

## 2026-07-15: 3-slot FX bus profile

Target: Raspberry Pi running `octessera-pi` with `OCTESSERA_PI_PROFILE_MODE=fx-limits` and `--profile-dsp`.

Profile setup:

- 44.1 kHz, 128-frame render blocks.
- 1,500 measured blocks per scenario.
- 8 active synth voices routed through bus FX.
- 2 fixed global FX slots: compressor + reverb.
- Active bus FX slots tested: 0, 2, 4, 6, 8, 10, 12.
- Momentary FX tested per bus load: 0, 1, 2.
- The 12-slot bus mix uses: delay, reverb, glitch, flanger, chorus, filter_lfo, wah, vibrato, vinyl, auto_pan, compressor, eq.

Worst observed current-effect bus scenarios:

| Active bus FX | Momentary FX | Avg raw ratio | P95 | P99 | Max |
|---:|---:|---:|---:|---:|---:|
| 8 | 2 | 0.579 | 0.589 | 0.638 | 0.924 |
| 10 | 2 | 0.529 | 0.538 | 0.551 | 0.869 |
| 12 | 2 | 0.529 | 0.537 | 0.550 | 0.896 |

The Pi reported `throttled=0x0`; temperature rose from 55.3 C to 58.0 C.

Recommendation: set `busFxWarningSlotCount` to 12 for the current effect set. This is the full current bus capacity: 4 buses × 3 slots. Global FX slots do not count toward this budget. Revisit the budget if heavier future FX are added.
