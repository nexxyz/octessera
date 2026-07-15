# Pi DSP FX Profile

This note records the current Pi-side basis for the bus FX warning budget.

## 2026-07-15: 3-slot FX bus profile

Target: Raspberry Pi running `octessera-pi` with `OCTESSERA_PI_PROFILE_MODE=fx-limits` and `--profile-dsp`.

Profile setup:

- 44.1 kHz, 128-frame render blocks.
- 1,500 measured blocks per scenario.
- 8 active synth voices routed through bus FX.
- 2 fixed global FX slots: compressor + reverb.
- Product active bus FX slots tested: 0, 2, 4, 6, 8, 10, 12.
- Synthetic over-cap active bus FX slots tested: 15, 18, 21, 24.
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

Synthetic over-cap rows were measured to see the scaling curve beyond the shipped 4-bus limit. They use up to 8 synthetic buses with 3 slots each. These are engine stress tests, not user-facing patch limits.

| Active bus FX | Momentary FX | Avg raw ratio | P95 | P99 | Max |
|---:|---:|---:|---:|---:|---:|
| 15 | 2 | 0.665 | 0.674 | 0.717 | 1.083 |
| 18 | 2 | 0.583 | 0.591 | 0.604 | 1.026 |
| 21 | 2 | 0.674 | 0.683 | 0.694 | 1.088 |
| 24 | 2 | 0.677 | 0.685 | 0.695 | 1.087 |

Beyond 12 slots, p99 remains below realtime, but occasional maximum blocks cross the realtime budget when 2 momentary FX are also active. That supports keeping the product warning budget at 12 rather than treating the synthetic headroom as a safe patch target.
