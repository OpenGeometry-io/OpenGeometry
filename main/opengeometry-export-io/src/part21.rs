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
