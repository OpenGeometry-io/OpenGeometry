use super::body::BodyEmission;
use super::entities::{real, refs};
use crate::exchange::part21::{sanitize_string_literal, Part21Writer};

pub(super) fn emit_context(writer: &mut Part21Writer, unit: &str, bound: f64, scale: f64) -> usize {
    let prefix = if unit == "millimetre" { ".MILLI." } else { "$" };
    let length = writer.add_entity(format!(
        "(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT({prefix},.METRE.))"
    ));
    let angle = writer.add_entity("(NAMED_UNIT(*) PLANE_ANGLE_UNIT() SI_UNIT($,.RADIAN.))");
    let solid_angle =
        writer.add_entity("(NAMED_UNIT(*) SI_UNIT($,.STERADIAN.) SOLID_ANGLE_UNIT())");
    let uncertainty = writer.add_entity(format!(
        "UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({}),#{length},'distance_accuracy_value','exchange error bound')",
        real(bound * scale)
    ));
    writer.add_entity(format!(
        "(GEOMETRIC_REPRESENTATION_CONTEXT(3) GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{uncertainty})) GLOBAL_UNIT_ASSIGNED_CONTEXT((#{length},#{angle},#{solid_angle})) REPRESENTATION_CONTEXT('',''))"
    ))
}

pub(super) fn emit_representations(
    writer: &mut Part21Writer,
    bodies: &[BodyEmission],
    context: usize,
) -> Vec<usize> {
    bodies
        .iter()
        .map(|body| {
            writer.add_entity(format!(
                "ADVANCED_BREP_SHAPE_REPRESENTATION('',({}),#{context})",
                refs(&body.solids)
            ))
        })
        .collect()
}

pub(super) fn emit_products(writer: &mut Part21Writer, names: &[&str], representations: &[usize]) {
    let app = writer.add_entity("APPLICATION_CONTEXT('automotive design')");
    writer.add_entity(format!(
        "APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app})"
    ));
    let product_context = writer.add_entity(format!("PRODUCT_CONTEXT('',#{app},'mechanical')"));
    let mut definition_context = None;
    for (name, representation) in names.iter().zip(representations) {
        let name = sanitize_string_literal(name);
        let product = writer.add_entity(format!(
            "PRODUCT('{name}','{name}','',(#{product_context}))"
        ));
        let formation = writer.add_entity(format!(
            "PRODUCT_DEFINITION_FORMATION_WITH_SPECIFIED_SOURCE('1','',#{product},.NOT_KNOWN.)"
        ));
        let definition_context = *definition_context.get_or_insert_with(|| {
            writer.add_entity(format!(
                "PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design')"
            ))
        });
        let definition = writer.add_entity(format!(
            "PRODUCT_DEFINITION('','',#{formation},#{definition_context})"
        ));
        let shape = writer.add_entity(format!("PRODUCT_DEFINITION_SHAPE('','',#{definition})"));
        writer.add_entity(format!(
            "SHAPE_DEFINITION_REPRESENTATION(#{shape},#{representation})"
        ));
    }
}
