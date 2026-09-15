# Protective case prototype

This is a printed protective-case check-fit prototype for a powered-off,
disconnected Octessera instrument. It adapts the Andy Wings CadQuery box with
two hinge assemblies, two-wall corner restraints, and branding. It is not a
transport shell, waterproof, drop-rated, or transport-qualified hardware.
Remove power and cables before closing it.

## Loading and opening

1. Put the deep tub exterior-down and cavity-up.
2. Lower the device controls-down, screen toward the north/latch and away from the hinge, onto the
   stationary corner restraints.
3. Close the shallow lid over the device's foam-backed underside.

The deep source top is normalized to the final case frame; the shallow source
bottom is the closing lid. The two hinge axes are at `X=63.9` and `X=191.7`,
with transformed axis `Y=-1.00/Z=54.85`.
The shallow lid has two protruding/flexible front/north latch actuators at
`X=72.8` and `X=182.8`; the deep tub has matching small rigid catch ridges.
Pull the lid tabs outward toward `+Y`, then lift. The latch engagement is
intentional kinematic collision from `0.01°` through `1.45°`, peaking at `0.55°`
and releasing at `1.46°`. Print-test force, wear, and fatigue; this is not FEA.
The upstream alternating knuckles are checked at source-relative X `-6,+6`
for the tub and `-12,0,+12` for the lid, across both hinge assemblies.

The copied upstream source is built with `clip_positions=()` for this case, so
it owns no transport latch solids. The project adapter reuses and mirrors the
upstream `Clip.top` and `Clip.bottom` profiles as the lid actuators and tub
catches. The copied upstream source is minimally adapted: its horizontal floor/roof
easing is conditional, so the configured `chamfer_xy=0` removes that easing on
both halves while the upstream default `4.0 mm` remains regression-tested. The
generated halves use a full-height rounded `251.2 x 144 mm R9.5` interior
centered at `(127.8,74.2)`: the tub spans `Z=2.2..54.65` and the lid spans
`Z=54.85..62.65`. Its segmented rounded `250 x 142 mm R8.5` lip is cut through
`Z=52.55..54.85`. The shallow lid has six `1.3 mm` tongue rails from `Z=52.65`
to `54.85`; the deep tub has matching receiver cuts with `0.25 mm` XY
clearance. The south rail is split around the hinge leaves. Four accepted
`Z=54.85` corner shoulders remain outside all 21 device-passage probes. The
validator checks center and `+/-1 mm` XY nominal engagement, plus the full-height
cavity in sampled sections. The unsupported hinge-side bar remnants are removed
between the south rails; the remaining visible openings are intentional
alternating-knuckle clearance and must not be filled. This does not alter or
weaken the copied hinge loops, knuckles, or pin path. Reprint the shallow lid
after this source fix and physically test the hinge fit. The upstream diagnostic
lip remains `246.30 x 139.10 mm` at `Z=53` with measured diagonal normal
penetration `0.417 mm`.

## Pins and hinge

Use two independent pieces of standard `1.75 mm` filament, each suggested at
`34.5 mm`, leaving `2.0 mm` exposed at both ends. The upstream pivot bore is
`2.25 mm`, giving `0.25 mm` nominal radial clearance. Standard filament is a
dimensional contract, not a material-compatibility guarantee; check the
physical fit.

After assembly, use controlled heated flat pliers/tool to pinch each exposed
end into a one-sided paddle directed away from the case wall. Do not use open
flame. Ventilate, avoid touching or deforming the printed hinge, and remove a
pin only by cutting and replacing it. The modeled paddle envelope is a
validation-only clearance allowance: `2.0 mm` axial length, `4.0 mm` vertical
span, `2.0 mm` outward depth, and `0.4 mm` inward allowance. It is not
guaranteed hand-formed geometry or a strength qualification.

## Corner restraints and device fit

The deep tub has a flat inner floor at `Z=2.2`. Each of the four solid
restraints is one continuous six-vertex ruled shelf support from `Z=2.00` to
its shelf underside; there are no additional ridge solids. The diagonal
exterior corner is left clear. Their validation contract
is at least `30 mm³` against each adjacent wall witness and at least `3.0 mm`
sampled support contact length at `Z=4`, with positive floor overlap and no
control keepout collision. The sloped supports avoid a designed hard overhang;
only the permitted `<=0.30 mm` rounded-corner slivers remain. The upper shelves
merge flush into both adjacent walls across the full band, with no designed
dust slot. They are shelves, not isolated pillars. The contact pads round their
plan corners by
`1.0 mm` and their exposed top edges by `0.6 mm`, while keeping a centered flat
contact face at the configured contact Z. This reduces sharp contact but does not
guarantee a scratch-free fit; check the slicer and test the printed case
physically.

The face-down placement is source-derived, not a proxy-envelope assumption.
The checked-in top source is `X=-1..247/Y=0..140/Z=-10..20`; applying
`(x,y,z)->(250.8-x,4.2+y,42-z)` gives the device frame
`X=3.8..251.8/Y=4.2..144.2/Z=22..52`. Put the controls down with the screen
toward the north/latch and away from the hinge. The raised physical wave reaches
the lower SE shelf; because Z is inverted, NE is the sole high shelf. The
corrected shelf base/contact pairs are NW/SW/SE `23.5/25.0` and NE `28.5/30.0`.
The source-built validator measures common planar contact of NW/SW `90.391 mm²`
and NE/SE `199.751 mm²`, and checks the final deep tub directly.
The floor carries one 76-component physical orientation guide: the outer
outline, screen, four encoder rings, four NeoKey outlines, sixty-four Trellis
cells, and an original two-curved-arrow turnaround symbol beside the east-side
screen and encoder cluster. Keep the controls down and the screen toward the
north/latch, away from the hinge. The
device-map strokes are
`0.8 mm`; the arrow symbol intentionally uses a stronger `1.0 mm` annular
stroke. Its two `135°` CCW arcs are the top and bottom halves of one circular
turn, and both arrowheads follow that same direction. Each arrowhead is a
directionally aligned flat-tip trapezoid, not text.

