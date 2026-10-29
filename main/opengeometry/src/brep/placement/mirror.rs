use super::check_coordinate_precision;
use crate::brep::error::GeometryError;
use crate::brep::frame::{checked, unit, Frame3, UV};
use crate::brep::geometry::{CurveGeometry, IntersectionDefinition, SurfaceGeometry};
use crate::brep::pcurve::PcurveGeometry;
use crate::brep::topology::BrepEnvelope;
use crate::math::{add, dot, scale, sub, Interval, Point3};
use std::f64::consts::TAU;

#[derive(Clone, Copy)]
struct Reflection {
    origin: Point3,
    normal: Point3,
}

#[derive(Clone, Copy, PartialEq)]
struct ChartMap {
    sign: [f64; 2],
    shift: [f64; 2],
}

const PLANE_MAP: ChartMap = ChartMap {
    sign: [1.0, -1.0],
    shift: [0.0; 2],
};
const REVOLVED_MAP: ChartMap = ChartMap {
    sign: [-1.0, 1.0],
    shift: [TAU, 0.0],
};

impl Reflection {
    fn vector(self, v: Point3) -> Point3 {
        sub(v, scale(self.normal, 2.0 * dot(v, self.normal)))
    }

    fn point(self, p: Point3) -> Point3 {
        add(
            p,
            scale(self.normal, -2.0 * dot(sub(p, self.origin), self.normal)),
        )
    }

    fn surface_frame(self, frame: Frame3) -> Frame3 {
        Frame3 {
            origin: self.point(frame.origin),
            x: self.vector(frame.x),
            y: scale(self.vector(frame.y), -1.0),
            z: self.vector(frame.z),
        }
    }

    fn curve_frame(self, frame: Frame3) -> Frame3 {
        Frame3 {
            origin: self.point(frame.origin),
            x: self.vector(frame.x),
            y: self.vector(frame.y),
            z: scale(self.vector(frame.z), -1.0),
        }
    }
}

impl ChartMap {
    fn of(surface: &SurfaceGeometry) -> Self {
        if matches!(surface, SurfaceGeometry::Plane { .. }) {
            PLANE_MAP
        } else {
            REVOLVED_MAP
        }
    }

    fn point(self, uv: UV) -> UV {
        std::array::from_fn(|axis| self.sign[axis] * uv[axis] + self.shift[axis])
    }

    fn vector(self, uv: UV) -> UV {
        std::array::from_fn(|axis| self.sign[axis] * uv[axis])
    }

    fn interval(self, axis: usize, range: Interval) -> Result<Interval, GeometryError> {
        let ends = [range.lo, range.hi].map(|end| self.sign[axis] * end + self.shift[axis]);
        Ok(Interval::new(ends[0].min(ends[1]), ends[0].max(ends[1]))?)
    }

    fn lift(self, lift: [i32; 2]) -> [i32; 2] {
        std::array::from_fn(|axis| {
            if self.sign[axis] < 0.0 {
                -lift[axis]
            } else {
                lift[axis]
            }
        })
    }
}

pub fn mirrored(
    input: &BrepEnvelope,
    origin: Point3,
    normal: Point3,
) -> Result<BrepEnvelope, GeometryError> {
    input.validate()?;
    let reflection = Reflection {
        origin: checked(origin)?,
        normal: unit(normal)?,
    };
    let mut output = input.clone();
    output.revision = input
        .revision
        .checked_add(1)
        .ok_or_else(|| GeometryError::LimitExceeded("BRep revision overflow".into()))?;
    reflect_surfaces(&mut output.geometry.surfaces, reflection);
    reflect_curves(&mut output.geometry.curves, reflection);
    for vertex in &mut output.topology.vertices {
        vertex.position = reflection.point(vertex.position);
    }
    remap_pcurves(input, &mut output);
    remap_trims(&mut output)?;
    remap_intersections(
        &mut output.geometry.intersections,
        &input.geometry.surfaces,
        reflection,
    )?;
    reverse_loops(&mut output);
    toggle_reversed_provenance(&mut output);
    output.validate()?;
    check_coordinate_precision(
        &output,
        "mirrored coordinate precision exceeds geometric tolerance",
    )?;
    Ok(output)
}

fn reflect_surfaces(surfaces: &mut [SurfaceGeometry], reflection: Reflection) {
    for surface in surfaces {
        match surface {
            SurfaceGeometry::Plane { frame }
            | SurfaceGeometry::Sphere { frame, .. }
            | SurfaceGeometry::Cylinder { frame, .. }
            | SurfaceGeometry::Cone { frame, .. }
            | SurfaceGeometry::Torus { frame, .. } => *frame = reflection.surface_frame(*frame),
        }
    }
}

