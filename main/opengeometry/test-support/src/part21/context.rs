use super::document::Document;
use super::lexer::{fields, keyword, record, records, reference, references};
use std::collections::BTreeMap;

const LINKS: [(&str, usize); 5] = [
    ("APPLICATION_PROTOCOL_DEFINITION", 3),
    ("PRODUCT_CONTEXT", 1),
    ("PRODUCT_DEFINITION_CONTEXT", 1),
    ("ADVANCED_BREP_SHAPE_REPRESENTATION", 2),
    ("DEFINITIONAL_REPRESENTATION", 2),
];

const UNIT_KINDS: [&str; 3] = ["LENGTH_UNIT", "PLANE_ANGLE_UNIT", "SOLID_ANGLE_UNIT"];

struct Link {
    owner: usize,
    kind: &'static str,
    target: Option<usize>,
}

struct ContextEntities {
    applications: Vec<usize>,
    contexts2: Vec<usize>,
    contexts3: Vec<usize>,
    units: BTreeMap<usize, &'static str>,
    assigned_units: BTreeMap<usize, Vec<usize>>,
    assigned_uncertainties: BTreeMap<usize, Vec<usize>>,
    uncertainties: Vec<(usize, Option<usize>)>,
    links: Vec<Link>,
}

impl Document {
    pub(super) fn check_contexts(&self) -> Result<(), String> {
        let entities = self.context_entities()?;
        check_context_presence(&entities)?;
        check_context_links(&entities)?;
        check_units(&entities)
    }

    pub fn length_unit(&self) -> Result<&'static str, String> {
        let length = length_unit_id(&self.context_entities()?)?;
        let unit = record(self.entity(length)?, "SI_UNIT")?
            .ok_or_else(|| format!("length unit #{length} is not an SI_UNIT"))?;
        let args = fields(unit)?;
        match (args[0].as_str(), args.get(1).map(String::as_str)) {
            ("$", Some(".METRE.")) => Ok("metre"),
            (".MILLI.", Some(".METRE.")) => Ok("millimetre"),
            _ => Err(format!("unsupported length unit {unit}")),
        }
    }

    pub fn check_length_unit(&self, unit: &str) -> Result<(), String> {
        let actual = self.length_unit()?;
        if actual != unit {
            return Err(format!("file length unit is {actual}, expected {unit}"));
        }
        Ok(())
    }

    fn context_entities(&self) -> Result<ContextEntities, String> {
        let mut entities = ContextEntities {
            applications: Vec::new(),
            contexts2: Vec::new(),
            contexts3: Vec::new(),
            units: BTreeMap::new(),
            assigned_units: BTreeMap::new(),
            assigned_uncertainties: BTreeMap::new(),
            uncertainties: Vec::new(),
            links: Vec::new(),
        };
        for (id, expression) in &self.entities {
            for record in records(expression)? {
                add_context_record(&mut entities, *id, record)?;
            }
        }
        Ok(entities)
    }
}

fn add_context_record(
    entities: &mut ContextEntities,
    id: usize,
    record: &str,
) -> Result<(), String> {
    let name = keyword(record);
    if let Some((kind, field)) = LINKS.into_iter().find(|(kind, _)| *kind == name) {
        let target = link_target(&fields(record)?, field)?;
        entities.links.push(Link {
            owner: id,
            kind,
            target,
        });
        return Ok(());
    }
    if let Some(kind) = UNIT_KINDS.into_iter().find(|kind| *kind == name) {
        entities.units.insert(id, kind);
        return Ok(());
    }
    match name {
        "APPLICATION_CONTEXT" => entities.applications.push(id),
        "UNCERTAINTY_MEASURE_WITH_UNIT" => {
            let unit = link_target(&fields(record)?, 1)?;
            entities.uncertainties.push((id, unit));
        }
        "GEOMETRIC_REPRESENTATION_CONTEXT" => match fields(record)?[0].as_str() {
            "2" => entities.contexts2.push(id),
            "3" => entities.contexts3.push(id),
            dimension => return Err(format!("#{id} has context dimension {dimension}")),
        },
        "GLOBAL_UNIT_ASSIGNED_CONTEXT" => {
            entities.assigned_units.insert(id, references(record));
        }
        "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT" => {
            entities
                .assigned_uncertainties
                .insert(id, references(record));
        }
        _ => {}
    }
    Ok(())
}

