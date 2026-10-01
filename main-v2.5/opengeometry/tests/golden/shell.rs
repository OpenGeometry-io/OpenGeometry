use crate::boolean_case::result_body;
use crate::fixtures::fine;
use crate::json::brep_sha256;
use crate::kernel::{primitives, shell_brep, BrepEnvelope, Frame3, GeometryError};
use crate::planar_shapes::profile_shell;
use crate::profiles::rounded_outer;
use crate::runner::Case;

type Build = fn() -> Result<BrepEnvelope, GeometryError>;

fn cuboid() -> Result<BrepEnvelope, GeometryError> {
    primitives::cuboid("box".into(), Frame3::IDENTITY, [2.0; 3], fine())
}

fn cylinder() -> Result<BrepEnvelope, GeometryError> {
    primitives::cylinder("cylinder".into(), Frame3::IDENTITY, 1.0, 2.0, fine())
}

fn sphere() -> Result<BrepEnvelope, GeometryError> {
    primitives::sphere("sphere".into(), Frame3::IDENTITY, 1.0, fine())
}

fn torus() -> Result<BrepEnvelope, GeometryError> {
    primitives::torus("torus".into(), Frame3::IDENTITY, 3.0, 1.0, fine())
}

fn cone() -> Result<BrepEnvelope, GeometryError> {
    primitives::cone("cone".into(), Frame3::IDENTITY, 2.0, 3.0, fine())
}

fn frustum() -> Result<BrepEnvelope, GeometryError> {
    primitives::frustum("frustum".into(), Frame3::IDENTITY, 2.0, 1.0, 3.0, fine())
}

fn arc_edged() -> Result<BrepEnvelope, GeometryError> {
    primitives::arc_edged_extrusion(
        "arc-edged".into(),
        Frame3::IDENTITY,
        rounded_outer(),
        2.0,
        fine(),
    )
}

const SHELLS: [(&str, Build, f64); 11] = [
    ("box", cuboid, 0.2),
    ("cylinder", cylinder, 0.2),
    ("sphere", sphere, 0.2),
    ("torus", torus, 0.2),
    ("cone", cone, 0.2),
    ("frustum", frustum, 0.2),
    ("profile", profile_shell, 0.2),
    ("sphere-collapsed", sphere, 1.0),
    ("box-below-resolution", cuboid, 1e-9),
    ("profile-collapsed", profile_shell, 0.6),
    ("arc-edged-unsupported", arc_edged, 0.2),
];

fn shell(name: &str, build: Build, thickness: f64) -> Case {
    Case::new(format!("shell.{name}"), move |record| {
        let input = build()?;
        record.section("shell");
        record.field("input", &input.id);
        record.field("input.sha256", brep_sha256(&input));
        record.debug("thickness", thickness);
        result_body(
            record,
            shell_brep(&input, thickness, format!("{}-shell", input.id)),
        );
        Ok(())
    })
}

pub(crate) fn cases() -> Vec<Case> {
    SHELLS
        .iter()
        .map(|(name, build, thickness)| shell(name, *build, *thickness))
        .collect()
}
