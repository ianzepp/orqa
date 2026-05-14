#![allow(clippy::absurd_extreme_comparisons)]

use std::{fs, path::Path};

const MAX_UNWRAP: usize = 0;
const MAX_EXPECT: usize = 0;
const MAX_PANIC: usize = 0;
const MAX_UNREACHABLE: usize = 0;
const MAX_TODO: usize = 0;
const MAX_UNIMPLEMENTED: usize = 0;

struct SourceFile {
    path: String,
    content: String,
}

fn source_files() -> Vec<SourceFile> {
    let mut files = Vec::new();
    collect_rs_files(Path::new("src"), &mut files);
    files
}

fn collect_rs_files(dir: &Path, out: &mut Vec<SourceFile>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
            continue;
        }

        if path.extension().is_some_and(|ext| ext == "rs") {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            if name.ends_with("_test.rs") || name.ends_with(".test.rs") {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&path) {
                out.push(SourceFile {
                    path: path.to_string_lossy().into_owned(),
                    content,
                });
            }
        }
    }
}

fn count_in_source(files: &[SourceFile], pattern: &str) -> usize {
    files
        .iter()
        .map(|file| {
            file.content
                .lines()
                .filter(|line| line.contains(pattern))
                .count()
        })
        .sum()
}

fn assert_budget(name: &str, observed: usize, max: usize) {
    assert!(
        observed <= max,
        "{name} budget exceeded: found {observed}, max {max}"
    );
}

#[test]
fn risky_macro_and_unwrap_budgets() {
    let files = source_files();

    assert_budget(
        ".unwrap()",
        count_in_source(&files, ".unwrap()"),
        MAX_UNWRAP,
    );
    assert_budget(".expect(", count_in_source(&files, ".expect("), MAX_EXPECT);
    assert_budget("panic!(", count_in_source(&files, "panic!("), MAX_PANIC);
    assert_budget(
        "unreachable!(",
        count_in_source(&files, "unreachable!("),
        MAX_UNREACHABLE,
    );
    assert_budget("todo!(", count_in_source(&files, "todo!("), MAX_TODO);
    assert_budget(
        "unimplemented!(",
        count_in_source(&files, "unimplemented!("),
        MAX_UNIMPLEMENTED,
    );
}

#[test]
fn test_modules_use_companion_files() {
    for file in source_files() {
        let lines: Vec<_> = file.content.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !line.trim().starts_with("#[cfg(test)]") {
                continue;
            }

            let next = lines
                .iter()
                .skip(index + 1)
                .find(|candidate| !candidate.trim().is_empty())
                .map(|candidate| candidate.trim());
            if !matches!(next, Some(next) if next.starts_with("#[path = ")) {
                continue;
            }

            let after_path = lines
                .iter()
                .skip(index + 2)
                .find(|candidate| !candidate.trim().is_empty())
                .map(|candidate| candidate.trim());
            assert!(
                matches!(after_path, Some("mod tests;")),
                "{} has #[cfg(test)] #[path = ...] without `mod tests;`",
                file.path
            );
        }

        assert!(
            !file.content.contains("#[cfg(test)]\nmod tests {"),
            "{} contains an inline test module; use a *_test.rs companion file",
            file.path
        );
    }
}
