pub(super) fn references(expression: &str) -> Vec<usize> {
    let bytes = expression.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    let mut quoted = false;
    while index < bytes.len() {
        if bytes[index] == b'\'' {
            if quoted && bytes.get(index + 1) == Some(&b'\'') {
                index += 2;
                continue;
            }
            quoted = !quoted;
            index += 1;
            continue;
        }
        if !quoted && bytes[index] == b'#' {
            let start = index + 1;
            index = start;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if let Ok(value) = expression[start..index].parse() {
                result.push(value);
            }
            continue;
        }
        index += 1;
    }
    result
}

pub(super) fn fields(expression: &str) -> Result<Vec<String>, String> {
    let start = expression
        .find('(')
        .ok_or("entity has no opening parenthesis")?;
    let end = expression
        .rfind(')')
        .ok_or("entity has no closing parenthesis")?;
    if end <= start {
        return Err("empty entity expression".into());
    }
    let content = &expression[start + 1..end];
    let bytes = content.as_bytes();
    let mut result = Vec::new();
    let mut depth = 0;
    let mut quoted = false;
    let mut from = 0;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' if quoted && bytes.get(index + 1) == Some(&b'\'') => index += 2,
            b'\'' => {
                quoted = !quoted;
                index += 1;
            }
            b'(' if !quoted => {
                depth += 1;
                index += 1;
            }
            b')' if !quoted => {
                if depth == 0 {
                    return Err("unbalanced entity expression".into());
                }
                depth -= 1;
                index += 1;
            }
            b',' if !quoted && depth == 0 => {
                result.push(content[from..index].trim().to_string());
                from = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    if quoted || depth != 0 {
        return Err("unbalanced entity expression".into());
    }
    result.push(content[from..].trim().to_string());
    Ok(result)
}

pub(super) fn reference(field: &str) -> Result<usize, String> {
    field
        .trim()
        .strip_prefix('#')
        .ok_or("missing entity reference")?
        .parse()
        .map_err(|_| "invalid entity reference".into())
}

pub(super) fn keywords(expression: &str) -> Vec<String> {
    let bytes = expression.as_bytes();
    let mut result = Vec::new();
    let mut index = 0;
    let mut quoted = false;
    while index < bytes.len() {
        if bytes[index] == b'\'' {
            if quoted && bytes.get(index + 1) == Some(&b'\'') {
                index += 2;
                continue;
            }
            quoted = !quoted;
            index += 1;
            continue;
        }
        if !quoted && bytes[index].is_ascii_uppercase() {
            let start = index;
            index += 1;
            while index < bytes.len()
                && (bytes[index].is_ascii_uppercase()
                    || bytes[index].is_ascii_digit()
                    || bytes[index] == b'_')
            {
                index += 1;
            }
            if bytes.get(index) == Some(&b'(') {
                result.push(expression[start..index].into());
            }
            continue;
        }
        index += 1;
    }
    result
}
