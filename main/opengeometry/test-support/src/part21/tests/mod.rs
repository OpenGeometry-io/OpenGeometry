mod base;
mod reports;
mod rules;
mod values;

use super::Document;
use std::cmp::Ordering;

fn parsed(text: &str) -> Document {
    Document::parse(text).unwrap_or_else(|error| panic!("{error}"))
}

fn assert_rejected(text: &str, needle: &str) {
    match Document::parse(text) {
        Ok(_) => panic!("the text parsed; expected an error naming {needle}"),
        Err(error) => assert!(error.contains(needle), "{error} does not name {needle}"),
    }
}

fn assert_error(result: Result<(), String>, needle: &str) {
    let error = result.expect_err(needle);
    assert!(error.contains(needle), "{error} does not name {needle}");
}

fn assert_close(actual: [f64; 3], expected: [f64; 3]) {
    for axis in 0..3 {
        assert!(
            (actual[axis] - expected[axis]).abs() < 1e-12,
            "{actual:?} is not {expected:?}"
        );
    }
}

fn edited(text: &str, id: usize, from: &str, to: &str) -> String {
    let prefix = format!("#{id}=");
    let mut found = false;
    let mut result = String::with_capacity(text.len());
    for line in text.lines() {
        if line.starts_with(&prefix) && line.contains(from) {
            found = true;
            result.push_str(&line.replacen(from, to, 1));
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    assert!(found, "#{id} lacks {from}");
    result
}

fn without(text: &str, id: usize) -> String {
    let prefix = format!("#{id}=");
    let mut result = String::with_capacity(text.len());
    for line in text.lines().filter(|line| !line.starts_with(&prefix)) {
        result.push_str(&renumbered(line, id));
        result.push('\n');
    }
    result
}

fn renumbered(line: &str, removed: usize) -> String {
    let mut result = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(index) = rest.find('#') {
        result.push_str(&rest[..index]);
        let digits = rest[index + 1..]
            .bytes()
            .take_while(u8::is_ascii_digit)
            .count();
        let target: usize = rest[index + 1..index + 1 + digits].parse().unwrap();
        match target.cmp(&removed) {
            Ordering::Less => result.push_str(&format!("#{target}")),
            Ordering::Equal => result.push('$'),
            Ordering::Greater => result.push_str(&format!("#{}", target - 1)),
        }
        rest = &rest[index + 1 + digits..];
    }
    result.push_str(rest);
    result
}

fn appended(text: &str, expression: &str) -> String {
    let count = text.lines().filter(|line| line.starts_with('#')).count();
    let end = text.rfind("ENDSEC;").unwrap();
    format!(
        "{}#{}={expression};\n{}",
        &text[..end],
        count + 1,
        &text[end..]
    )
}
