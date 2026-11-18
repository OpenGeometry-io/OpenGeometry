use super::boundary::{boundary, plane_boundary, uv_line};
use super::brep_builder::{Builder, Use};
use crate::brep::accuracy::Accuracy;
use crate::brep::frame::Frame3;
use crate::brep::geometry::{CurveGeometry, SurfaceGeometry, Wrap};
use crate::brep::topology::{BrepEnvelope, Orientation};
use crate::math::{Interval, Point3};

pub(crate) struct Sector {
    pub(crate) inner: f64,
    pub(crate) thickness: f64,
    pub(crate) wrap_radius: f64,
    pub(crate) length: f64,
    pub(crate) rise: f64,
    pub(crate) width: f64,
}

pub(crate) const SECTOR: Sector = Sector {
    inner: 2.0,
    thickness: 2.0,
    wrap_radius: 2.8,
    length: 4.0,
    rise: 1.0,
    width: 10.0,
};

const SECTOR_ACCURACY: Accuracy = Accuracy {
    geometric: 1e-6,
    intersection: 1e-7,
    tessellation: 1e-3,
    exchange: 1e-5,
};

impl Sector {
    pub(crate) fn wrap(&self) -> Wrap {
        Wrap {
            frame: Frame3::from_axis([1.0, 2.0, 3.0], [0.2, 1.0, 0.1], [1.0, 0.0, 0.3])
                .expect("sector frame"),
            radius: self.wrap_radius,
            semi_angle: 0.0,
        }
    }

    pub(crate) fn volume(&self) -> f64 {
        let area = self.length * self.width - self.length * self.rise / 2.0;
        let outer = self.inner + self.thickness;
        area * (outer * outer - self.inner * self.inner) / (2.0 * self.wrap_radius)
    }

    fn slant(&self) -> f64 {
        self.length.hypot(self.rise)
    }

    fn levels(&self) -> [f64; 2] {
        [self.inner, self.inner + self.thickness]
    }

    fn corners(&self) -> [[f64; 2]; 4] {
        [
            [0.0, 0.0],
            [self.length, self.rise],
            [self.length, self.width],
            [0.0, self.width],
        ]
    }
}

struct Edges {
    vertices: Vec<[u32; 2]>,
    ab: [u32; 2],
    bc: [u32; 2],
    cd: [u32; 2],
    da: [u32; 2],
    up: Vec<u32>,
}

fn line(b: &mut Builder, origin: Point3, direction: Point3, length: f64) -> u32 {
    b.edge(
        CurveGeometry::Line { origin, direction },
        Interval::new(0.0, length).expect("line range"),
        false,
    )
}

fn level_edges(b: &mut Builder, s: &Sector, z: f64) -> [u32; 4] {
    let w = s.wrap();
    let f = w.frame;
    let d = [s.length / s.slant(), s.rise / s.slant(), 0.0];
    let ab = b.edge(
        CurveGeometry::Wrapped {
            wrap: w,
            curve: Box::new(CurveGeometry::Line {
                origin: [0.0, 0.0, z],
                direction: d,
            }),
        },
        Interval::new(0.0, s.slant()).expect("slant range"),
        false,
    );
    let bc = line(b, w.point([s.length, s.rise, z]), f.z, s.width - s.rise);
    let da = line(b, w.point([0.0, 0.0, z]), f.z, s.width);
    let cd = b.edge(
        CurveGeometry::Circle {
            frame: Frame3 {
                origin: f.point([0.0, 0.0, s.width]),
                ..f
            },
            radius: z,
        },
        Interval::new(0.0, s.length / s.wrap_radius).expect("arc range"),
        false,
    );
    [ab, bc, cd, da]
}

fn edges(b: &mut Builder, s: &Sector) -> Edges {
    let w = s.wrap();
    let vertices = s
        .corners()
        .iter()
        .map(|c| s.levels().map(|z| b.vertex(w.point([c[0], c[1], z]))))
        .collect();
    let [inner, outer] = s.levels().map(|z| level_edges(b, s, z));
    let radial = |x: f64| {
        w.frame
            .vector([(x / s.wrap_radius).cos(), (x / s.wrap_radius).sin(), 0.0])
    };
    let up = s
        .corners()
        .iter()
        .map(|c| line(b, w.point([c[0], c[1], s.inner]), radial(c[0]), s.thickness))
        .collect();
    Edges {
        vertices,
        ab: [inner[0], outer[0]],
        bc: [inner[1], outer[1]],
        cd: [inner[2], outer[2]],
        da: [inner[3], outer[3]],
        up,
    }
}

