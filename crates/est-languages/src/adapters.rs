//! Built-in adapters. First priority (architecture v0.2, §6.1): Rust, C,
//! C++. Second priority: Python, JavaScript/TypeScript.

use std::path::Path;

use est_domain::{CommandKind, CommandSpec, LanguageDetection};

use crate::{LanguageAdapter, cmd, count_ext, file_name, shallowest};

fn detection(language: &str, files: u32, build_systems: Vec<String>) -> Option<LanguageDetection> {
    (files > 0).then(|| LanguageDetection {
        language: language.to_owned(),
        files,
        build_systems,
    })
}

// ---- Rust ----------------------------------------------------------------

pub struct Rust;

impl LanguageAdapter for Rust {
    fn id(&self) -> &'static str {
        "rust"
    }

    fn detect(&self, _root: &Path, files: &[String]) -> Option<LanguageDetection> {
        let bs = if shallowest(files, "Cargo.toml").is_some() {
            vec!["cargo".to_owned()]
        } else {
            vec![]
        };
        detection("rust", count_ext(files, &["rs"]), bs)
    }

    fn build_commands(&self, _root: &Path, files: &[String]) -> Vec<CommandSpec> {
        shallowest(files, "Cargo.toml")
            .map(|dir| {
                vec![cmd(
                    CommandKind::Build,
                    "rust",
                    &dir,
                    "cargo",
                    &["build", "--all-targets"],
                    "Build all targets",
                )]
            })
            .unwrap_or_default()
    }

    fn discover_tests(&self, _root: &Path, files: &[String]) -> Vec<CommandSpec> {
        shallowest(files, "Cargo.toml")
            .map(|dir| {
                vec![cmd(
                    CommandKind::Test,
                    "rust",
                    &dir,
                    "cargo",
                    &["test"],
                    "Run unit, integration and doc tests",
                )]
            })
            .unwrap_or_default()
    }

    fn discover_checks(&self, _root: &Path, files: &[String]) -> Vec<CommandSpec> {
        shallowest(files, "Cargo.toml")
            .map(|dir| {
                vec![
                    cmd(
                        CommandKind::Check,
                        "rust",
                        &dir,
                        "cargo",
                        &["fmt", "--all", "--check"],
                        "Formatting (rustfmt)",
                    ),
                    cmd(
                        CommandKind::Check,
                        "rust",
                        &dir,
                        "cargo",
                        &["clippy", "--all-targets"],
                        "Lints (Clippy)",
                    ),
                ]
            })
            .unwrap_or_default()
    }
}

// ---- C and C++ -----------------------------------------------------------

const C_EXT: &[&str] = &["c"];
const CPP_EXT: &[&str] = &["cc", "cpp", "cxx", "c++", "hpp", "hh", "hxx"];

fn native_build_systems(files: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for (marker, name) in [
        ("CMakeLists.txt", "cmake"),
        ("meson.build", "meson"),
        ("Makefile", "make"),
    ] {
        if files.iter().any(|f| file_name(f) == marker) {
            out.push(name.to_owned());
        }
    }
    out
}

fn makefile_has_target(root: &Path, dir: &str, target: &str) -> bool {
    std::fs::read_to_string(root.join(dir).join("Makefile"))
        .map(|t| t.lines().any(|l| l.starts_with(&format!("{target}:"))))
        .unwrap_or(false)
}

fn native_build(lang: &str, root: &Path, files: &[String]) -> Vec<CommandSpec> {
    if let Some(dir) = shallowest(files, "CMakeLists.txt") {
        return vec![
            cmd(
                CommandKind::Build,
                lang,
                &dir,
                "cmake",
                &["-S", ".", "-B", "build"],
                "Configure (CMake)",
            ),
            cmd(
                CommandKind::Build,
                lang,
                &dir,
                "cmake",
                &["--build", "build"],
                "Build (CMake)",
            ),
        ];
    }
    if let Some(dir) = shallowest(files, "meson.build") {
        return vec![
            cmd(
                CommandKind::Build,
                lang,
                &dir,
                "meson",
                &["setup", "build"],
                "Configure (Meson)",
            ),
            cmd(
                CommandKind::Build,
                lang,
                &dir,
                "meson",
                &["compile", "-C", "build"],
                "Build (Meson)",
            ),
        ];
    }
    if let Some(dir) = shallowest(files, "Makefile") {
        let _ = root;
        return vec![cmd(
            CommandKind::Build,
            lang,
            &dir,
            "make",
            &[],
            "Build (make)",
        )];
    }
    Vec::new()
}

