import logging
from types import SimpleNamespace as Measures

import cadquery as cq

# Adapted from “Simple and Light Parametric BOX – CadQuery” by Andy Wings /
# @WingsWorld_2406962.
# Source: https://www.printables.com/model/1069138-simple-and-light-parametric-box-cadquery
# License: CC BY-SA 4.0: https://creativecommons.org/licenses/by-sa/4.0/
# Octessera adapted and modified it with reusable construction, configurable clip placement,
# conditional horizontal chamfer, and guarded default export.

# ===== (1) Measures =====
def default_measures():
    return Measures(
        width=100,
        depth=60,
        height=30,
        thickness=2,
        clearance=0.25,
        top_height=10,
        chamfer_z=8,
        chamfer_xy=4,
        text="BOX",
        hinge=Measures(
            thickness=1.5,
            kunkle_size=6.0,
            number_of_kunkles=5,
            clearance=0.5,
            leaf_width=6.0,
            leaf_height=2.0,
            pivot_radius=1.75 / 2,
        ),
        clip=Measures(length=10.0, positions_x=(0.0,)),
    )


def _derive_leaf_depth(hinge):
    hinge.leaf_depth = hinge.number_of_kunkles * hinge.kunkle_size + hinge.clearance
    return hinge


measures = default_measures()
_derive_leaf_depth(measures.hinge)

log = logging.getLogger(__name__)

# ===== (3) Implementation =====

class Hinge():

    def __init__(self, measures):
        """
        A parametric hinge.

        :param workplane: The CadQuery workplane to create this part on. (NOT USED)
        :param measures: The measures to use for the parameters of this design. Expects a nested
            [SimpleNamespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace)
            object.

        .. todo:: Complete the class implementation

        """
        self.debug = False
        self.measures = measures

        m = self.measures

        self.build()

    def build(self):
        m = self.measures
        _derive_leaf_depth(m)

        d=m.kunkle_size
        n=m.number_of_kunkles
        tick=m.thickness
        clear=m.clearance
        leaf_w=m.leaf_width
        leaf_h=m.leaf_height
        leaf_p=m.leaf_depth
        pivot_radius=m.pivot_radius

        l=d-clear
        r0=0
        if (pivot_radius > 0):
            r1=pivot_radius
            r2=pivot_radius + clear/2
        else:
            r1=tick-clear/2
            r2=tick+clear/2

        r3=tick+tick

        vuoto = ( cq.Workplane(origin=(0, r2, 0))
             .rarray(d*2,1,int(n/2),1,True).rect(l,r3-r2, centered=False).revolve(360, (0, -r2, 0), (-1, -r2, 0))
             #.rarray(d*2,1,int(n/2),1,True).rect(l,r3-r2).extrude(1)
        )
        vuoto=vuoto.translate(vuoto.val().BoundingBox().center.multiply(-1))

        s1=cq.Sketch().rarray(d*2,1,int(n/2)+1,1).rect(l,r3).moved(cq.Location(cq.Vector(0, r3/2, 0)))
        s2=cq.Sketch().rect(d*n-clear,r1-r0).moved(cq.Location(cq.Vector(0, (r1-r0)/2, 0)))
        pieno=cq.Workplane().placeSketch(s1,s2).revolve(360,(0,0,0),(1,0,0))


        cutter_inv_v=  ( cq.Workplane(origin=(0, r2, 0))
             .rarray(d*2,1,int(n/2),1,True).rect(l+2*clear,r3+clear-r2, centered=False).revolve(360, (0, -r2, 0), (-1, -r2, 0))
        )
        cutter_inv_v=cutter_inv_v.translate(cutter_inv_v.val().BoundingBox().center.multiply(-1))

        #cutter=cq.Workplane("ZY").cylinder(n*d,r3).union( vuoto.val().scale((r3+clear)/r3) )
        cutter_p=cq.Workplane("ZY").cylinder(n*d-clear,r3).union( cutter_inv_v )

        s3=cq.Sketch().rarray(d*2,1,int(n/2)+1,1).rect(l+2*clear,r3+clear).moved(cq.Location(cq.Vector(0, (r3+clear)/2, 0)))
        cutter_inv_p=cq.Workplane().placeSketch(s3).revolve(360,(0,0,0),(1,0,0))
        cutter_v=cq.Workplane("ZY").cylinder(leaf_p,r3).union( cutter_inv_p )

        pieno=pieno.union(
                cq.Workplane(origin=(0,0,-r3+leaf_h/2)).box(leaf_p,leaf_w,leaf_h).translate((0,leaf_w/2,0))
            .union(
                cq.Workplane(origin=(0,0,0)).box(leaf_p,3*tick,(2)*tick).translate((0,3/2*tick,-(1)*tick)).faces(">Y")
                .edges(">Z").chamfer(tick)
            ).cut(cutter_p)
        )

        # for holed hinge
        if (pivot_radius > 0):
            pivot=cq.Workplane("ZY").cylinder( leaf_p , pivot_radius+clear/2)
            pieno=pieno.cut(pivot)

        vuoto=vuoto.union(
            cq.Workplane(origin=(0,0,-r3+leaf_h/2)).box(leaf_p,leaf_w,leaf_h).translate((0,-leaf_w/2,0)).union(
                cq.Workplane(origin=(0,0,0)).box(leaf_p,3*tick,2*tick).translate((0,-3/2*tick,-tick)).faces("<Y").edges(">Z").chamfer(tick)
            )
            .cut(cutter_v)
        )

        def hinge_base():
            #return cq.Workplane(origin=(0,0,0)).box(leaf_p,2*leaf_w,leaf_h+clear)
            return (cq.Workplane(origin=(0,0,-r3+leaf_h/2)).box(leaf_p,2*leaf_w,leaf_h)
                .union(cutter_p.union(cutter_v))  )

        self.pieno=pieno.rotate((0,0,0),(1,0,0),-90)
        self.vuoto=vuoto.rotate((0,0,0),(1,0,0),-90)
        self.solid=self.base=hinge_base().rotate((0,0,0),(1,0,0),-90)

        return