fn link_target(args: &[String], index: usize) -> Result<Option<usize>, String> {
    let field = args.get(index).ok_or("context link lacks its field")?;
    if field == "$" {
        return Ok(None);
    }
    reference(field).map(Some)
}

fn check_context_presence(entities: &ContextEntities) -> Result<(), String> {
    for (kind, ids) in [
        ("APPLICATION_CONTEXT", &entities.applications),
        ("2D GEOMETRIC_REPRESENTATION_CONTEXT", &entities.contexts2),
        ("3D GEOMETRIC_REPRESENTATION_CONTEXT", &entities.contexts3),
    ] {
        if ids.len() != 1 {
            return Err(format!("expected exactly one {kind}, found {}", ids.len()));
        }
    }
    for kind in [
        "APPLICATION_PROTOCOL_DEFINITION",
        "PRODUCT_CONTEXT",
        "PRODUCT_DEFINITION_CONTEXT",
    ] {
        if !entities.links.iter().any(|link| link.kind == kind) {
            return Err(format!("missing {kind}"));
        }
    }
    Ok(())
}

fn check_context_links(entities: &ContextEntities) -> Result<(), String> {
    for link in &entities.links {
        let (expected, name) = match link.kind {
            "ADVANCED_BREP_SHAPE_REPRESENTATION" => (entities.contexts3[0], "the 3D context"),
            "DEFINITIONAL_REPRESENTATION" => (entities.contexts2[0], "the 2D context"),
            _ => (entities.applications[0], "the APPLICATION_CONTEXT"),
        };
        if link.target != Some(expected) {
            return Err(format!(
                "{} #{} does not reference {name}",
                link.kind, link.owner
            ));
        }
    }
    Ok(())
}

fn check_units(entities: &ContextEntities) -> Result<(), String> {
    let length = length_unit_id(entities)?;
    let [(uncertainty, unit)] = entities.uncertainties[..] else {
        return Err(format!(
            "expected exactly one UNCERTAINTY_MEASURE_WITH_UNIT, found {}",
            entities.uncertainties.len()
        ));
    };
    if unit != Some(length) {
        return Err(format!(
            "UNCERTAINTY_MEASURE_WITH_UNIT #{uncertainty} does not name the length unit"
        ));
    }
    let assigned = entities.assigned_uncertainties.get(&entities.contexts3[0]);
    if assigned.map(Vec::as_slice) != Some(&[uncertainty]) {
        return Err(
            "GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT does not reference the UNCERTAINTY_MEASURE_WITH_UNIT"
                .into(),
        );
    }
    Ok(())
}

fn length_unit_id(entities: &ContextEntities) -> Result<usize, String> {
    let context = entities
        .contexts3
        .first()
        .ok_or("missing 3D GEOMETRIC_REPRESENTATION_CONTEXT")?;
    let assigned = entities
        .assigned_units
        .get(context)
        .map(Vec::as_slice)
        .unwrap_or_default();
    let unit_kind = |id: &usize| entities.units.get(id).copied().unwrap_or_default();
    let mut kinds = assigned.iter().map(unit_kind).collect::<Vec<_>>();
    kinds.sort_unstable();
    if kinds != UNIT_KINDS {
        return Err("GLOBAL_UNIT_ASSIGNED_CONTEXT does not name one length, one plane-angle and one solid-angle unit".into());
    }
    assigned
        .iter()
        .copied()
        .find(|id| unit_kind(id) == "LENGTH_UNIT")
        .ok_or_else(|| "missing length unit".into())
}
