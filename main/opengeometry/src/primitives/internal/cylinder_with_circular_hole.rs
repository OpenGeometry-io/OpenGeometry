use crate::brep::{
    boundary, dimensions, plane_boundary, uv_line, Accuracy, BrepEnvelope, Builder, CurveGeometry,
    Frame3, GeometryError, Orientation, SurfaceGeometry,
};
use crate::math::{dot, norm, scale, sub, Interval};
use Orientation::{Forward as F, Reverse as R};

struct AnnulusTopology {
    outer_low: u32,
    outer_high: u32,
    inner_low: u32,
    inner_high: u32,
    high_frame: Frame3,
    outer_bottom: u32,
    outer_top: u32,
    inner_bottom: u32,
    inner_top: u32,
    outer_seam: u32,
    inner_seam: u32,
}

pub fn cylinder_with_circular_hole(
    id: String,
    frame: Frame3,
    inner_frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    accuracy: Accuracy,
) -> Result<BrepEnvelope, GeometryError> {
    frame.validate()?;
    inner_frame.validate()?;
    accuracy.validate()?;
    dimensions(&[inner_radius, outer_radius, height])?;
    let inner_offset = sub(inner_frame.origin, frame.origin);
    let axial_offset = dot(inner_offset, frame.z);
    let radial_offset = sub(inner_offset, scale(frame.z, axial_offset));
    if norm(sub(inner_frame.z, frame.z)) > 1e-12 || axial_offset.abs() > accuracy.geometric {
        return Err(GeometryError::InvalidGeometry(
            "circular hole axis must share the outer cylinder span".into(),
        ));
    }
    if norm(radial_offset) + inner_radius >= outer_radius {
        return Err(GeometryError::InvalidGeometry(
            "circular hole must lie strictly inside the outer cylinder".into(),
        ));
    }
    if outer_radius - inner_radius - norm(radial_offset) <= 4.0 * accuracy.geometric
        || height <= 4.0 * accuracy.geometric
    {
        return Err(GeometryError::UnresolvedIntersection(
            "cylindrical shell is below geometric resolution".into(),
        ));
    }

    let mut b = Builder::new(id, accuracy)?;
    let tau = std::f64::consts::TAU;
    let range = Interval::new(0.0, tau)?;
    let topology = add_annulus_topology(
        &mut b,
        frame,
        inner_frame,
        inner_radius,
        outer_radius,
        height,
        range,
    )?;
    add_lateral_faces(
        &mut b,
        &topology,
        frame,
        inner_frame,
        inner_radius,
        outer_radius,
        height,
    )?;
    add_annulus_caps(&mut b, &topology, frame, outer_radius)?;
    b.finish_solid()
}

fn add_annulus_topology(
    b: &mut Builder,
    frame: Frame3,
    inner_frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
    range: Interval,
) -> Result<AnnulusTopology, GeometryError> {
    let outer_low = b.vertex(frame.point([outer_radius, 0.0, 0.0]));
    let outer_high = b.vertex(frame.point([outer_radius, 0.0, height]));
    let inner_low = b.vertex(inner_frame.point([inner_radius, 0.0, 0.0]));
    let inner_high = b.vertex(inner_frame.point([inner_radius, 0.0, height]));
    let low_frame = frame;
    let mut high_frame = frame;
    high_frame.origin = frame.point([0.0, 0.0, height]);
    let mut inner_high_frame = inner_frame;
    inner_high_frame.origin = inner_frame.point([0.0, 0.0, height]);

    let outer_bottom = b.edge(
        CurveGeometry::Circle {
            frame: low_frame,
            radius: outer_radius,
        },
        range,
        false,
    );
    let outer_top = b.edge(
        CurveGeometry::Circle {
            frame: high_frame,
            radius: outer_radius,
        },
        range,
        false,
    );
    let inner_bottom = b.edge(
        CurveGeometry::Circle {
            frame: inner_frame,
            radius: inner_radius,
        },
        range,
        false,
    );
    let inner_top = b.edge(
        CurveGeometry::Circle {
            frame: inner_high_frame,
            radius: inner_radius,
        },
        range,
        false,
    );
    let [outer_seam, inner_seam] =
        add_seam_edges(b, frame, inner_frame, inner_radius, outer_radius, height)?;
    Ok(AnnulusTopology {
        outer_low,
        outer_high,
        inner_low,
        inner_high,
        high_frame,
        outer_bottom,
        outer_top,
        inner_bottom,
        inner_top,
        outer_seam,
        inner_seam,
    })
}