class Clip():
    def __init__(self, measures):
        """
        A parametric clop.

        :param measures: The measures to use for the parameters of this design. Expects a nested
            [SimpleNamespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace)
            object.

        .. todo:: Complete the class implementation

        """
        self.debug = False
        self.measures = measures
        self.build()

    def build(self):
        m = self.measures
        lip_clip=(cq.Workplane("XY")
            .moveTo(-8,0)
            .lineTo(-5,3)
            .lineTo(5.5,3)

            .lineTo(7,3)
            .lineTo(5.5,0)
            .lineTo(4,1.5)
            .lineTo(2,1.5)
            .lineTo(-2.5,1.5)
            .lineTo(-4, 0)
            .close()
          .extrude(m.length, both=True)
             .edges(">>X[1] or <<X[0] or <<X[1] or <<X[2] or <<X[3] or >Y").fillet(0.3)

        ).mirror("XZ").rotate((0,-1,0),(0,1,0),90  )

        bottom_clip = (
             cq.Workplane("XY")
                .moveTo(5.5,0)
                .lineTo(4,1.5)
                .lineTo(1,0)
                .close()
             .extrude(m.length+2, both=True)
            .edges("not (<Y)").fillet(0.3)

        ).mirror("XZ").rotate((0,-1,0),(0,1,0),90  )

        self.top=lip_clip
        self.bottom=bottom_clip