fn native_tests(lang: &str, root: &Path, files: &[String]) -> Vec<CommandSpec> {
    if let Some(dir) = shallowest(files, "CMakeLists.txt") {
        return vec![cmd(
            CommandKind::Test,
            lang,
            &dir,
            "ctest",
            &["--test-dir", "build", "--output-on-failure"],
            "Run CTest tests",
        )];
    }
    if let Some(dir) = shallowest(files, "meson.build") {
        return vec![cmd(
            CommandKind::Test,
            lang,
            &dir,
            "meson",
            &["test", "-C", "build"],
            "Run Meson tests",
        )];
    }
    if let Some(dir) = shallowest(files, "Makefile") {
        for target in ["test", "check"] {
            if makefile_has_target(root, &dir, target) {
                return vec![cmd(
                    CommandKind::Test,
                    lang,
                    &dir,
                    "make",
                    &[target],
                    "Run tests (make)",
                )];
            }
        }
    }
    Vec::new()
}

pub struct C;

impl LanguageAdapter for C {
    fn id(&self) -> &'static str {
        "c"
    }

    fn detect(&self, _root: &Path, files: &[String]) -> Option<LanguageDetection> {
        // Headers count as C only when there is no C++ in the project.
        let headers = if count_ext(files, CPP_EXT) == 0 {
            count_ext(files, &["h"])
        } else {
            0
        };
        let sources = count_ext(files, C_EXT);
        if sources == 0 {
            return None;
        }
        detection("c", sources + headers, native_build_systems(files))
    }

    fn build_commands(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        native_build("c", root, files)
    }

    fn discover_tests(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        native_tests("c", root, files)
    }

    fn discover_checks(&self, _root: &Path, _files: &[String]) -> Vec<CommandSpec> {
        Vec::new()
    }
}

pub struct Cpp;

impl LanguageAdapter for Cpp {
    fn id(&self) -> &'static str {
        "cpp"
    }

    fn detect(&self, _root: &Path, files: &[String]) -> Option<LanguageDetection> {
        let n = count_ext(files, CPP_EXT);
        if n == 0 {
            return None;
        }
        detection(
            "cpp",
            n + count_ext(files, &["h"]),
            native_build_systems(files),
        )
    }

    fn build_commands(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        // In mixed C/C++ projects the build is shared: describe it once.
        if C.detect(root, files).is_some() {
            return Vec::new();
        }
        native_build("cpp", root, files)
    }

    fn discover_tests(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        if C.detect(root, files).is_some() {
            return Vec::new();
        }
        native_tests("cpp", root, files)
    }

    fn discover_checks(&self, _root: &Path, files: &[String]) -> Vec<CommandSpec> {
        if files.iter().any(|f| f == ".clang-tidy") && shallowest(files, "CMakeLists.txt").is_some()
        {
            return vec![cmd(
                CommandKind::Check,
                "cpp",
                "",
                "run-clang-tidy",
                &["-p", "build"],
                "Static analysis (clang-tidy)",
            )];
        }
        Vec::new()
    }
}

// ---- Python --------------------------------------------------------------

pub struct Python;

fn pyproject_has(root: &Path, table: &str) -> bool {
    std::fs::read_to_string(root.join("pyproject.toml"))
        .map(|t| t.contains(&format!("[{table}")))
        .unwrap_or(false)
}

impl LanguageAdapter for Python {
    fn id(&self) -> &'static str {
        "python"
    }

    fn detect(&self, _root: &Path, files: &[String]) -> Option<LanguageDetection> {
        let mut bs = Vec::new();
        for (marker, name) in [
            ("pyproject.toml", "pyproject"),
            ("setup.py", "setuptools"),
            ("requirements.txt", "pip"),
        ] {
            if files.iter().any(|f| f == marker) {
                bs.push(name.to_owned());
            }
        }
        detection("python", count_ext(files, &["py"]), bs)
    }

    fn build_commands(&self, _root: &Path, _files: &[String]) -> Vec<CommandSpec> {
        Vec::new()
    }

    fn discover_tests(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        let pytest = files.iter().any(|f| {
            matches!(file_name(f), "pytest.ini" | "conftest.py") || f.starts_with("tests/test_")
        }) || pyproject_has(root, "tool.pytest");
        if pytest {
            vec![cmd(
                CommandKind::Test,
                "python",
                "",
                "python",
                &["-m", "pytest"],
                "Run tests (pytest)",
            )]
        } else {
            vec![cmd(
                CommandKind::Test,
                "python",
                "",
                "python",
                &["-m", "unittest", "discover"],
                "Run tests (unittest)",
            )]
        }
    }

    fn discover_checks(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        if files
            .iter()
            .any(|f| matches!(f.as_str(), "ruff.toml" | ".ruff.toml"))
            || pyproject_has(root, "tool.ruff")
        {
            vec![cmd(
                CommandKind::Check,
                "python",
                "",
                "ruff",
                &["check", "."],
                "Lints (Ruff)",
            )]
        } else {
            Vec::new()
        }
    }
}

