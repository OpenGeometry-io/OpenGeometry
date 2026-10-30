use super::cells::{flat_index, neighbor, GridArrangement};
use crate::brep::{
    boundary, padded_uv_bounds, unit, uv_line, BrepEnvelope, Builder, CurveGeometry, EdgeGeometry,
    FaceProvenance, FaceRole, FaceSource, Frame3, GeometryError, Orientation, SurfaceGeometry, Use,
};
use crate::math::{dot, norm, scale, sub, Interval, Point3};
use crate::operations::modifying::boolean::operands::{
    containing_faces, source, BoxInput, GridPoint,
};
use crate::operations::modifying::boolean::types::BooleanOp;
use std::collections::BTreeMap;

struct GridBoundary {
    builder: Builder,
    vertex_ids: BTreeMap<(usize, GridPoint), u32>,
    edge_ids: BTreeMap<(usize, GridPoint, GridPoint), u32>,
    face_components: Vec<usize>,
    volumes: Vec<f64>,
}

pub(super) struct BoundaryFaces {
    pub(super) brep: BrepEnvelope,
    pub(super) face_components: Vec<usize>,
    pub(super) volumes: Vec<f64>,
}

struct FaceOwner<'a> {
    input: &'a BoxInput<'a>,
    source_face: usize,
    reversed: bool,
    cutter: bool,
    lo: Point3,
    hi: Point3,
}

pub(super) fn boundary_faces(
    arrangement: &GridArrangement<'_>,
    material: &[bool],
    components: &[usize],
    id: String,
) -> Result<BoundaryFaces, GeometryError> {
    let &GridArrangement { n, accuracy, .. } = arrangement;
    let mut state = GridBoundary {
        builder: Builder::new(id, accuracy)?,
        vertex_ids: BTreeMap::new(),
        edge_ids: BTreeMap::new(),
        face_components: Vec::new(),
        volumes: Vec::new(),
    };
    for x in 0..n[0] {
        for y in 0..n[1] {
            for z in 0..n[2] {
                let p = [x, y, z];
                let cell = flat_index(p, n);
                if !material[cell] {
                    continue;
                }
                let component = components[cell];
                for axis in 0..3 {
                    for upper in [false, true] {
                        if neighbor(p, axis, upper, n).is_some_and(|q| material[flat_index(q, n)]) {
                            continue;
                        }
                        add_boundary_face(&mut state, arrangement, p, axis, upper, component)?;
                    }
                }
            }
        }
    }
    Ok(BoundaryFaces {
        brep: state.builder.brep,
        face_components: state.face_components,
        volumes: state.volumes,
    })
}

fn add_boundary_face(
    state: &mut GridBoundary,
    arrangement: &GridArrangement<'_>,
    p: GridPoint,
    axis: usize,
    upper: bool,
    component: usize,
) -> Result<(), GeometryError> {
    let &GridArrangement {
        a,
        grid,
        canonical_boxes,
        operation,
        ..
    } = arrangement;
    let coordinate = grid[axis][p[axis] + usize::from(upper)];
    let mut center: Point3 =
        std::array::from_fn(|i| grid[i][p[i]] + (grid[i][p[i] + 1] - grid[i][p[i]]) / 2.0);
    center[axis] = coordinate;
    let mut sources = Vec::new();
    let normal_sign = if upper { 1.0 } else { -1.0 };
    let FaceOwner {
        input,
        source_face,
        reversed,
        cutter,
        lo,
        hi,
    } = face_owner(
        &mut sources,
        arrangement,
        axis,
        upper,
        coordinate,
        center,
        normal_sign,
    )?;
    let surface = input.brep.geometry.surfaces
        [input.brep.topology.faces[source_face].surface as usize]
        .clone();
    let face_frame = *surface.frame();
    let u = (axis + 1) % 3;
    let v = (axis + 2) % 3;
    let corners = face_corners(p, axis, upper, u, v);
    let (uses, bounds) = face_uses(state, arrangement, component, corners, face_frame)?;
    let face = add_side_face(state, input, source_face, surface, bounds, uses)?;
    let whole = canonical_boxes
        && [u, v]
            .into_iter()
            .all(|i| grid[i][p[i]] == lo[i] && grid[i][p[i] + 1] == hi[i]);
    let surface_reversed = if canonical_boxes {
        reversed
    } else {
        dot(
            face_frame.z,
            scale([a.frame.x, a.frame.y, a.frame.z][axis], normal_sign),
        ) < 0.0
    };
    state.builder.brep.topology.faces[face].sense = if surface_reversed {
        Orientation::Reverse
    } else {
        Orientation::Forward
    };
    state.builder.brep.topology.faces[face].provenance =
        side_provenance(sources, reversed, cutter, operation, whole);
    state.face_components.push(component);
    state.volumes.push(
        normal_sign
            * coordinate
            * (grid[u][p[u] + 1] - grid[u][p[u]])
            * (grid[v][p[v] + 1] - grid[v][p[v]])
            / 3.0,
    );
    Ok(())
}

