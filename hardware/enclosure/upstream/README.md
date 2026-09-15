# Upstream parametric box

`andy_wings_parametric_box.py` is adapted from **Simple and Light Parametric
BOX – CadQuery** by Andy Wings / `@WingsWorld_2406962`:

- Printables: https://www.printables.com/model/1069138-simple-and-light-parametric-box-cadquery
- Matching Thingiverse listing: https://www.thingiverse.com/thing:6842165
- License: CC BY-SA, version unspecified by controlling source

Octessera keeps the upstream shell, mating ring, hinge, and Clip profile
algorithms. The adaptation only makes construction reusable, derives the hinge
leaf depth from each measure instance, supports configured clip positions, skips
the horizontal chamfer when `chamfer_xy=0` while preserving the default chamfer,
and guards default construction/export behind the module entrypoint.