class Box():

    def __init__(self, measures):
        """
        A parametric Box.

        :param measures: The measures to use for the parameters of this design. Expects a nested
            [SimpleNamespace](https://docs.python.org/3/library/types.html#types.SimpleNamespace)
            object.

        .. todo:: Complete the class implementation

        """
        self.debug = False
        self.measures = measures
        self.clipped = False
        self.hinged = False
        self.build()

    def build(self):
        m = self.measures

        b_w = m.width
        b_d = m.depth
        b_h = m.height
        b_t = (m.thickness , m.thickness)
        gap_t = m.clearance
        top_h = m.top_height
        _derive_leaf_depth(m.hinge)
        m.bottom_height= bottom_h = b_h -top_h
        self.rel_z = m.bottom_height - (m.height/2)

        #box=cq.Workplane().box(b_w, b_d, b_h).chamfer(4)
        box=( cq.Workplane(origin=(0, 0, 0)).box(b_w, b_d, b_h)
            .edges("|Z").chamfer(m.chamfer_z)
            )
        if m.chamfer_xy > 0:
            box = box.edges("#Z").chamfer(m.chamfer_xy)

        box_b=box.faces(">Z").workplane(offset=-top_h).split(keepBottom=True)
        box_t=box.faces(">Z").workplane(offset=-(top_h-gap_t)).split(keepTop=True)

        # Giro anello inferiore
        tool1 = box_b.faces(">Z").wires().toPending().offset2D(-b_t[0]).extrude(-b_t[0]-gap_t, combine=False)
        tool1 = tool1.faces("<Z").wires().toPending().extrude(b_t[0]+gap_t, combine='cut', taper=45)

        #box_b1 = box_b.faces(">Z").wires().toPending().offset2D(-b_t[0]-gap_t).extrude(b_t[1])\
        #    .faces(">Z").shell(-b_t[0])

        # Giro anello superiore
        tool2 = box_b.faces(">Z").wires().toPending().offset2D(-b_t[0]-gap_t).extrude(b_t[1], combine=False) \
            .faces(">Z").wires().toPending().offset2D(-b_t[0]).extrude(-b_t[1], combine='cut')
        tool1.union(tool2)
        box_b1=box_b.faces(">Z").shell(-b_t[0]).union(tool1).union(tool2)
        box_t1=box_t.faces("<Z").shell(-b_t[0])
        if m.text:
            box_t1=box_t1.faces(">Z").workplane().text(m.text, 20, -1.0)

        self._top_source = box_t1
        self._bottom_source = box_b1
        self._top = box_t1
        self._bottom= box_b1
        self.place_hinge()
        self.place_clip()

    def place_clip(self):
        m=self.measures
        rel_z= self.rel_z
        rel_y= m.thickness
        if (not self.clipped):
            self.clip= Clip(m.clip)
            for position_x in m.clip.positions_x:
                self._top=self._top.union(self.clip.top.translate((position_x, -m.depth/2  , rel_z-2)) )
                self._bottom=self._bottom.union(self.clip.bottom.translate((position_x, -m.depth/2  , rel_z-2)) )
            self._top=self._top.translate((0,-m.depth/2-rel_y,0))
            self._bottom=self._bottom.translate((0,-m.depth/2-rel_y,0))
            self._top_source=self._top_source.translate((0,-m.depth/2-rel_y,0))
            self._bottom_source=self._bottom_source.translate((0,-m.depth/2-rel_y,0))
            self.clipped = True

    def place_hinge(self):
        m=self.measures
        rel_z= self.rel_z
        b_w=m.width
        b_d=m.depth

        if (not self.hinged):
            hinge = Hinge(m.hinge)
            rel_y= 2 * m.hinge.thickness - m.hinge.leaf_height
            if (m.width >= 2 * m.hinge.leaf_depth + 4*m.chamfer_z):
                # 2 hinges
                box_b3= (self._bottom
                    .cut(hinge.solid.translate((b_w * 0.25,b_d/2+ rel_y,rel_z)) )
                    .cut(hinge.solid.translate((- b_w * 0.25,b_d/2 + rel_y,rel_z)) )
                    .union(hinge.pieno.translate(( b_w * 0.25, b_d/2 + rel_y ,rel_z) ) )
                    .union(hinge.pieno.translate( (- b_w * 0.25, b_d/2 + rel_y ,rel_z) ) )
                )

                box_t3= (self._top
                    .cut(hinge.solid.translate((b_w * 0.25,b_d/2+ rel_y,rel_z)) )
                    .cut(hinge.solid.translate((- b_w * 0.25,b_d/2+ rel_y,rel_z)) )
                    .union(hinge.vuoto.translate(( b_w * 0.25, b_d/2 + rel_y ,rel_z) ) )
                    .union(hinge.vuoto.translate( (- b_w * 0.25, b_d/2 + rel_y ,rel_z) ) )
                )
            else:
                # 1 hinge
                box_b3= (self._bottom
                    .cut(hinge.solid.translate((0 ,b_d/2+ rel_y,rel_z)) )
                    .union(hinge.pieno.translate(( 0, b_d/2 + rel_y ,rel_z) ) )
                )

                box_t3= (self._top
                    .cut(hinge.solid.translate((0,b_d/2+ rel_y,rel_z)) )
                    .union(hinge.vuoto.translate(( 0, b_d/2 + rel_y ,rel_z) ) )
                )



            self._top=box_t3
            self._bottom=box_b3
            self.hinged = True

    def top(self):
        return self._top

    def bottom(self):
        return self._bottom

    def source_top(self):
        return self._top_source

    def source_bottom(self):
        return self._bottom_source

    def save_stl(self):
        m = self.measures
        dims = "H"+str(m.height).zfill(3) +"_W" \
            + str(m.width).zfill(3) +"_D" \
            + str(m.depth).zfill(3)
        print("Saving dims: ", dims)
        cq.exporters.export(self._top,"Box_" + dims + "_Lid.stl")
        cq.exporters.export(self._bottom,"Box_" + dims + "_Bottom.stl")


def build_box(measures):
    return Box(measures)


if __name__ == "__main__":
    build_box(measures).save_stl()
