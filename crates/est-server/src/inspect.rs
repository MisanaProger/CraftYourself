//! Project inspection: license and language detection (spec 3.1).

use std::collections::HashMap;
use std::path::Path;

/// Best-effort SPDX identifier of the project's license.
pub fn detect_license(root: &Path, files: &[String]) -> Option<String> {
    // 1. Explicit metadata wins.
    if let Ok(cargo) = std::fs::read_to_string(root.join("Cargo.toml")) {
        if let Ok(value) = cargo.parse::<toml::Table>() {
            if let Some(l) = value
                .get("package")
                .and_then(|p| p.get("license"))
                .and_then(|l| l.as_str())
            {
                return Some(l.to_owned());
            }
        }
    }
    if let Ok(pkg) = std::fs::read_to_string(root.join("package.json")) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&pkg) {
            if let Some(l) = v.get("license").and_then(|l| l.as_str()) {
                return Some(l.to_owned());
            }
        }
    }

    // 2. License files at the repository root.
    let mut found: Vec<&'static str> = files
        .iter()
        .filter(|f| !f.contains('/'))
        .filter(|f| {
            let up = f.to_ascii_uppercase();
            up.starts_with("LICENSE") || up.starts_with("LICENCE") || up.starts_with("COPYING")
        })
        .filter_map(|f| std::fs::read_to_string(root.join(f)).ok())
        .filter_map(|text| identify_license_text(&text))
        .collect();
    found.sort_unstable();
    found.dedup();
    match found.len() {
        0 => None,
        _ => Some(found.join(" OR ")),
    }
}

pub fn identify_license_text(text: &str) -> Option<&'static str> {
    let t = text.to_ascii_lowercase();
    let has = |s: &str| t.contains(s);
    Some(if has("gnu affero general public license") {
        "AGPL-3.0"
    } else if has("gnu lesser general public license") {
        if has("version 3") {
            "LGPL-3.0"
        } else {
            "LGPL-2.1"
        }
    } else if has("gnu general public license") {
        if has("version 3") {
            "GPL-3.0"
        } else {
            "GPL-2.0"
        }
    } else if has("apache license") && has("version 2.0") {
        "Apache-2.0"
    } else if has("mozilla public license") && has("2.0") {
        "MPL-2.0"
    } else if has("permission is hereby granted, free of charge") {
        "MIT"
    } else if has("redistribution and use in source and binary forms") {
        if has("neither the name") {
            "BSD-3-Clause"
        } else {
            "BSD-2-Clause"
        }
    } else if has("this is free and unencumbered software") {
        "Unlicense"
    } else if has("permission to use, copy, modify, and/or distribute this software") {
        "ISC"
    } else {
        return None;
    })
}

const EXTENSIONS: &[(&str, &str)] = &[
    ("rs", "rust"),
    ("py", "python"),
    ("go", "go"),
    ("js", "javascript"),
    ("mjs", "javascript"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("c", "c"),
    ("h", "c"),
    ("cc", "cpp"),
    ("cpp", "cpp"),
    ("hpp", "cpp"),
    ("java", "java"),
    ("kt", "kotlin"),
    ("rb", "ruby"),
    ("cs", "csharp"),
    ("swift", "swift"),
    ("zig", "zig"),
    ("hs", "haskell"),
    ("ml", "ocaml"),
    ("ex", "elixir"),
    ("php", "php"),
];

/// Primary language by number of tracked source files.
pub fn detect_language(files: &[String]) -> Option<String> {
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for f in files {
        if let Some(ext) = Path::new(f).extension().and_then(|e| e.to_str()) {
            if let Some((_, lang)) = EXTENSIONS.iter().find(|(e, _)| *e == ext) {
                *counts.entry(lang).or_default() += 1;
            }
        }
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .map(|(l, _)| l.to_owned())
}

/// `https://github.com/owner/repo.git` → `repo`.
pub fn name_from_url(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_texts() {
        assert_eq!(
            identify_license_text(
                "MIT License\n\nPermission is hereby granted, free of charge, to any person"
            ),
            Some("MIT")
        );
        assert_eq!(
            identify_license_text("GNU GENERAL PUBLIC LICENSE\nVersion 3, 29 June 2007"),
            Some("GPL-3.0")
        );
        assert_eq!(
            identify_license_text("Apache License\nVersion 2.0, January 2004"),
            Some("Apache-2.0")
        );
        assert_eq!(identify_license_text("All rights reserved."), None);
    }

    #[test]
    fn dual_license_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("LICENSE-MIT"),
            "Permission is hereby granted, free of charge",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("LICENSE-APACHE"),
            "Apache License Version 2.0",
        )
        .unwrap();
        let files = vec!["LICENSE-MIT".to_owned(), "LICENSE-APACHE".to_owned()];
        assert_eq!(
            detect_license(dir.path(), &files).as_deref(),
            Some("Apache-2.0 OR MIT")
        );
    }

    #[test]
    fn language_and_name() {
        let files: Vec<String> = ["src/main.rs", "src/lib.rs", "build.py", "README.md"]
            .map(String::from)
            .into();
        assert_eq!(detect_language(&files).as_deref(), Some("rust"));
        assert_eq!(
            name_from_url("https://github.com/BurntSushi/ripgrep.git"),
            "ripgrep"
        );
        assert_eq!(name_from_url("git@github.com:owner/repo.git"), "repo");
        assert_eq!(name_from_url("/tmp/x/origin/"), "origin");
    }
}
