use std::f64::consts::TAU;

pub(super) const UNCERTAINTY: f64 = 1e-3;

const HEADER: &str = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('OpenGeometry analytic BRep'),'2;1');\nFILE_NAME('crafted','1970-01-01T00:00:00',('OpenGeometry'),('OpenGeometry'),'OpenGeometry','OpenGeometry','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN'));\nENDSEC;\nDATA;\n";

pub(super) struct Base {
    pub(super) text: String,
    pub(super) entities: usize,
    pub(super) cylinder: Cylinder,
    pub(super) lens: EllipseLens,
    pub(super) spline: SplineLens,
    pub(super) oriented_shell: usize,
    pub(super) solids: [usize; 2],
    pub(super) context: Context,
    pub(super) representations: [usize; 2],
    pub(super) products: Products,
}

pub(super) struct Rim {
    pub(super) circle: usize,
    pub(super) conic: usize,
    pub(super) definition: usize,
    pub(super) edge: usize,
}

pub(super) struct Cylinder {
    pub(super) shell: usize,
    pub(super) lower: Rim,
    pub(super) upper: Rim,
    pub(super) lower_cap_normal: usize,
    pub(super) seam_origin: usize,
    pub(super) seam_line: usize,
    pub(super) seam: usize,
}

pub(super) struct EllipseLens {
    pub(super) shell: usize,
    pub(super) ellipse: usize,
    pub(super) vertex_point: usize,
    pub(super) edge: usize,
    pub(super) faces: [usize; 2],
}

pub(super) struct SplineLens {
    pub(super) shell: usize,
    pub(super) curve: usize,
    pub(super) edge: usize,
}

pub(super) struct Context {
    pub(super) length: usize,
    pub(super) angle: usize,
    pub(super) solid_angle: usize,
    pub(super) uncertainty: usize,
    pub(super) context3: usize,
}

pub(super) struct Products {
    pub(super) application: usize,
    pub(super) product_context: usize,
    pub(super) products: [usize; 2],
}

struct Writer {
    lines: Vec<String>,
}

pub(super) fn real(value: f64) -> String {
    format!("{value:.17E}")
}

fn reals(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| real(*value))
        .collect::<Vec<_>>()
        .join(",")
}

