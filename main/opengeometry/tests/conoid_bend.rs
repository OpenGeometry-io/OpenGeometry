use opengeometry::brep::{
    conoid_ellipse, mirrored, placed, plane_boundary, projected_boundary, uv_line, BrepEnvelope,
    Builder, CurveGeometry, Frame3, GeometryError, Orientation, SurfaceGeometry, Use,
};
use opengeometry::exchange::export_step;
use opengeometry::math::{Interval, Point3};
use opengeometry::operations::modifying::boolean::{boolean_brep, BooleanOp};
use opengeometry::primitives::cuboid;
use opengeometry::query::{classify_point, PointClassification};
use opengeometry_test_support::accuracy::accuracy;
use opengeometry_test_support::part21::Document;
use opengeometry_test_support::volume::estimate;
use std::f64::consts::FRAC_PI_2;

const RADII: [f64; 2] = [2.0, 4.0];
const ENDS: [f64; 2] = [0.0, FRAC_PI_2];
const PAD: f64 = 1e-3;

struct Shell {
    frame: Frame3,
    c: [f64; 3],
}

fn dot(a: Point3, b: Point3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn combine(a: Point3, s: f64, b: Point3, t: f64) -> Point3 {
    std::array::from_fn(|i| a[i] * s + b[i] * t)
}

impl Shell {
    fn new() -> Self {
        let frame = Frame3::from_axis([1.0, 2.0, 3.0], [1.0, 0.2, 0.1], [0.0, 1.0, 0.3]).unwrap();
        let normal = [1.0, -1.0, 0.0];
        let local = [frame.x, frame.y, frame.z].map(|axis| dot(normal, axis));
        let k = dot(normal, frame.origin) + 5.0 * local[2];
        let c = [
            (k - dot(normal, frame.origin)) / local[2],
            -RADII[0] * local[0] / local[2],
            -RADII[0] * local[1] / local[2],
        ];
        Self { frame, c }
    }

    fn height(&self, u: f64) -> f64 {
        self.c[0] + self.c[1] * u.cos() + self.c[2] * u.sin()
    }

    fn at(&self, u: f64, radius: f64, z: f64) -> Point3 {
        self.frame.point([radius * u.cos(), radius * u.sin(), z])
    }

    fn radial(&self, u: f64) -> Point3 {
        combine(self.frame.x, u.cos(), self.frame.y, u.sin())
    }

    fn tangent(&self, u: f64) -> Point3 {
        combine(self.frame.x, -u.sin(), self.frame.y, u.cos())
    }

    fn volume(&self) -> f64 {
        let n = 64;
        let h = FRAC_PI_2 / n as f64;
        let ring = (RADII[1] * RADII[1] - RADII[0] * RADII[0]) / 2.0;
        let sum: f64 = (0..=n)
            .map(|i| {
                let weight = if i == 0 || i == n {
                    1.0
                } else {
                    2.0 + 2.0 * (i % 2) as f64
                };
                weight * self.height(i as f64 * h)
            })
            .sum();
        ring * sum * h / 3.0
    }
}

struct Edges {
    vertices: [[[u32; 2]; 2]; 2],
    arcs: [u32; 2],
    ellipses: [(u32, f64); 2],
    radials: [u32; 2],
    rulings: [u32; 2],
    axials: [[u32; 2]; 2],
}

fn line(b: &mut Builder, origin: Point3, direction: Point3, length: f64) -> u32 {
    let curve = CurveGeometry::Line { origin, direction };
    b.edge(curve, Interval::new(0.0, length).unwrap(), false)
}

fn edges(b: &mut Builder, s: &Shell) -> Edges {
    let mut vertices = [[[0; 2]; 2]; 2];
    for (e, &u) in ENDS.iter().enumerate() {
        for (r, &radius) in RADII.iter().enumerate() {
            vertices[e][r] = [0.0, s.height(u)].map(|z| b.vertex(s.at(u, radius, z)));
        }
    }
    let quarter = Interval::new(0.0, FRAC_PI_2).unwrap();
    let arcs = RADII.map(|radius| {
        let circle = CurveGeometry::Circle {
            frame: s.frame,
            radius,
        };
        b.edge(circle, quarter, false)
    });
    let ellipses = RADII.map(|radius| {
        let (ellipse, phase) = conoid_ellipse(s.frame, s.c, radius).unwrap();
        let range = Interval::new(-phase, FRAC_PI_2 - phase).unwrap();
        (b.edge(ellipse, range, false), phase)
    });
    let thickness = RADII[1] - RADII[0];
    let radials = ENDS.map(|u| line(b, s.at(u, RADII[0], 0.0), s.radial(u), thickness));
    let rulings = ENDS.map(|u| line(b, s.at(u, RADII[0], s.height(u)), s.radial(u), thickness));
    let axials =
        ENDS.map(|u| RADII.map(|radius| line(b, s.at(u, radius, 0.0), s.frame.z, s.height(u))));
    Edges {
        vertices,
        arcs,
        ellipses,
        radials,
        rulings,
        axials,
    }
}

type Step = (u32, u32, u32, Orientation);

fn plane(b: &mut Builder, key: &str, frame: Frame3, steps: [Step; 4]) {
    let uses = steps
        .map(|(edge, from, to, sense)| plane_boundary(b, frame, edge, from, to, sense).unwrap());
    let big = 100.0;
    let surface = SurfaceGeometry::Plane { frame };
    b.face(key, surface, [[-big, big], [-big, big]], uses.into())
        .unwrap();
}

fn planes(b: &mut Builder, s: &Shell, e: &Edges) {
    let (fwd, rev) = (Orientation::Forward, Orientation::Reverse);
    let v = |end: usize, r: usize, level: usize| e.vertices[end][r][level];
    let f = s.frame;
    let bottom = Frame3::from_axis(f.origin, f.z.map(|x| -x), f.x).unwrap();
    plane(
        b,
        "bottom",
        bottom,
        [
            (e.arcs[0], v(0, 0, 0), v(1, 0, 0), fwd),
            (e.radials[1], v(1, 0, 0), v(1, 1, 0), fwd),
            (e.arcs[1], v(1, 1, 0), v(0, 1, 0), rev),
            (e.radials[0], v(0, 1, 0), v(0, 0, 0), rev),
        ],
    );
    for (end, &u) in ENDS.iter().enumerate() {
        let outward = s.tangent(u).map(|x| if end == 0 { -x } else { x });
        let frame = Frame3::from_axis(f.origin, outward, s.radial(u)).unwrap();
        plane(
            b,
            ["start", "end"][end],
            frame,
            [
                (e.radials[end], v(end, 0, 0), v(end, 1, 0), fwd),
                (e.axials[end][1], v(end, 1, 0), v(end, 1, 1), fwd),
                (e.rulings[end], v(end, 1, 1), v(end, 0, 1), rev),
                (e.axials[end][0], v(end, 0, 1), v(end, 0, 0), rev),
            ],
        );
    }
}

fn cylinders(b: &mut Builder, s: &Shell, e: &Edges) {
    let (fwd, rev) = (Orientation::Forward, Orientation::Reverse);
    let top = s.c[0] + s.c[1].hypot(s.c[2]);
    for (r, &radius) in RADII.iter().enumerate() {
        let v = |end: usize, level: usize| e.vertices[end][r][level];
        let surface = SurfaceGeometry::Cylinder {
            frame: s.frame,
            radius,
        };
        let ellipse = projected_boundary(
            b,
            &surface,
            e.ellipses[r].0,
            v(1, 1),
            v(0, 1),
            rev,
            Some([0.0, s.height(0.0)]),
        )
        .unwrap();
        let uses: Vec<Use> = vec![
            uv_use(e.arcs[r], v(0, 0), v(1, 0), fwd, [0.0, 0.0], [1.0, 0.0]),
            uv_use(
                e.axials[1][r],
                v(1, 0),
                v(1, 1),
                fwd,
                [ENDS[1], 0.0],
                [0.0, 1.0],
            ),
            ellipse,
            uv_use(
                e.axials[0][r],
                v(0, 1),
                v(0, 0),
                rev,
                [0.0, 0.0],
                [0.0, 1.0],
            ),
        ];
        let bounds = [[-PAD, FRAC_PI_2 + PAD], [-PAD, top + PAD]];
        let face = b
            .face(["inner", "outer"][r], surface, bounds, uses)
            .unwrap();
        if r == 0 {
            b.set_face_sense(face, rev).unwrap();
        }
    }
}

fn uv_use(
    edge: u32,
    from: u32,
    to: u32,
    sense: Orientation,
    origin: [f64; 2],
    dir: [f64; 2],
) -> Use {
    opengeometry::brep::boundary(edge, from, to, sense, uv_line(origin, dir))
}

fn conoid_face(b: &mut Builder, s: &Shell, e: &Edges, bounds: [[f64; 2]; 2]) {
    let (fwd, rev) = (Orientation::Forward, Orientation::Reverse);
    let v = |end: usize, r: usize| e.vertices[end][r][1];
    let [(inner, inner_phase), (outer, outer_phase)] = e.ellipses;
    let uses = vec![
        uv_use(
            inner,
            v(0, 0),
            v(1, 0),
            fwd,
            [inner_phase, RADII[0]],
            [1.0, 0.0],
        ),
        uv_use(
            e.rulings[1],
            v(1, 0),
            v(1, 1),
            fwd,
            [ENDS[1], RADII[0]],
            [0.0, 1.0],
        ),
        uv_use(
            outer,
            v(1, 1),
            v(0, 1),
            rev,
            [outer_phase, RADII[1]],
            [1.0, 0.0],
        ),
        uv_use(
            e.rulings[0],
            v(0, 1),
            v(0, 0),
            rev,
            [ENDS[0], RADII[0]],
            [0.0, 1.0],
        ),
    ];
    let surface = SurfaceGeometry::Conoid {
        frame: s.frame,
        c: s.c,
    };
    let face = b.face("cut", surface, bounds, uses).unwrap();
    b.set_face_sense(face, rev).unwrap();
}

fn build(s: &Shell, conoid_bounds: [[f64; 2]; 2]) -> Result<BrepEnvelope, GeometryError> {
    let mut b = Builder::new("conoid-cut".into(), accuracy(1e-5))?;
    let e = edges(&mut b, s);
    planes(&mut b, s, &e);
    cylinders(&mut b, s, &e);
    conoid_face(&mut b, s, &e, conoid_bounds);
    b.finish_solid()
}

fn padded() -> [[f64; 2]; 2] {
    [[-PAD, FRAC_PI_2 + PAD], [RADII[0] - PAD, RADII[1] + PAD]]
}

fn exact() -> [[f64; 2]; 2] {
    [ENDS, RADII]
}

fn check_volume(brep: &BrepEnvelope, exact: f64, deflection: f64, relative: f64) {
    let volume = estimate(brep, deflection);
    let error = (volume.value - exact).abs();
    assert!(error <= volume.error_bound, "{} vs {exact}", volume.value);
    assert!(error <= relative * exact, "{} vs {exact}", volume.value);
}

#[test]
fn a_conoid_cut_shell_is_a_valid_solid_with_its_exact_volume() {
    let s = Shell::new();
    let exact_volume = s.volume();
    let closed_form = 6.0 * (s.c[0] * FRAC_PI_2 + s.c[1] + s.c[2]);
    assert!((exact_volume - closed_form).abs() < 1e-9 * closed_form);
    for (bounds, relative) in [(exact(), 2e-4), (padded(), 2e-4)] {
        let brep = build(&s, bounds).unwrap();
        brep.validate().unwrap();
        assert_eq!(brep.topology.faces.len(), 6);
        assert_eq!(brep.topology.edges.len(), 12);
        check_volume(&brep, exact_volume, 1e-3, relative);
    }
}

#[test]
fn points_on_either_side_of_the_conoid_classify_correctly() {
    let s = Shell::new();
    let brep = build(&s, exact()).unwrap();
    for u in [0.2, FRAC_PI_2 / 2.0, 1.3] {
        let h = s.height(u);
        let classify = |radius: f64, z: f64| classify_point(&brep, s.at(u, radius, z)).unwrap();
        assert_eq!(classify(3.0, h / 2.0), PointClassification::Inside);
        assert_eq!(classify(3.5, h - 0.05), PointClassification::Inside);
        assert_eq!(classify(2.5, h + 0.05), PointClassification::Outside);
        assert_eq!(classify(4.5, h / 2.0), PointClassification::Outside);
    }
}

#[test]
fn a_conoid_cut_shell_exports_to_step_and_round_trips_through_json() {
    let s = Shell::new();
    let brep = build(&s, exact()).unwrap();
    let (step, report) = export_step(&brep, "metre").unwrap();
    assert!(report.exchange_error_bound <= brep.accuracy.exchange);
    let document = Document::parse(&step).unwrap();
    assert_eq!(document.count("ADVANCED_FACE"), 6);
    assert_eq!(document.count("B_SPLINE_SURFACE_WITH_KNOTS"), 1);
    assert_eq!(document.count("CYLINDRICAL_SURFACE"), 2);
    assert_eq!(document.count("SURFACE_CURVE"), 10);
    assert_eq!(document.solids().unwrap().len(), 1);
    let (millimetres, _) = export_step(&brep, "millimetre").unwrap();
    Document::parse(&millimetres).unwrap();
    let json = brep.to_json().unwrap();
    assert!(json.contains("\"kind\":\"Conoid\""));
    let back = BrepEnvelope::from_json(&json).unwrap();
    back.validate().unwrap();
    assert_eq!(back.geometry, brep.geometry);
    assert_eq!(back.to_json().unwrap(), json);
}

#[test]
fn a_conoid_cut_shell_moves_scales_and_mirrors_but_refuses_booleans() {
    let s = Shell::new();
    let brep = build(&s, exact()).unwrap();
    let volume = s.volume();
    let placement = Frame3::from_axis([-4.0, 1.0, 2.0], [0.3, -1.0, 2.0], [1.0, 0.0, 0.0]).unwrap();
    check_volume(
        &placed(&brep, placement, 2.0).unwrap(),
        8.0 * volume,
        2e-3,
        2e-4,
    );
    let mirror = mirrored(&brep, [0.5, 0.0, 1.0], [1.0, 2.0, -0.5]).unwrap();
    check_volume(&mirror, volume, 1e-3, 2e-4);
    let cube = cuboid("cube".into(), Frame3::IDENTITY, [1.0; 3], brep.accuracy).unwrap();
    for (a, b) in [(&brep, &cube), (&cube, &brep)] {
        assert!(matches!(
            boolean_brep(a, b, BooleanOp::Union, "u".into()),
            Err(GeometryError::UnsupportedGeometry(_))
        ));
    }
}