fn face_owner<'a>(
    sources: &mut Vec<FaceSource>,
    arrangement: &GridArrangement<'a>,
    axis: usize,
    upper: bool,
    coordinate: f64,
    center: Point3,
    normal_sign: f64,
) -> Result<FaceOwner<'a>, GeometryError> {
    let &GridArrangement {
        a,
        b,
        alo,
        ahi,
        blo,
        bhi,
        canonical_boxes,
        operation,
        accuracy,
        ..
    } = arrangement;
    let mut owner = None;
    for (input, lo, hi, cutter) in [(a, alo, ahi, false), (b, blo, bhi, true)] {
        let faces = if canonical_boxes {
            [false, true]
                .into_iter()
                .filter_map(|side| {
                    let plane = if side { hi[axis] } else { lo[axis] };
                    (coordinate == plane
                        && (0..3)
                            .filter(|i| *i != axis)
                            .all(|i| center[i] > lo[i] && center[i] < hi[i]))
                    .then_some((face_index(axis, side), side != upper))
                })
                .collect::<Vec<_>>()
        } else {
            containing_faces(
                input,
                a.frame.point(center),
                scale([a.frame.x, a.frame.y, a.frame.z][axis], normal_sign),
                accuracy.intersection,
            )?
        };
        for (face, reversed) in faces {
            if !reversed || (operation == BooleanOp::Subtraction && cutter) {
                sources.push(source(input, face));
                if owner.is_none() {
                    owner = Some((input, face, reversed, cutter, lo, hi));
                }
            }
        }
    }
    let (input, source_face, reversed, cutter, lo, hi) = owner.ok_or_else(|| {
        GeometryError::InvalidTopology("cuboid boundary has no input-face ancestry".into())
    })?;
    Ok(FaceOwner {
        input,
        source_face,
        reversed,
        cutter,
        lo,
        hi,
    })
}

fn face_index(axis: usize, upper: bool) -> usize {
    match axis {
        0 => 2 + usize::from(upper),
        1 => 4 + usize::from(upper),
        _ => usize::from(upper),
    }
}

fn face_corners(p: GridPoint, axis: usize, upper: bool, u: usize, v: usize) -> [GridPoint; 4] {
    let mut corners = [[0; 3]; 4];
    for (i, (du, dv)) in [(0, 0), (1, 0), (1, 1), (0, 1)].into_iter().enumerate() {
        corners[i] = p;
        corners[i][axis] += usize::from(upper);
        corners[i][u] += du;
        corners[i][v] += dv;
    }
    if !upper {
        corners.reverse();
    }
    corners
}

fn face_uses(
    state: &mut GridBoundary,
    arrangement: &GridArrangement<'_>,
    component: usize,
    corners: [GridPoint; 4],
    face_frame: Frame3,
) -> Result<(Vec<Use>, [[f64; 2]; 2]), GeometryError> {
    let &GridArrangement {
        a, grid, accuracy, ..
    } = arrangement;
    let local = corners.map(|q| std::array::from_fn(|i| grid[i][q[i]]));
    let points = local.map(|q| a.frame.point(q));
    let mut vids = [0; 4];
    for i in 0..4 {
        vids[i] = *state
            .vertex_ids
            .entry((component, corners[i]))
            .or_insert_with(|| state.builder.vertex(points[i]));
    }
    let mut uses = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        let (from, to) = if corners[i] < corners[j] {
            (i, j)
        } else {
            (j, i)
        };
        let key = (component, corners[from], corners[to]);
        let edge = grid_edge(state, key, points, from, to)?;
        let EdgeGeometry::Curve { curve, .. } =
            state.builder.brep.topology.edges[edge as usize].geometry
        else {
            return Err(GeometryError::InvalidTopology(
                "cuboid edge is not a line".into(),
            ));
        };
        let CurveGeometry::Line { origin, direction } =
            state.builder.brep.geometry.curves[curve as usize]
        else {
            return Err(GeometryError::InvalidTopology(
                "cuboid curve is not a line".into(),
            ));
        };
        let uv = face_frame.local(origin);
        uses.push(boundary(
            edge,
            vids[i],
            vids[j],
            if i == from {
                Orientation::Forward
            } else {
                Orientation::Reverse
            },
            uv_line(
                [uv[0], uv[1]],
                [dot(direction, face_frame.x), dot(direction, face_frame.y)],
            ),
        ));
    }
    let bounds = padded_uv_bounds(
        points.map(|point| face_frame.local(point)),
        accuracy.geometric,
    );
    Ok((uses, bounds))
}

fn grid_edge(
    state: &mut GridBoundary,
    key: (usize, GridPoint, GridPoint),
    points: [Point3; 4],
    from: usize,
    to: usize,
) -> Result<u32, GeometryError> {
    let edge = if let Some(&edge) = state.edge_ids.get(&key) {
        edge
    } else {
        let delta = sub(points[to], points[from]);
        let length = norm(delta);
        let edge = state.builder.edge(
            CurveGeometry::Line {
                origin: points[from],
                direction: unit(delta)?,
            },
            Interval::new(0.0, length)?,
            false,
        );
        state.edge_ids.insert(key, edge);
        edge
    };
    Ok(edge)
}

fn add_side_face(
    state: &mut GridBoundary,
    input: &BoxInput<'_>,
    source_face: usize,
    surface: SurfaceGeometry,
    bounds: [[f64; 2]; 2],
    uses: Vec<Use>,
) -> Result<usize, GeometryError> {
    let face = state.builder.brep.topology.faces.len();
    state
        .builder
        .face(
            &format!("{}-{face}", input.brep.topology.faces[source_face].key),
            surface,
            bounds,
            uses,
        )
        .map_err(|error| match error {
            GeometryError::InvalidTopology(message) if message == "nonmanifold primitive edge" => {
                GeometryError::UnresolvedIntersection(
                    "cuboid boundary contains an unresolved contact event".into(),
                )
            }
            other => other,
        })?;
    Ok(face)
}

fn side_provenance(
    sources: Vec<FaceSource>,
    reversed: bool,
    cutter: bool,
    operation: BooleanOp,
    whole: bool,
) -> FaceProvenance {
    FaceProvenance {
        role: if sources.len() > 1 {
            FaceRole::Coincident
        } else if cutter && operation == BooleanOp::Subtraction {
            FaceRole::Cut
        } else if whole {
            FaceRole::Preserved
        } else {
            FaceRole::Split
        },
        sources,
        reversed,
    }
}
