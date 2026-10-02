use super::lexer::fields;

pub fn normalise_step(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        result.push_str(&positive_zeros(&renamed(line)));
        result.push('\n');
    }
    result
}

fn renamed(line: &str) -> String {
    let Some(expression) = line.strip_suffix(';') else {
        return line.into();
    };
    let head = &expression[..expression.find('(').unwrap_or(0)];
    let names: &[(usize, &str)] = match head.split_once('=').map_or(head, |(_, kind)| kind) {
        "FILE_DESCRIPTION" => &[(0, "('description')")],
        "FILE_NAME" => &[(0, "'name'")],
        "PRODUCT" => &[(0, "'product'"), (1, "'product'")],
        "MANIFOLD_SOLID_BREP" | "BREP_WITH_VOIDS" => &[(0, "'solid'")],
        _ => return line.into(),
    };
    let Ok(mut args) = fields(expression) else {
        return line.into();
    };
    for (index, name) in names {
        if let Some(field) = args.get_mut(*index) {
            *field = (*name).into();
        }
    }
    format!("{head}({});", args.join(","))
}

fn positive_zeros(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut result = String::with_capacity(line.len());
    let mut quoted = false;
    let mut from = 0;
    for (index, byte) in bytes.iter().enumerate() {
        match byte {
            b'\'' => quoted = !quoted,
            b'-' if !quoted
                && index > 0
                && matches!(bytes[index - 1], b'(' | b',')
                && negative_zero(&line[index..]) =>
            {
                result.push_str(&line[from..index]);
                from = index + 1;
            }
            _ => {}
        }
    }
    result.push_str(&line[from..]);
    result
}

fn negative_zero(token: &str) -> bool {
    let end = token.find([',', ')']).unwrap_or(token.len());
    token[..end].parse::<f64>().is_ok_and(|value| value == 0.0)
}