fn cylinder_faces(b: &mut Builder, s: &Sector, e: &Edges) {
    let (fwd, rev) = (Orientation::Forward, Orientation::Reverse);
    let end = s.length / s.wrap_radius;
    let ab = uv_line(
        [0.0, 0.0],
        [s.length / s.slant() / s.wrap_radius, s.rise / s.slant()],
    );
    let bc = uv_line([end, s.rise], [0.0, 1.0]);
    let cd = uv_line([0.0, s.width], [1.0, 0.0]);
    let da = uv_line([0.0, 0.0], [0.0, 1.0]);
    let v = |corner: usize, level: usize| e.vertices[corner][level];
    let inner = vec![
        boundary(e.da[0], v(0, 0), v(3, 0), fwd, da.clone()),
        boundary(e.cd[0], v(3, 0), v(2, 0), fwd, cd.clone()),
        boundary(e.bc[0], v(2, 0), v(1, 0), rev, bc.clone()),
        boundary(e.ab[0], v(1, 0), v(0, 0), rev, ab.clone()),
    ];
    let outer = vec![
        boundary(e.ab[1], v(0, 1), v(1, 1), fwd, ab),
        boundary(e.bc[1], v(1, 1), v(2, 1), fwd, bc),
        boundary(e.cd[1], v(2, 1), v(3, 1), rev, cd),
        boundary(e.da[1], v(3, 1), v(0, 1), rev, da),
    ];
    let pad = 1e-3;
    for (level, uses) in [inner, outer].into_iter().enumerate() {
        let face = b.brep.topology.faces.len();
        b.face(
            if level == 0 { "inner" } else { "outer" },
            SurfaceGeometry::Cylinder {
                frame: s.wrap().frame,
                radius: s.levels()[level],
            },
            [[-pad, end + pad], [-pad, s.width + pad]],
            uses,
        )
        .expect("cylinder face");
        if level == 0 {
            b.brep.topology.faces[face].sense = rev;
        }
    }
}

fn plane(b: &mut Builder, key: &str, frame: Frame3, uses: &[(u32, u32, u32, Orientation)]) {
    let uses: Vec<Use> = uses
        .iter()
        .map(|&(e, from, to, sense)| {
            plane_boundary(b, frame, e, from, to, sense).expect("plane boundary")
        })
        .collect();
    let big = 4.0 * (SECTOR.inner + SECTOR.thickness + SECTOR.width);
    b.face(
        key,
        SurfaceGeometry::Plane { frame },
        [[-big, big], [-big, big]],
        uses,
    )
    .expect("plane face");
}

fn plane_faces(b: &mut Builder, s: &Sector, e: &Edges) {
    let (fwd, rev) = (Orientation::Forward, Orientation::Reverse);
    let f = s.wrap().frame;
    let v = |corner: usize, level: usize| e.vertices[corner][level];
    let end = s.length / s.wrap_radius;
    let start = Frame3::from_axis(f.origin, f.vector([0.0, -1.0, 0.0]), f.x).expect("start");
    let radial = f.vector([end.cos(), end.sin(), 0.0]);
    let finish =
        Frame3::from_axis(f.origin, f.vector([-end.sin(), end.cos(), 0.0]), radial).expect("end");
    let start_uses = [
        (e.up[0], v(0, 0), v(0, 1), fwd),
        (e.da[1], v(0, 1), v(3, 1), fwd),
        (e.up[3], v(3, 1), v(3, 0), rev),
        (e.da[0], v(3, 0), v(0, 0), rev),
    ];
    plane(b, "start", start, &start_uses);
    let end_uses = [
        (e.bc[0], v(1, 0), v(2, 0), fwd),
        (e.up[2], v(2, 0), v(2, 1), fwd),
        (e.bc[1], v(2, 1), v(1, 1), rev),
        (e.up[1], v(1, 1), v(1, 0), rev),
    ];
    plane(b, "end", finish, &end_uses);
    let far = Frame3 {
        origin: f.point([0.0, 0.0, s.width]),
        ..f
    };
    let uses = [
        (e.cd[0], v(2, 0), v(3, 0), rev),
        (e.up[3], v(3, 0), v(3, 1), fwd),
        (e.cd[1], v(3, 1), v(2, 1), fwd),
        (e.up[2], v(2, 1), v(2, 0), rev),
    ];
    plane(b, "far", far, &uses);
}

fn slanted_face(b: &mut Builder, s: &Sector, e: &Edges) {
    let (fwd, rev) = (Orientation::Forward, Orientation::Reverse);
    let slant = s.slant();
    let [r, top] = s.levels();
    let flat = Frame3 {
        origin: [0.0; 3],
        x: [s.length / slant, s.rise / slant, 0.0],
        y: [0.0, 0.0, 1.0],
        z: [s.rise / slant, -s.length / slant, 0.0],
    };
    let v = |corner: usize, level: usize| e.vertices[corner][level];
    let uses = vec![
        boundary(
            e.ab[0],
            v(0, 0),
            v(1, 0),
            fwd,
            uv_line([0.0, r], [1.0, 0.0]),
        ),
        boundary(
            e.up[1],
            v(1, 0),
            v(1, 1),
            fwd,
            uv_line([slant, r], [0.0, 1.0]),
        ),
        boundary(
            e.ab[1],
            v(1, 1),
            v(0, 1),
            rev,
            uv_line([0.0, top], [1.0, 0.0]),
        ),
        boundary(
            e.up[0],
            v(0, 1),
            v(0, 0),
            rev,
            uv_line([0.0, r], [0.0, 1.0]),
        ),
    ];
    let pad = 1e-3;
    b.face(
        "slant",
        SurfaceGeometry::Wrapped {
            wrap: s.wrap(),
            surface: Box::new(SurfaceGeometry::Plane { frame: flat }),
        },
        [[-pad, slant + pad], [r - pad, top + pad]],
        uses,
    )
    .expect("slanted face");
}

pub(crate) fn wrapped_sector() -> BrepEnvelope {
    let s = &SECTOR;
    let mut b = Builder::new("sector".into(), SECTOR_ACCURACY).expect("builder");
    let e = edges(&mut b, s);
    cylinder_faces(&mut b, s, &e);
    plane_faces(&mut b, s, &e);
    slanted_face(&mut b, s, &e);
    b.finish_solid().expect("wrapped sector")
}
