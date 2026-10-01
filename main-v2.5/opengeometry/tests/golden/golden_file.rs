use crate::digest::sha256;
use crate::record::Failure;
use std::collections::BTreeMap;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const RECORD_INSTRUCTION: &str = "record it with `OG_GOLDEN_UPDATE=1 cargo test --offline --test golden` from opengeometry/ in the debug profile, then review and commit tests/fixtures/golden/<target>.json";

const GOLDEN_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/golden");

const DIFFERING_DIRECTORY: &str = concat!(env!("CARGO_TARGET_TMPDIR"), "/golden");

const COMMITTED: [(&str, &str); 2] = [
    (
        "aarch64-apple-darwin",
        include_str!("../fixtures/golden/aarch64-apple-darwin.json"),
    ),
    (
        "x86-64-unknown-linux-gnu",
        include_str!("../fixtures/golden/x86-64-unknown-linux-gnu.json"),
    ),
];

const UPDATE_ALLOWED_EVENT: &str = "workflow_dispatch";

pub(crate) type Golden = BTreeMap<String, String>;

#[derive(Debug, PartialEq)]
pub(crate) enum Mode {
    Compare,
    Record,
    Dump(PathBuf),
}

#[derive(Debug)]
pub(crate) struct Mismatch {
    pub(crate) missing: Vec<String>,
    pub(crate) stale: Vec<String>,
    pub(crate) differing: Vec<String>,
    unrecorded: bool,
}

impl Display for Mismatch {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        if self.unrecorded {
            return write!(
                formatter,
                "no golden is recorded for this target ({} scenes); {RECORD_INSTRUCTION}",
                self.missing.len()
            );
        }
        let groups = [
            ("scenes missing from the golden", &self.missing),
            ("golden keys with no scene", &self.stale),
            ("scenes differing from the golden", &self.differing),
        ];
        for (title, keys) in groups.iter().filter(|(_, keys)| !keys.is_empty()) {
            writeln!(formatter, "{title} ({}): {}", keys.len(), keys.join(", "))?;
        }
        write!(formatter, "if the change is intended, {RECORD_INSTRUCTION}")
    }
}

pub(crate) fn current_target() -> String {
    let system = if cfg!(target_os = "macos") {
        "apple-darwin"
    } else if cfg!(target_os = "linux") {
        "unknown-linux-gnu"
    } else {
        std::env::consts::OS
    };
    format!("{}-{system}", std::env::consts::ARCH).replace('_', "-")
}

pub(crate) fn parse(text: &str) -> Result<Golden, Failure> {
    Ok(serde_json::from_str(text)?)
}

pub(crate) fn committed(target: &str) -> Result<Golden, Failure> {
    let text = COMMITTED
        .iter()
        .find(|(stem, _)| *stem == target)
        .map_or("{}", |(_, text)| text);
    parse(text)
}

pub(crate) fn digests(texts: &BTreeMap<String, String>) -> Golden {
    texts
        .iter()
        .map(|(key, text)| (key.clone(), sha256(text.as_bytes())))
        .collect()
}

pub(crate) fn compare(expected: &Golden, actual: &Golden) -> Result<(), Mismatch> {
    let absent = |keys: &Golden, other: &Golden| {
        keys.keys()
            .filter(|key| !other.contains_key(*key))
            .cloned()
            .collect::<Vec<_>>()
    };
    let mismatch = Mismatch {
        missing: absent(actual, expected),
        stale: absent(expected, actual),
        differing: actual
            .iter()
            .filter(|(key, digest)| expected.get(*key).is_some_and(|golden| golden != *digest))
            .map(|(key, _)| key.clone())
            .collect(),
        unrecorded: expected.is_empty(),
    };
    if mismatch.missing.is_empty() && mismatch.stale.is_empty() && mismatch.differing.is_empty() {
        return Ok(());
    }
    Err(mismatch)
}

pub(crate) fn update_mode(
    update: Option<&str>,
    github_actions: Option<&str>,
    event_name: Option<&str>,
) -> Result<Mode, String> {
    if update != Some("1") {
        return Ok(Mode::Compare);
    }
    if github_actions.is_some() && event_name != Some(UPDATE_ALLOWED_EVENT) {
        return Err(format!(
            "OG_GOLDEN_UPDATE=1 is refused on CI outside the {UPDATE_ALLOWED_EVENT} record job; this event is {}",
            event_name.unwrap_or("unset")
        ));
    }
    Ok(Mode::Record)
}

pub(crate) fn mode(
    update: Option<&str>,
    github_actions: Option<&str>,
    event_name: Option<&str>,
    dump: Option<PathBuf>,
) -> Result<Mode, String> {
    match dump {
        Some(directory) => Ok(Mode::Dump(directory)),
        None => update_mode(update, github_actions, event_name),
    }
}

pub(crate) fn record(target: &str, actual: &Golden) -> Result<PathBuf, Failure> {
    let file = Path::new(GOLDEN_DIRECTORY).join(format!("{target}.json"));
    fs::write(
        &file,
        format!("{}\n", serde_json::to_string_pretty(actual)?),
    )?;
    Ok(file)
}

fn prepare(output: &Path) -> Result<(), Failure> {
    fs::create_dir_all(output)?;
    if fs::read_dir(output)?.next().is_some() {
        return Err(format!("output directory {} must be empty", output.display()).into());
    }
    Ok(())
}

fn write_texts<'a>(
    directory: &Path,
    texts: impl IntoIterator<Item = (&'a String, &'a String)>,
) -> Result<(), Failure> {
    for (name, text) in texts {
        fs::write(directory.join(format!("{name}.txt")), text)?;
    }
    Ok(())
}

pub(crate) fn dump(directory: &Path, texts: &BTreeMap<String, String>) -> Result<(), Failure> {
    prepare(directory)?;
    write_texts(directory, texts)
}

pub(crate) fn write_differing(
    target: &str,
    differing: &[String],
    texts: &BTreeMap<String, String>,
) -> Result<PathBuf, Failure> {
    let directory = Path::new(DIFFERING_DIRECTORY).join(target);
    if directory.exists() {
        fs::remove_dir_all(&directory)?;
    }
    fs::create_dir_all(&directory)?;
    write_texts(
        &directory,
        differing.iter().filter_map(|key| texts.get_key_value(key)),
    )?;
    Ok(directory)
}
