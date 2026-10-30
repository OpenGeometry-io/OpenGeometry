use super::*;

#[test]
fn builds_deterministic_part21_document() {
    let mut writer = Part21Writer::new("AUTOMOTIVE_DESIGN");
    writer.set_file_name("test");
    let p1 = writer.add_entity("CARTESIAN_POINT('',(0.,0.,0.))");
    let p2 = writer.add_entity("CARTESIAN_POINT('',(1.,0.,0.))");
    writer.add_entity(format!(
        "POLY_LOOP('',({},{},{}))",
        Part21Writer::reference(p1),
        Part21Writer::reference(p2),
        Part21Writer::reference(p1)
    ));

    let text = writer.build().expect("part21 should build");
    assert!(text.starts_with("ISO-10303-21;"));
    assert!(text.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN'));"));
    assert!(text.contains("#1=CARTESIAN_POINT"));
    assert!(text.contains("#2=CARTESIAN_POINT"));
    assert!(text.ends_with("END-ISO-10303-21;\n"));
}

#[test]
fn fails_for_unresolved_reference() {
    let mut writer = Part21Writer::new("IFC4");
    writer.add_entity("IFCPROJECT('x',#999,$,$,$,$,$,$)");
    let err = writer
        .build()
        .expect_err("writer should reject unresolved refs");
    assert!(err.to_string().contains("undefined id #999"));
}

#[test]
fn sanitizes_non_ascii_literals() {
    let raw = "A'B\nCø";
    let sanitized = sanitize_string_literal(raw);
    assert_eq!(sanitized, "A''B C\\X2\\00F8\\X0\\");
}

#[test]
fn ignores_reference_like_text_inside_escaped_literals() {
    let mut writer = Part21Writer::new("AUTOMOTIVE_DESIGN");
    let point = writer.add_entity("CARTESIAN_POINT('body #999''s point',(0.,0.,0.))");
    writer.add_entity(format!("VERTEX_POINT('#888',#{point})"));
    assert!(writer.build().is_ok());
    assert_eq!(
        extract_references("X('a #999''b',#12,'#88',#3)"),
        vec![12, 3]
    );
}