fn add_seam_edges(
    b: &mut Builder,
    frame: Frame3,
    inner_frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
) -> Result<[u32; 2], GeometryError> {
    let outer_seam = b.edge(
        CurveGeometry::Line {
            origin: frame.point([outer_radius, 0.0, 0.0]),
            direction: frame.z,
        },
        Interval::new(0.0, height)?,
        true,
    );
    let inner_seam = b.edge(
        CurveGeometry::Line {
            origin: inner_frame.point([inner_radius, 0.0, 0.0]),
            direction: inner_frame.z,
        },
        Interval::new(0.0, height)?,
        true,
    );
    Ok([outer_seam, inner_seam])
}

fn add_lateral_faces(
    b: &mut Builder,
    topology: &AnnulusTopology,
    frame: Frame3,
    inner_frame: Frame3,
    inner_radius: f64,
    outer_radius: f64,
    height: f64,
) -> Result<(), GeometryError> {
    let tau = std::f64::consts::TAU;
    let &AnnulusTopology {
        outer_low,
        outer_high,
        inner_low,
        inner_high,
        outer_bottom,
        outer_top,
        inner_bottom,
        inner_top,
        outer_seam,
        inner_seam,
        ..
    } = topology;
    let lateral_uses = |bottom, top, seam, low, high| {
        vec![
            boundary(bottom, low, low, F, uv_line([0.0, 0.0], [1.0, 0.0])),
            boundary(seam, low, high, F, uv_line([tau, 0.0], [0.0, 1.0])),
            boundary(top, high, high, R, uv_line([0.0, height], [1.0, 0.0])),
            boundary(seam, high, low, R, uv_line([0.0, 0.0], [0.0, 1.0])),
        ]
    };
    b.face(
        "outer",
        SurfaceGeometry::Cylinder {
            frame,
            radius: outer_radius,
        },
        [[0.0, tau], [0.0, height]],
        lateral_uses(outer_bottom, outer_top, outer_seam, outer_low, outer_high),
    )?;
    b.face(
        "inner",
        SurfaceGeometry::Cylinder {
            frame: inner_frame,
            radius: inner_radius,
        },
        [[0.0, tau], [0.0, height]],
        lateral_uses(inner_bottom, inner_top, inner_seam, inner_low, inner_high),
    )?;
    b.brep.topology.faces[1].sense = R;
    Ok(())
}

fn add_annulus_caps(
    b: &mut Builder,
    topology: &AnnulusTopology,
    frame: Frame3,
    outer_radius: f64,
) -> Result<(), GeometryError> {
    let &AnnulusTopology {
        outer_low,
        outer_high,
        inner_low,
        inner_high,
        high_frame,
        outer_bottom,
        outer_top,
        inner_bottom,
        inner_top,
        ..
    } = topology;
    let mut low_frame = frame;
    low_frame.y = scale(low_frame.y, -1.0);
    low_frame.z = scale(low_frame.z, -1.0);
    let outer_lower = vec![plane_boundary(
        b,
        low_frame,
        outer_bottom,
        outer_low,
        outer_low,
        R,
    )?];
    let inner_lower = vec![plane_boundary(
        b,
        low_frame,
        inner_bottom,
        inner_low,
        inner_low,
        R,
    )?];
    b.face_with_holes(
        "lower_cap",
        SurfaceGeometry::Plane { frame: low_frame },
        [[-outer_radius, outer_radius]; 2],
        outer_lower,
        vec![inner_lower],
    )?;
    let outer_upper = vec![plane_boundary(
        b, high_frame, outer_top, outer_high, outer_high, F,
    )?];
    let inner_upper = vec![plane_boundary(
        b, high_frame, inner_top, inner_high, inner_high, F,
    )?];
    b.face_with_holes(
        "upper_cap",
        SurfaceGeometry::Plane { frame: high_frame },
        [[-outer_radius, outer_radius]; 2],
        outer_upper,
        vec![inner_upper],
    )
}
