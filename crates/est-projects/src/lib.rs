//! Project inspection (architecture v0.2, §5.1, §6; ТЗ §3.1): which
//! languages a project uses and under which license it is distributed.

use std::path::Path;

use est_application::ports::{ProjectInspection, ProjectInspector};
use est_domain::CommandSpec;
use est_languages::LanguageRegistry;

/// [`ProjectInspector`] backed by the language adapters.
#[derive(Default)]
pub struct Inspector {
    pub languages: LanguageRegistry,
}

impl ProjectInspector for Inspector {
    fn inspect(&self, root: &Path, files: &[String]) -> ProjectInspection {
        ProjectInspection {
            languages: self.languages.detect(root, files),
            license: detect_license(root, files),
        }
    }

    fn commands(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        self.languages.commands(root, files)
    }
}

/// Best-effort SPDX identifier of the project's license.
pub fn detect_license(root: &Path, files: &[String]) -> Option<String> {
    // 1. Explicit metadata wins.
    if let Ok(cargo) = std::fs::read_to_string(root.join("Cargo.toml"))
        && let Ok(value) = cargo.parse::<toml::Table>()
        && let Some(l) = value
            .get("package")
            .and_then(|p| p.get("license"))
            .and_then(|l| l.as_str())
    {
        return Some(l.to_owned());
    }
    if let Ok(pkg) = std::fs::read_to_string(root.join("package.json"))
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&pkg)
        && let Some(l) = v.get("license").and_then(|l| l.as_str())
    {
        return Some(l.to_owned());
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
    fn inspector_combines_languages_and_license() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"x\"\nlicense = \"MIT OR Apache-2.0\"\n",
        )
        .unwrap();
        let files = vec!["Cargo.toml".to_owned(), "src/lib.rs".to_owned()];
        let i = Inspector::default();
        let r = i.inspect(dir.path(), &files);
        assert_eq!(r.license.as_deref(), Some("MIT OR Apache-2.0"));
        assert_eq!(r.languages[0].language, "rust");
        assert_eq!(i.commands(dir.path(), &files).len(), 4);
    }
}