// ---- JavaScript / TypeScript --------------------------------------------

pub struct JavaScript;

fn package_scripts(root: &Path) -> Vec<String> {
    std::fs::read_to_string(root.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| {
            v.get("scripts")?
                .as_object()
                .map(|o| o.keys().cloned().collect())
        })
        .unwrap_or_default()
}

fn package_manager(files: &[String]) -> &'static str {
    if files.iter().any(|f| f == "pnpm-lock.yaml") {
        "pnpm"
    } else if files.iter().any(|f| f == "yarn.lock") {
        "yarn"
    } else {
        "npm"
    }
}

impl JavaScript {
    fn script(
        &self,
        root: &Path,
        files: &[String],
        kind: CommandKind,
        names: &[&str],
    ) -> Vec<CommandSpec> {
        let scripts = package_scripts(root);
        let pm = package_manager(files);
        names
            .iter()
            .filter(|n| scripts.iter().any(|s| s == *n))
            .map(|n| {
                cmd(
                    kind,
                    "javascript",
                    "",
                    pm,
                    &["run", n],
                    &format!("package.json script `{n}`"),
                )
            })
            .collect()
    }
}

impl LanguageAdapter for JavaScript {
    fn id(&self) -> &'static str {
        "javascript"
    }

    fn detect(&self, _root: &Path, files: &[String]) -> Option<LanguageDetection> {
        let bs = if files.iter().any(|f| f == "package.json") {
            vec![package_manager(files).to_owned()]
        } else {
            vec![]
        };
        let n = count_ext(files, &["js", "mjs", "cjs", "jsx", "ts", "tsx", "mts"]);
        detection("javascript", n, bs)
    }

    fn build_commands(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        self.script(root, files, CommandKind::Build, &["build"])
    }

    fn discover_tests(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        self.script(root, files, CommandKind::Test, &["test"])
    }

    fn discover_checks(&self, root: &Path, files: &[String]) -> Vec<CommandSpec> {
        self.script(root, files, CommandKind::Check, &["lint", "typecheck"])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn rust_workspace_commands() {
        let dir = tempfile::tempdir().unwrap();
        let f = files(&["Cargo.toml", "crates/a/Cargo.toml", "crates/a/src/lib.rs"]);
        let tests = Rust.discover_tests(dir.path(), &f);
        assert_eq!(tests[0].display(), "cargo test");
        assert_eq!(tests[0].workdir, "");
        assert_eq!(Rust.discover_checks(dir.path(), &f).len(), 2);
    }

    #[test]
    fn c_with_makefile_targets() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Makefile"),
            "all:\n\tcc main.c\ncheck: all\n\t./run\n",
        )
        .unwrap();
        let f = files(&["Makefile", "main.c", "util.h"]);
        let det = C.detect(dir.path(), &f).unwrap();
        assert_eq!(det.files, 2);
        assert_eq!(det.build_systems, vec!["make"]);
        assert!(Cpp.detect(dir.path(), &f).is_none());
        assert_eq!(C.discover_tests(dir.path(), &f)[0].display(), "make check");
    }

    #[test]
    fn cpp_cmake() {
        let dir = tempfile::tempdir().unwrap();
        let f = files(&["CMakeLists.txt", "src/a.cpp", "include/a.h"]);
        let det = Cpp.detect(dir.path(), &f).unwrap();
        assert_eq!(det.files, 2);
        assert!(C.detect(dir.path(), &f).is_none());
        assert_eq!(Cpp.build_commands(dir.path(), &f).len(), 2);
        assert_eq!(Cpp.discover_tests(dir.path(), &f)[0].program, "ctest");
    }

    #[test]
    fn python_and_js() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[tool.pytest.ini_options]\n[tool.ruff]\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{"scripts":{"test":"vitest","lint":"eslint ."}}"#,
        )
        .unwrap();
        let f = files(&[
            "pyproject.toml",
            "pkg/m.py",
            "package.json",
            "pnpm-lock.yaml",
            "web/a.ts",
        ]);
        assert_eq!(
            Python.discover_tests(dir.path(), &f)[0].display(),
            "python -m pytest"
        );
        assert_eq!(Python.discover_checks(dir.path(), &f)[0].program, "ruff");
        assert_eq!(
            JavaScript.discover_tests(dir.path(), &f)[0].display(),
            "pnpm run test"
        );
        assert_eq!(JavaScript.discover_checks(dir.path(), &f).len(), 1);
        assert!(JavaScript.build_commands(dir.path(), &f).is_empty());
    }
}
