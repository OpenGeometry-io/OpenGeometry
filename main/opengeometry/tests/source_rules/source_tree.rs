use crate::lexer::{tokenize, Token};
use crate::scanner::{scan_file, FileScan};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) struct SourceFile {
    pub(crate) path: String,
    pub(crate) module: Vec<String>,
    pub(crate) line_count: usize,
    pub(crate) comments: Vec<Token>,
    pub(crate) code: Vec<Token>,
    pub(crate) scan: FileScan,
    pub(crate) test: bool,
}

impl SourceFile {
    pub(crate) fn is_test_at(&self, index: usize) -> bool {
        self.test || self.scan.contexts[index].test
    }

    pub(crate) fn directory(&self) -> &str {
        self.path
            .rsplit_once('/')
            .map_or("", |(directory, _)| directory)
    }

    pub(crate) fn stem(&self) -> &str {
        let name = self
            .path
            .rsplit_once('/')
            .map_or(self.path.as_str(), |(_, name)| name);
        name.strip_suffix(".rs").unwrap_or(name)
    }
}

pub(crate) struct SourceTree {
    pub(crate) files: Vec<SourceFile>,
    pub(crate) directories: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct LoadError {
    pub(crate) path: PathBuf,
    pub(crate) reason: String,
}

impl SourceTree {
    pub(crate) fn load(root: &Path) -> Result<SourceTree, LoadError> {
        let mut paths = Vec::new();
        let mut directories = Vec::new();
        collect(root, "", &mut paths, &mut directories)?;
        let files = paths
            .into_iter()
            .map(|path| load_file(root, path))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tree = SourceTree { files, directories };
        mark_test_files(&mut tree);
        Ok(tree)
    }

    pub(crate) fn from_sources(sources: &[(&str, &str)]) -> SourceTree {
        let mut sources = sources.to_vec();
        sources.sort();
        let files = sources
            .iter()
            .map(|(path, text)| {
                parse_file(path.to_string(), text)
                    .unwrap_or_else(|reason| panic!("{path}: {reason}"))
            })
            .collect();
        let mut directories: Vec<String> = sources
            .iter()
            .flat_map(|(path, _)| {
                path.match_indices('/')
                    .map(|(end, _)| path[..end].to_string())
                    .collect::<Vec<_>>()
            })
            .collect();
        directories.sort();
        directories.dedup();
        let mut tree = SourceTree { files, directories };
        mark_test_files(&mut tree);
        tree
    }

    pub(crate) fn module_index(&self, module: &[String]) -> Option<usize> {
        self.files.iter().position(|file| file.module == module)
    }

    pub(crate) fn has_file(&self, path: &str) -> bool {
        self.files.iter().any(|file| file.path == path)
    }
}

fn read_error(path: &Path, error: &std::io::Error) -> LoadError {
    LoadError {
        path: path.to_path_buf(),
        reason: error.to_string(),
    }
}

fn collect(
    root: &Path,
    relative: &str,
    files: &mut Vec<String>,
    directories: &mut Vec<String>,
) -> Result<(), LoadError> {
    let directory = root.join(relative);
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| read_error(&directory, &error))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| read_error(&directory, &error))?;
    entries.sort();
    for entry in entries {
        let name = entry
            .file_name()
            .map_or(String::new(), |name| name.to_string_lossy().into_owned());
        let child = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if entry.is_dir() {
            directories.push(child.clone());
            collect(root, &child, files, directories)?;
        } else if name.ends_with(".rs") {
            files.push(child);
        }
    }
    Ok(())
}

fn module_path(relative: &str) -> Vec<String> {
    let without_extension = relative.strip_suffix(".rs").unwrap_or(relative);
    let mut module: Vec<String> = without_extension.split('/').map(str::to_string).collect();
    if module.last().is_some_and(|last| last == "mod") || module == ["lib"] {
        module.pop();
    }
    module
}

fn load_file(root: &Path, path: String) -> Result<SourceFile, LoadError> {
    let absolute = root.join(&path);
    let text = fs::read_to_string(&absolute).map_err(|error| read_error(&absolute, &error))?;
    parse_file(path, &text).map_err(|reason| LoadError {
        path: absolute,
        reason,
    })
}

fn parse_file(path: String, text: &str) -> Result<SourceFile, String> {
    let tokens =
        tokenize(text).map_err(|error| format!("line {}: {}", error.line, error.reason))?;
    let (comments, code): (Vec<Token>, Vec<Token>) =
        tokens.into_iter().partition(Token::is_comment);
    let scan = scan_file(&code);
    Ok(SourceFile {
        module: module_path(&path),
        line_count: text.lines().count(),
        path,
        comments,
        code,
        scan,
        test: false,
    })
}

fn declared_as_test(parent: &SourceFile, name: &str) -> bool {
    parent.scan.mods.iter().any(|declaration| {
        declaration.name == name && declaration.scope.is_empty() && declaration.test
    })
}

fn mark_test_files(tree: &mut SourceTree) {
    let mut order: Vec<usize> = (0..tree.files.len()).collect();
    order.sort_by_key(|&index| tree.files[index].module.len());
    for index in order {
        let module = tree.files[index].module.clone();
        let Some((name, parent_module)) = module.split_last() else {
            continue;
        };
        let parent = tree
            .module_index(parent_module)
            .map(|parent| &tree.files[parent]);
        let test = module.iter().any(|segment| segment == "tests")
            || parent.is_some_and(|parent| parent.test || declared_as_test(parent, name));
        tree.files[index].test = test;
    }
}
