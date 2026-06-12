//! Optional OpenCASCADE cross-check (debt items D1/D9, NFR-4).
//!
//! When a real OCC binding (pythonocc `OCC.Core` or `OCP`) is available on the
//! machine, this exports a cylinder to STEP, reads it back through OCC, and
//! asserts OCC sees a genuine `Geom_CylindricalSurface` — the strongest form of
//! NFR-4. When OCC is not installed (the common case, and always in WASM/CI
//! without it), the test prints a skip notice and passes, so it never blocks the
//! build. The always-on guarantee lives in `tests/step_roundtrip_oracle.rs`.

#![cfg(not(target_arch = "wasm32"))]

use std::process::Command;

use opengeometry::export::step::{export_brep_to_step_text, StepExportConfig};
use opengeometry::primitives::cylinder::OGCylinder;
use openmaths::Vector3;

const PY_CHECK: &str = r#"
import sys
step_path = sys.argv[1]
try:
    try:
        from OCC.Core.STEPControl import STEPControl_Reader
        from OCC.Core.TopExp import TopExp_Explorer
        from OCC.Core.TopAbs import TopAbs_FACE
        from OCC.Core.BRep import BRep_Tool
        from OCC.Core.TopoDS import topods
        from OCC.Core.GeomAbs import GeomAbs_Cylinder
        from OCC.Core.BRepAdaptor import BRepAdaptor_Surface
        backend = "pythonocc"
    except ImportError:
        from OCP.STEPControl import STEPControl_Reader
        from OCP.TopExp import TopExp_Explorer
        from OCP.TopAbs import TopAbs_FACE
        from OCP.TopoDS import TopoDS
        from OCP.GeomAbs import GeomAbs_Cylinder
        from OCP.BRepAdaptor import BRepAdaptor_Surface
        backend = "ocp"
except Exception:
    print("SKIP: no OCC binding")
    sys.exit(0)

reader = STEPControl_Reader()
status = reader.ReadFile(step_path)
reader.TransferRoots()
shape = reader.OneShape()

cyl = 0
exp = TopExp_Explorer(shape, TopAbs_FACE)
while exp.More():
    face = exp.Current()
    try:
        ad = BRepAdaptor_Surface(face if backend=="pythonocc" else face)
        if ad.GetType() == GeomAbs_Cylinder:
            cyl += 1
    except Exception:
        pass
    exp.Next()

print("CYLINDERS=%d" % cyl)
sys.exit(0 if cyl >= 1 else 2)
"#;

#[test]
fn occ_reads_exported_cylinder_as_cylindrical_surface() {
    // Build + export a cylinder to a temp STEP file.
    let mut cyl = OGCylinder::new("occ-cyl".into());
    cyl.set_config(
        Vector3::new(0.0, 0.0, 0.0),
        1.0,
        2.0,
        2.0 * std::f64::consts::PI,
        32,
    )
    .unwrap();
    let (text, _) = export_brep_to_step_text(&cyl.world_brep(), &StepExportConfig::default())
        .expect("step export");

    let dir = std::env::temp_dir();
    let step_path = dir.join("opengeometry_occ_oracle.step");
    let py_path = dir.join("opengeometry_occ_oracle.py");
    if std::fs::write(&step_path, &text).is_err() || std::fs::write(&py_path, PY_CHECK).is_err() {
        eprintln!("SKIP: could not write temp files for OCC oracle");
        return;
    }

    let output = match Command::new("python3")
        .arg(&py_path)
        .arg(&step_path)
        .output()
    {
        Ok(o) => o,
        Err(_) => {
            eprintln!("SKIP: python3 not available for OCC oracle");
            return;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("SKIP") {
        eprintln!("SKIP: OCC binding not installed ({})", stdout.trim());
        return;
    }

    assert!(
        stdout.contains("CYLINDERS=") && output.status.success(),
        "OCC did not read a cylindrical surface from the exported STEP: stdout={:?} stderr={:?}",
        stdout,
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!("OCC oracle: {}", stdout.trim());
}
