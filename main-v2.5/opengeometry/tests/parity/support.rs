use std::path::{Path, PathBuf};

pub fn parity_fixtures(folder: &str, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/parity")
        .join(folder);
    let mut files = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            name.ends_with(".json") && keep(&name)
        })
        .collect::<Vec<_>>();
    files.sort();
    files
}
