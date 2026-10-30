use serde_json::Value;

pub fn ordered_bits(value: f64) -> u64 {
    let bits = value.to_bits();
    if bits >> 63 == 0 {
        bits | (1 << 63)
    } else {
        !bits
    }
}

pub fn within_four_ulp(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) => {
            if left == right {
                return true;
            }
            match (left.as_f64(), right.as_f64()) {
                (Some(a), Some(b)) if a == 0.0 && b == 0.0 => true,
                (Some(a), Some(b)) => ordered_bits(a).abs_diff(ordered_bits(b)) <= 4,
                _ => false,
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            left.len() == right.len() && left.iter().zip(right).all(|(a, b)| within_four_ulp(a, b))
        }
        (Value::Object(left), Value::Object(right)) => {
            left.len() == right.len()
                && left.iter().all(|(key, value)| {
                    right
                        .get(key)
                        .is_some_and(|other| within_four_ulp(value, other))
                })
        }
        _ => actual == expected,
    }
}

pub fn step_parts(text: &str) -> (String, Vec<f64>) {
    let normalized = text
        .lines()
        .map(|line| {
            if line.starts_with("FILE_DESCRIPTION(") {
                "FILE_DESCRIPTION(('OpenGeometry analytic BRep'),'2;1');".to_string()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let bytes = normalized.as_bytes();
    let mut structure = String::with_capacity(normalized.len());
    let mut reals = Vec::new();
    let mut cursor = 0;
    let mut index = 0;
    while index < bytes.len() {
        let start = index;
        if bytes[index] == b'-' || bytes[index] == b'+' {
            index += 1;
        }
        let digits = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index > digits && index < bytes.len() && bytes[index] == b'.' {
            index += 1;
            let fractional = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > fractional && index < bytes.len() && bytes[index] == b'E' {
                index += 1;
                if index < bytes.len() && (bytes[index] == b'-' || bytes[index] == b'+') {
                    index += 1;
                }
                let exponent = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index > exponent {
                    structure.push_str(&normalized[cursor..start]);
                    structure.push('@');
                    reals.push(normalized[start..index].parse().unwrap());
                    cursor = index;
                    continue;
                }
            }
        }
        index = start + 1;
    }
    structure.push_str(&normalized[cursor..]);
    (structure, reals)
}
