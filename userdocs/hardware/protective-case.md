# Protective case check-fit prototype

This printed case is a check-fit prototype for a powered-off, disconnected
Octessera. It is not waterproof, drop-rated, or transport-qualified. Treat the
first print as a friendly argument with reality, not proof that every printer
and assembled instrument will fit.

## Load and open it

1. Put the deep tub exterior-down and cavity-up.
2. Disconnect power and every cable from the instrument.
3. Lower the instrument controls-down. Point the screen toward the north/front
   latches, away from the hinge.
4. Seat it gently on all four corner shelves. Stop if a control, connector, or
   enclosure edge touches hard geometry.
5. Close the shallow lid over the instrument's underside.

To open the case, pull the two lid tabs outward toward `+Y`, then lift. Do not
force the latches. Their fit, wear, and fatigue still need physical print tests.

## Fit status

The case is checked in CAD against both enclosure tops, Raspberry and Orange,
not against a printed instrument. Four corner shelves carry the instrument and
foam strips keep it centered, so expect to tune the foam on your first fit.
The guide on the tub floor shows which way round the instrument goes.
The CAD numbers live in the [enclosure CAD
workflow](../../hardware/enclosure/CAD_WORKFLOW.md#protective-case-check-fit-source).

### Foam retention

Use one narrow, replaceable closed-cell adhesive foam strip at the middle of
each straight inner wall. Start at `1 mm` thickness and increase only as needed,
up to `2 mm`. Use opposing walls to keep the instrument centered.

The wall pads accept one patch each:

- west/east: `20 x 10 x 1.5 mm`;
- south/north: `20 x 10 x 2.0 mm`.

Do not stack patches. Keep foam clear of controls, connectors, ventilation,
latches, hinges, shelves, and the mating lip. The instrument must remain
removable without hard force.

## Hinge pins

Use two pieces of `1.75 mm` filament, each about `34.5 mm` long, with `2 mm`
exposed at each end. The modeled pivot bore is `2.25 mm`; verify the printed fit.

If you retain the pins by flattening their exposed ends, use controlled heated
flat pliers and direct each flattened end away from the case wall. Do not use an
open flame. Ventilate, avoid heating the printed hinge, and remove a pin by
cutting and replacing it.

## Printing and branding

STEP files remain in assembly coordinates. STL and 3MF files are oriented
exterior-down for printing. The deep tub uses a centered logo and a floor
orientation guide. The shallow lid uses the logo and wordmark. Recessed and
multicolor markings are `0.40 mm` deep; multicolor marks use extruder 2 over the
body on extruder 1.

Both halves fit the source `260 x 260 mm` bed check. This does not establish a
support-free print. Inspect the slicer preview, especially the shelf transitions,
hinges, latches, first layer, and recessed marks.

## Outputs

- `../../hardware/enclosure/step/protective_case_deep_tub_debossed_logo.step`
- `../../hardware/enclosure/stl/protective_case_deep_tub_debossed_logo.stl`
- `../../hardware/enclosure/3mf-single-material/protective_case_deep_tub_debossed_logo.3mf`
- `../../hardware/enclosure/3mf-multicolor/protective_case_deep_tub_multicolor_logo.3mf`
- `../../hardware/enclosure/step/protective_case_shallow_lid_debossed_branding.step`
- `../../hardware/enclosure/stl/protective_case_shallow_lid_debossed_branding.stl`
- `../../hardware/enclosure/3mf-single-material/protective_case_shallow_lid_debossed_branding.3mf`
- `../../hardware/enclosure/3mf-multicolor/protective_case_shallow_lid_multicolor_branding.3mf`

Source attribution is in
[`../../hardware/ATTRIBUTIONS.md`](../../hardware/ATTRIBUTIONS.md). Generator
ownership and validation commands are in
[`../../hardware/enclosure/CAD_WORKFLOW.md`](../../hardware/enclosure/CAD_WORKFLOW.md).

Before using the case, physically check the disconnected instrument, all side
openings, corner contacts, foam compression, mating lip, hinges, pins, latches,
branding depth, and first layer.