fn reflect_curves(curves: &mut [CurveGeometry], reflection: Reflection) {
    for curve in curves {
        match curve {
            CurveGeometry::Line { origin, direction } => {
                *origin = reflection.point(*origin);
                *direction = reflection.vector(*direction);
            }
            CurveGeometry::Circle { frame, .. } | CurveGeometry::Ellipse { frame, .. } => {
                *frame = reflection.curve_frame(*frame)
            }
            CurveGeometry::Intersection { .. } => {}
        }
    }
}

fn remap_pcurves(input: &BrepEnvelope, output: &mut BrepEnvelope) {
    let mut maps = vec![None; input.geometry.pcurves.len()];
    for use_ in &mut output.topology.halfedges {
        let (Some(face), Some(id)) = (use_.face, use_.geometry_use.pcurve) else {
            continue;
        };
        let surface = input.topology.faces[face as usize].surface;
        let map = ChartMap::of(&input.geometry.surfaces[surface as usize]);
        match maps[id as usize] {
            None => maps[id as usize] = Some(map),
            Some(first) if first != map => {
                let mut copy = input.geometry.pcurves[id as usize].clone();
                remap_pcurve(&mut copy, map);
                use_.geometry_use.pcurve = Some(output.geometry.pcurves.len() as u32);
                output.geometry.pcurves.push(copy);
            }
            _ => {}
        }
    }
    for (id, original) in input.geometry.pcurves.iter().enumerate() {
        let map = match original {
            PcurveGeometry::ProjectedCurve { surface, .. } => {
                Some(ChartMap::of(&input.geometry.surfaces[*surface as usize]))
            }
            _ => maps[id],
        };
        if let Some(map) = map {
            remap_pcurve(&mut output.geometry.pcurves[id], map);
        }
    }
}

fn remap_pcurve(geometry: &mut PcurveGeometry, map: ChartMap) {
    match geometry {
        PcurveGeometry::Line2 { origin, direction } => {
            *origin = map.point(*origin);
            *direction = map.vector(*direction);
        }
        PcurveGeometry::Conic2 {
            origin,
            axis_a,
            axis_b,
        } => {
            *origin = map.point(*origin);
            *axis_a = map.vector(*axis_a);
            *axis_b = map.vector(*axis_b);
        }
        PcurveGeometry::ProjectedCurve {
            uv_hint, uv_rate, ..
        } => {
            *uv_hint = map.point(*uv_hint);
            *uv_rate = map.vector(*uv_rate);
        }
        PcurveGeometry::IntersectionSide { .. } => {}
    }
}

fn remap_trims(output: &mut BrepEnvelope) -> Result<(), GeometryError> {
    let maps: Vec<_> = output.geometry.surfaces.iter().map(ChartMap::of).collect();
    for face in &mut output.topology.faces {
        let map = maps[face.surface as usize];
        for axis in 0..2 {
            face.trim.uv_bounds[axis] = map.interval(axis, face.trim.uv_bounds[axis])?;
        }
    }
    for use_ in &mut output.topology.halfedges {
        if let Some(face) = use_.face {
            let map = maps[output.topology.faces[face as usize].surface as usize];
            use_.geometry_use.periodic_lift = map.lift(use_.geometry_use.periodic_lift);
        }
    }
    Ok(())
}

fn remap_intersections(
    intersections: &mut [IntersectionDefinition],
    surfaces: &[SurfaceGeometry],
    reflection: Reflection,
) -> Result<(), GeometryError> {
    for definition in intersections {
        let a = ChartMap::of(&surfaces[definition.surfaces[0] as usize]);
        let b = ChartMap::of(&surfaces[definition.surfaces[1] as usize]);
        for anchor in &mut definition.anchors {
            anchor.point = reflection.point(anchor.point);
            anchor.uv_a = a.point(anchor.uv_a);
            anchor.uv_b = b.point(anchor.uv_b);
        }
        for tube in &mut definition.uv_tubes {
            for axis in 0..2 {
                tube[axis] = a.interval(axis, tube[axis])?;
                tube[axis + 2] = b.interval(axis, tube[axis + 2])?;
            }
        }
    }
    Ok(())
}

fn toggle_reversed_provenance(brep: &mut BrepEnvelope) {
    for face in &mut brep.topology.faces {
        face.provenance.reversed = !face.provenance.reversed;
    }
}

fn reverse_loops(brep: &mut BrepEnvelope) {
    for use_ in &mut brep.topology.halfedges {
        if use_.face.is_some() {
            use_.reverse();
        }
    }
    let mut outgoing = vec![None; brep.topology.vertices.len()];
    for use_ in brep.topology.halfedges.iter().rev() {
        outgoing[use_.from as usize] = Some(use_.id);
    }
    for vertex in &mut brep.topology.vertices {
        vertex.outgoing_halfedge = outgoing[vertex.id as usize];
    }
}