Both deep variants use this same XY guide. The single-material deep-tub version
raises all 76 components, including the arrows, from `Z=2.18..2.60`; the
markings overlap the `Z=2.2` floor by `0.02 mm` and belong to the one deep-tub
body. The deep multicolor version cuts the same guide from `Z=1.80` to `2.22`
and fills it with one second-color guide mesh from `Z=1.80..2.20`. The shallow
lid has no orientation guide.

The `248 x 140 R8 mm` device envelope and `249 x 141 R8 mm` check-fit envelope
are sampled from entry down to the seated `Z=7.5` base. Both seated solids have
zero collision with the closed pristine shallow lid, and their tops sit
`2.35 mm` below seam `Z=54.85`. Project restraint contacts are qualified
separately and are allowed by this fit check. The device and contact planes
remain provisional; measure the actual assembled instrument, feet, foam, and
controls before printing a final version.

### XY retention

Dry-fit the disconnected device before applying anything. The designed XY
retention is one narrow, replaceable closed-cell adhesive foam strip on each
straight inner-wall mid-span: west, east, south, and north. Start at `1 mm`
strip thickness and increase toward `2 mm` only as needed. Use opposing walls
for both axes so the device stays centered without a hard locator.

Keep the strips clear of the mating lip, upstream hinge and latch profiles, corner
shelves, controls, and ventilation openings. The device must remain removable
without hard force. The measured hard-wall clearance is nominal `+/-1.6 mm` in
X and `+/-2.0 mm` in Y; the check-fit envelope is `+/-1.1 mm` in X and
`+/-1.5 mm` in Y. Those are source-fit clearances, not a claim about compressed
final play: physical foam compression and device tolerance remain unmeasured.
The foam provides the designed XY retention; the solid shelves provide Z
support.

The deep tub also has four visible, wall-integrated landing pads that locate
the soft contact patches: west/east patches are `20 x 10 x 1.5 mm`, and
south/north patches are `20 x 10 x 2.0 mm`. Use one patch per pad, with no
stacking. Begin with the nominal fit (about `20%` compression) and dry-fit the
powered-off, disconnected device before trimming, replacing, or increasing
thickness. Check every side port and ventilation opening physically; the side
feature Z provenance is not complete enough to replace that check.

## Branding and print orientation

The deep tub uses a centered `72 mm` logo only, with authored bounds
`X91.8..163.8/Y48.364706..100.035294`; no wordmark is present. The shallow lid
reuses the canonical actual-device logo+wordmark placement from
`branding_marking_cadquery.py`: `64 mm` logo, `100 mm` wordmark, combined center
`(127.8,74.2)`, and `6 mm` gap. The complete shallow lockup is rotated `180°`
once around that center, so the logo and wordmark read correctly when the lid
is opened; their relative spacing and scale do not change. No device-origin
translation is applied. The shallow lid keeps the source orientation on
positive Z. The deep tub mirrors only X about `127.8` so the negative-Z exterior
reads correctly. Opened-lid bounds are logo
`X95.8..159.8/Y40.688124..86.617536` and wordmark
`X77.8..177.8/Y92.617536..107.711876`. Branding is recessed `0.40 mm`; flush
markings also use `0.40 mm` on extruder 2 over body extruder 1.
Each multicolor 3MF keeps one fixed owner-body transform under one root build
item: deep has body/logo/orientation-guide components, while shallow has
body/logo/wordmark components. Disconnected SVG strokes remain separate
marking mesh geometry.

STEP files remain in assembly coordinates. STL and 3MF bodies are oriented
exterior-down for printing. The printed deep tub is `255.6 x 153.835 x 57.85 mm`
and the shallow lid is `255.6 x 155.4 x 18.814590 mm`; both fit the `260 x 260 mm`
bed check. Debossed and multicolor outputs are experimental;
make no support-free claim before slicer and physical review.

## Outputs and source

The exact eight output paths are:

- `../../release-artifacts/enclosure/step/transport_case_deep_tub_debossed_logo.step`;
- `../../release-artifacts/enclosure/stl/transport_case_deep_tub_debossed_logo.stl`;
- `../../release-artifacts/enclosure/3mf/transport_case_deep_tub_debossed_logo.3mf`;
- `../../release-artifacts/enclosure/3mf-multicolor/transport_case_deep_tub_multicolor_logo.3mf`;
- `../../release-artifacts/enclosure/step/transport_case_shallow_lid_debossed_branding.step`;
- `../../release-artifacts/enclosure/stl/transport_case_shallow_lid_debossed_branding.stl`;
- `../../release-artifacts/enclosure/3mf/transport_case_shallow_lid_debossed_branding.3mf`;
- `../../release-artifacts/enclosure/3mf-multicolor/transport_case_shallow_lid_multicolor_branding.3mf`.

The source attribution and license note are in
[`../../hardware/enclosure/upstream/README.md`](../../hardware/enclosure/upstream/README.md).
The source, generator, and validator are documented in
[`../../hardware/enclosure/CAD_WORKFLOW.md`](../../hardware/enclosure/CAD_WORKFLOW.md).

The first physical print is a fit test. Check the shell mating ring, both hinge
assemblies, filament paddle forming, all restraints, branding depth, first
layer, and the disconnected device before using the case.
