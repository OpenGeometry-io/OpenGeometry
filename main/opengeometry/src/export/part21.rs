use std::collections::HashSet;

const DEFAULT_PART21_VERSION: &str = "2;1";

#[derive(Clone, Debug)]
pub struct Part21Writer {
    schema: String,
    description: String,
    file_name: String,
    timestamp: String,
    author: String,
    organization: String,
    preprocessor: String,
    originating_system: String,
    authorization: String,
    next_id: usize,
    entries: Vec<(usize, String)>,
}

impl Part21Writer {
    pub fn new(schema: impl Into<String>) -> Self {
        Self {
            schema: sanitize_string_literal(&schema.into()),
            description: "OpenGeometry Export".to_string(),
            file_name: "opengeometry-export".to_string(),
            timestamp: "1970-01-01T00:00:00".to_string(),
            author: "OpenGeometry".to_string(),
            organization: "OpenGeometry".to_string(),
            preprocessor: "OpenGeometry".to_string(),
            originating_system: "OpenGeometry".to_string(),
            authorization: String::new(),
            next_id: 1,
            entries: Vec::new(),
        }
    }

    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = sanitize_string_literal(&description.into());
    }

    pub fn set_file_name(&mut self, file_name: impl Into<String>) {
        self.file_name = sanitize_string_literal(&file_name.into());
    }

    pub fn set_timestamp(&mut self, timestamp: impl Into<String>) {
        self.timestamp = sanitize_string_literal(&timestamp.into());
    }

    pub fn set_author(&mut self, author: impl Into<String>) {
        self.author = sanitize_string_literal(&author.into());
    }

    pub fn set_organization(&mut self, organization: impl Into<String>) {
        self.organization = sanitize_string_literal(&organization.into());
    }

    pub fn set_preprocessor(&mut self, preprocessor: impl Into<String>) {
        self.preprocessor = sanitize_string_literal(&preprocessor.into());
    }

    pub fn set_originating_system(&mut self, originating_system: impl Into<String>) {
        self.originating_system = sanitize_string_literal(&originating_system.into());
    }

    pub fn set_authorization(&mut self, authorization: impl Into<String>) {
        self.authorization = sanitize_string_literal(&authorization.into());
    }

    pub fn add_entity(&mut self, expression: impl Into<String>) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.entries.push((id, expression.into()));
        id
    }

    pub fn reference(id: usize) -> String {
        format!("#{}", id)
    }

    pub fn build(self) -> Result<String, String> {
        if self.entries.is_empty() {
            return Err("Part-21 writer has no DATA entities".to_string());
        }

        let mut defined = HashSet::new();
        for (id, _) in &self.entries {
            if !defined.insert(*id) {
                return Err(format!("Duplicate Part-21 entity id detected: #{}", id));
            }
        }

        for (id, expression) in &self.entries {
            for reference in extract_references(expression) {
                if !defined.contains(&reference) {
                    return Err(format!(
                        "Part-21 entity #{} references undefined id #{}",
                        id, reference
                    ));
                }
            }
        }

        let mut output = String::new();
        output.push_str("ISO-10303-21;\n");
        output.push_str("HEADER;\n");
        output.push_str(&format!(
            "FILE_DESCRIPTION(('{}'),'{}');\n",
            self.description, DEFAULT_PART21_VERSION
        ));
        output.push_str(&format!(
            "FILE_NAME('{}','{}',('{}'),('{}'),'{}','{}','{}');\n",
            self.file_name,
            self.timestamp,
            self.author,
            self.organization,
            self.preprocessor,
            self.originating_system,
            self.authorization
        ));
        output.push_str(&format!("FILE_SCHEMA(('{}'));\n", self.schema));
        output.push_str("ENDSEC;\n");
        output.push_str("DATA;\n");

        for (id, expression) in &self.entries {
            output.push_str(&format!("#{}={};\n", id, expression));
        }

        output.push_str("ENDSEC;\n");
        output.push_str("END-ISO-10303-21;\n");
        Ok(output)
    }
}

pub fn sanitize_string_literal(value: &str) -> String {
    let mut sanitized = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\'' => sanitized.push_str("''"),
            '\n' | '\r' => sanitized.push(' '),
            _ if ch.is_ascii() => sanitized.push(ch),
            _ => sanitized.push('?'),
        }
    }
    sanitized
}

fn extract_references(expression: &str) -> Vec<usize> {
    let bytes = expression.as_bytes();
    let mut refs = Vec::new();
    let mut idx = 0;

    while idx < bytes.len() {
        if bytes[idx] != b'#' {
            idx += 1;
            continue;
        }

        idx += 1;
        let start = idx;
        while idx < bytes.len() && bytes[idx].is_ascii_digit() {
            idx += 1;
        }

        if idx > start {
            let raw = &expression[start..idx];
            if let Ok(id) = raw.parse::<usize>() {
                refs.push(id);
            }
        }
    }

    refs
}

#[cfg(test)]
mod tests {
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
        assert!(err.contains("undefined id #999"));
    }

    #[test]
    fn sanitizes_non_ascii_literals() {
        let raw = "A'B\nCø";
        let sanitized = sanitize_string_literal(raw);
        assert_eq!(sanitized, "A''B C?");
    }
}
