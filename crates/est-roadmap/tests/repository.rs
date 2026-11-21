//! The repository's own roadmap: no integrity errors, views up to date, and
//! the JSON Schema in step with the code. Runs in `cargo test --workspace`,
//! so CI enforces it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use est_roadmap::{Kind, Severity, Status, load, render, validate};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn roadmap_has_no_integrity_errors() {
    let root = root();
    let rm = load(&root).expect("roadmap/ parses");
    let findings = validate(&rm, &|p| root.join(p).exists());
    let errors: Vec<String> = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .map(ToString::to_string)
        .collect();
    assert!(
        errors.is_empty(),
        "roadmap errors (run `cargo run -p est-roadmap -- check`):\n{}",
        errors.join("\n")
    );
}

#[test]
fn generated_views_are_up_to_date() {
    let root = root();
    let rm = load(&root).expect("roadmap/ parses");
    let mut stale = Vec::new();
    for (path, contents) in render::views(&rm) {
        if std::fs::read_to_string(root.join(&path)).unwrap_or_default() != contents {
            stale.push(path);
        }
    }
    let summary =
        std::fs::read_to_string(root.join(&rm.config.summary_file)).expect("summary file");
    let spliced = render::splice_summary(&summary, &render::summary_block(&rm))
        .expect("summary file has the generated block markers");
    if spliced != summary {
        stale.push(rm.config.summary_file.clone());
    }
    assert!(
        stale.is_empty(),
        "out of date: {} — run `cargo run -p est-roadmap -- render`",
        stale.join(", ")
    );
}

fn enum_values(schema: &serde_json::Value, def: &str) -> BTreeSet<String> {
    schema["$defs"][def]["enum"]
        .as_array()
        .unwrap_or_else(|| panic!("$defs.{def}.enum"))
        .iter()
        .map(|v| v.as_str().expect("string").to_string())
        .collect()
}

fn serialized<T: serde::Serialize>(values: &[T]) -> BTreeSet<String> {
    values
        .iter()
        .map(|v| {
            serde_json::to_value(v)
                .expect("serialises")
                .as_str()
                .expect("string")
                .to_string()
        })
        .collect()
}

#[test]
fn schema_matches_the_code() {
    let text = std::fs::read_to_string(root().join("docs/schemas/roadmap-schema.json"))
        .expect("schema exists");
    let schema: serde_json::Value = serde_json::from_str(&text).expect("schema is JSON");

    assert_eq!(enum_values(&schema, "status"), serialized(&Status::ALL));
    assert_eq!(
        enum_values(&schema, "kind"),
        serialized(&[
            Kind::Program,
            Kind::Epic,
            Kind::Feature,
            Kind::Task,
            Kind::Subtask,
            Kind::Release
        ])
    );

    // Every field of an item, as the code serialises it, is in the schema
    // and vice versa (the export adds `roadmap`).
    let rm = est_roadmap::from_sources(
        r#"
version = 1
views_dir = "v"
summary_file = "S.md"
[[roadmap]]
id = "p"
title = "P"
prefix = "PRD"
file = "p.toml"
description = "d"
"#,
        &[(
            "p.toml",
            r#"
[[item]]
id = "PRD-001"
name = "x"
kind = "program"
status = "planned"
"#,
        )],
    )
    .expect("parses");
    let value = serde_json::to_value(&rm.items[0]).expect("serialises");
    let code: BTreeSet<String> = value.as_object().expect("object").keys().cloned().collect();
    let documented: BTreeSet<String> = schema["$defs"]["item"]["properties"]
        .as_object()
        .expect("item properties")
        .keys()
        .cloned()
        .collect();
    assert_eq!(code, documented);
}