fn refs(ids: &[usize]) -> String {
    ids.iter()
        .map(|id| format!("#{id}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn flag(value: bool) -> &'static str {
    if value {
        ".T."
    } else {
        ".F."
    }
}

impl Writer {
    fn add(&mut self, expression: impl Into<String>) -> usize {
        self.lines.push(expression.into());
        self.lines.len()
    }

    fn point(&mut self, values: &[f64]) -> usize {
        self.add(format!("CARTESIAN_POINT('',({}))", reals(values)))
    }

    fn direction(&mut self, values: &[f64]) -> usize {
        self.add(format!("DIRECTION('',({}))", reals(values)))
    }

    fn placement(&mut self, origin: [f64; 3], z: [f64; 3], x: [f64; 3]) -> usize {
        let origin = self.point(&origin);
        let z = self.direction(&z);
        let x = self.direction(&x);
        self.add(format!("AXIS2_PLACEMENT_3D('',#{origin},#{z},#{x})"))
    }

    fn plane(&mut self, origin: [f64; 3]) -> usize {
        let axis = self.placement(origin, [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
        self.add(format!("PLANE('',#{axis})"))
    }

    fn line(&mut self, origin: usize, direction: &[f64]) -> usize {
        let direction = self.direction(direction);
        let vector = self.add(format!("VECTOR('',#{direction},{})", real(1.0)));
        self.add(format!("LINE('',#{origin},#{vector})"))
    }

    fn line_at(&mut self, origin: &[f64], direction: &[f64]) -> usize {
        let origin = self.point(origin);
        self.line(origin, direction)
    }

    fn pcurve(&mut self, surface: usize, curve: usize) -> usize {
        let definition = self.add(format!("DEFINITIONAL_REPRESENTATION('',(#{curve}),#1)"));
        self.add(format!("PCURVE('',#{surface},#{definition})"))
    }

    fn vertex(&mut self, position: &[f64]) -> usize {
        let point = self.point(position);
        self.add(format!("VERTEX_POINT('',#{point})"))
    }

    fn edge(&mut self, ends: [usize; 2], support: usize) -> usize {
        self.add(format!(
            "EDGE_CURVE('',#{},#{},#{support},.T.)",
            ends[0], ends[1]
        ))
    }

    fn face(&mut self, uses: &[(usize, bool)], surface: usize, sense: bool) -> usize {
        let oriented = uses
            .iter()
            .map(|(edge, forward)| {
                self.add(format!("ORIENTED_EDGE('',*,*,#{edge},{})", flag(*forward)))
            })
            .collect::<Vec<_>>();
        let edge_loop = self.add(format!("EDGE_LOOP('',({}))", refs(&oriented)));
        let bound = self.add(format!("FACE_OUTER_BOUND('',#{edge_loop},.T.)"));
        self.add(format!(
            "ADVANCED_FACE('',(#{bound}),#{surface},{})",
            flag(sense)
        ))
    }

    fn shell(&mut self, faces: &[usize]) -> usize {
        self.add(format!("CLOSED_SHELL('',({}))", refs(faces)))
    }

    fn rim(&mut self, lateral: usize, cap: usize, height: f64, vertex: usize) -> Rim {
        let axis = self.placement([0.0, height, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
        let circle = self.add(format!("CIRCLE('',#{axis},{})", real(1.0)));
        let line = self.line_at(&[0.0, height], &[1.0, 0.0]);
        let on_lateral = self.pcurve(lateral, line);
        let origin = self.point(&[0.0, 0.0]);
        let x = self.direction(&[1.0, 0.0]);
        let placement = self.add(format!("AXIS2_PLACEMENT_2D('',#{origin},#{x})"));
        let conic = self.add(format!("CIRCLE('',#{placement},{})", real(1.0)));
        let definition = self.add(format!("DEFINITIONAL_REPRESENTATION('',(#{conic}),#1)"));
        let on_cap = self.add(format!("PCURVE('',#{cap},#{definition})"));
        let support = self.add(format!(
            "SURFACE_CURVE('',#{circle},(#{on_lateral},#{on_cap}),.CURVE_3D.)"
        ));
        let edge = self.edge([vertex, vertex], support);
        Rim {
            circle,
            conic,
            definition,
            edge,
        }
    }
}

pub(super) fn base() -> Base {
    let mut writer = Writer { lines: Vec::new() };
    writer.add("(GEOMETRIC_REPRESENTATION_CONTEXT(2) REPRESENTATION_CONTEXT('',''))");
    let cylinder = add_cylinder(&mut writer);
    let lens = add_ellipse_lens(&mut writer);
    let oriented_shell = writer.add(format!("ORIENTED_CLOSED_SHELL('',*,#{},.T.)", lens.shell));
    let solid_a = writer.add(format!(
        "BREP_WITH_VOIDS('a-0',#{},(#{oriented_shell}))",
        cylinder.shell
    ));
    let spline = add_spline_lens(&mut writer);
    let solid_b = writer.add(format!("MANIFOLD_SOLID_BREP('b-0',#{})", spline.shell));
    let context = add_context(&mut writer);
    let representations = [solid_a, solid_b].map(|solid| {
        writer.add(format!(
            "ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{solid}),#{})",
            context.context3
        ))
    });
    let products = add_products(&mut writer, representations);
    let mut text = HEADER.to_string();
    for (index, line) in writer.lines.iter().enumerate() {
        text.push_str(&format!("#{}={line};\n", index + 1));
    }
    text.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Base {
        text,
        entities: writer.lines.len(),
        cylinder,
        lens,
        spline,
        oriented_shell,
        solids: [solid_a, solid_b],
        context,
        representations,
        products,
    }
}

fn add_cylinder(writer: &mut Writer) -> Cylinder {
    let axis = writer.placement([0.0; 3], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
    let lateral = writer.add(format!("CYLINDRICAL_SURFACE('',#{axis},{})", real(1.0)));
    let lower_origin = writer.point(&[0.0; 3]);
    let lower_cap_normal = writer.direction(&[-0.0, -1.0, -0.0]);
    let lower_x = writer.direction(&[1.0, 0.0, 0.0]);
    let lower_axis = writer.add(format!(
        "AXIS2_PLACEMENT_3D('',#{lower_origin},#{lower_cap_normal},#{lower_x})"
    ));
    let lower_cap = writer.add(format!("PLANE('',#{lower_axis})"));
    let upper_axis = writer.placement([0.0, 2.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
    let upper_cap = writer.add(format!("PLANE('',#{upper_axis})"));
    let bottom = writer.vertex(&[1.0, 0.0, 0.0]);
    let top = writer.vertex(&[1.0, 2.0, 0.0]);
    let lower = writer.rim(lateral, lower_cap, 0.0, bottom);
    let upper = writer.rim(lateral, upper_cap, 2.0, top);
    let seam_origin = writer.point(&[1.0, 0.0, 0.0]);
    let seam_line = writer.line(seam_origin, &[0.0, 1.0, 0.0]);
    let after = writer.line_at(&[TAU, 0.0], &[0.0, 1.0]);
    let before = writer.line_at(&[0.0, 0.0], &[0.0, 1.0]);
    let pcurves = [
        writer.pcurve(lateral, after),
        writer.pcurve(lateral, before),
    ];
    let support = writer.add(format!(
        "SEAM_CURVE('',#{seam_line},({}),.CURVE_3D.)",
        refs(&pcurves)
    ));
    let seam = writer.edge([bottom, top], support);
    let faces = [
        writer.face(
            &[
                (lower.edge, true),
                (seam, true),
                (upper.edge, false),
                (seam, false),
            ],
            lateral,
            true,
        ),
        writer.face(&[(lower.edge, false)], lower_cap, true),
        writer.face(&[(upper.edge, true)], upper_cap, true),
    ];
    Cylinder {
        shell: writer.shell(&faces),
        lower,
        upper,
        lower_cap_normal,
        seam_origin,
        seam_line,
        seam,
    }
}

fn add_ellipse_lens(writer: &mut Writer) -> EllipseLens {
    let plane = writer.plane([10.0, 0.0, 0.0]);
    let axis = writer.placement([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0]);
    let ellipse = writer.add(format!("ELLIPSE('',#{axis},{},{})", real(2.0), real(1.0)));
    let vertex_point = writer.point(&[12.0, 0.0, 0.0]);
    let vertex = writer.add(format!("VERTEX_POINT('',#{vertex_point})"));
    let edge = writer.edge([vertex, vertex], ellipse);
    let faces = [
        writer.face(&[(edge, true)], plane, true),
        writer.face(&[(edge, false)], plane, false),
    ];
    EllipseLens {
        shell: writer.shell(&faces),
        ellipse,
        vertex_point,
        edge,
        faces,
    }
}

fn add_spline_lens(writer: &mut Writer) -> SplineLens {
    let plane = writer.plane([20.0, 0.0, 0.0]);
    let controls = [
        [20.0, 0.0, 0.0],
        [21.0, 1.0, 0.0],
        [19.0, 1.0, 0.0],
        [20.0, 0.0, 0.0],
    ]
    .map(|control| writer.point(&control));
    let curve = writer.add(format!(
        "B_SPLINE_CURVE_WITH_KNOTS('',3,({}),.UNSPECIFIED.,.F.,.F.,(4,4),({}),.UNSPECIFIED.)",
        refs(&controls),
        reals(&[0.0, 1.0])
    ));
    let vertex = writer.vertex(&[20.0, 0.0, 0.0]);
    let edge = writer.edge([vertex, vertex], curve);
    let faces = [
        writer.face(&[(edge, true)], plane, true),
        writer.face(&[(edge, false)], plane, false),
    ];
    SplineLens {
        shell: writer.shell(&faces),
        curve,
        edge,
    }
}

fn add_context(writer: &mut Writer) -> Context {
    let length = writer.add("(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT($,.METRE.))");
    let angle = writer.add("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))");
    let solid_angle = writer.add("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())");
    let uncertainty = writer.add(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({}),#{length},'distance_accuracy_value','exchange error bound')",
        real(UNCERTAINTY)
    ));
    let context3 = writer.add(format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid_angle})) REPRESENTATION_CONTEXT('',''))"
    ));
    Context {
        length,
        angle,
        solid_angle,
        uncertainty,
        context3,
    }
}

fn add_products(writer: &mut Writer, representations: [usize; 2]) -> Products {
    let application = writer.add("APPLICATION_CONTEXT('automotive design')");
    writer.add(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{application})"
    ));
    let product_context = writer.add(format!("PRODUCT_CONTEXT('',#{application},'mechanical')"));
    let mut products = [0; 2];
    let mut definition_context = None;
    for (index, name) in ["a", "b"].into_iter().enumerate() {
        let product = writer.add(format!(
            "PRODUCT('{name}','{name}','',(#{product_context}))"
        ));
        let formation = writer.add(format!(
            "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE('1','',#{product},.NOT_KNOWN.)"
        ));
        let context = *definition_context.get_or_insert_with(|| {
            writer.add(format!(
                "PRODUCT_DEFINITION_CONTEXT('part definition',#{application},'design')"
            ))
        });
        let definition = writer.add(format!("PRODUCT_DEFINITION('','',#{formation},#{context})"));
        let shape = writer.add(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
        writer.add(format!(
            "SHAPE_DEFINITION_REPRESENTATION(#{shape},#{})",
            representations[index]
        ));
        products[index] = product;
    }
    Products {
        application,
        product_context,
        products,
    }
}
