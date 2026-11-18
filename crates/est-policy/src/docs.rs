//! Documentation Router: turns symbols into precise links to primary
//! sources (spec 4.3: "направлять к конкретному месту, а не на главную").

use serde::{Deserialize, Serialize};
use url::Url;

/// A documentation source declared by the exercise manifest.
#[derive(Debug, Clone, Deserialize)]
pub struct DocSource {
    pub title: String,
    pub url: String,
    pub topics: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DocLinkKind {
    /// Curated link from the exercise author.
    Curated,
    /// Definition in the project's own source code.
    Source,
    /// Search in the official API reference of the language/ecosystem.
    ReferenceSearch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DocLink {
    pub kind: DocLinkKind,
    pub title: String,
    /// URL, or `path:line` for [`DocLinkKind::Source`].
    pub target: String,
    pub symbol: Option<String>,
}

pub struct DocRouter<'a> {
    pub language: &'a str,
    pub sources: &'a [DocSource],
}

const RUST_STD_ROOTS: &[&str] = &["std", "core", "alloc"];
const RUST_PRELUDE_TYPES: &[&str] = &[
    "Vec",
    "String",
    "str",
    "Option",
    "Result",
    "Box",
    "HashMap",
    "HashSet",
    "BTreeMap",
    "BTreeSet",
    "VecDeque",
    "Rc",
    "Arc",
    "RefCell",
    "Cell",
    "Mutex",
    "RwLock",
    "Iterator",
    "IntoIterator",
    "slice",
    "char",
    "Cow",
    "Path",
    "PathBuf",
];

impl DocRouter<'_> {
    /// Links for the given symbols, most specific first. Source-code
    /// locations are added separately by the caller (see
    /// [`definition_patterns`]), since this crate has no file access.
    pub fn links(&self, symbols: &[String]) -> Vec<DocLink> {
        let mut out = Vec::new();
        for sym in symbols {
            let needle = sym.to_lowercase();
            let last = needle
                .rsplit([':', '.'])
                .next()
                .unwrap_or(&needle)
                .to_owned();
            for src in self.sources {
                let hit = src.topics.iter().any(|t| {
                    let t = t.to_lowercase();
                    t == needle || t == last || needle.ends_with(&format!("::{t}"))
                });
                if hit && !out.iter().any(|l: &DocLink| l.target == src.url) {
                    out.push(DocLink {
                        kind: DocLinkKind::Curated,
                        title: src.title.clone(),
                        target: src.url.clone(),
                        symbol: Some(sym.clone()),
                    });
                }
            }
            if let Some(link) = self.reference_search(sym) {
                out.push(link);
            }
        }
        out
    }

    fn reference_search(&self, sym: &str) -> Option<DocLink> {
        let (title, url) = match self.language.to_ascii_lowercase().as_str() {
            "rust" => {
                let first = sym.split("::").next().unwrap_or(sym);
                let first_type = sym.split(['.', ':']).next().unwrap_or(sym);
                if RUST_STD_ROOTS.contains(&first) || RUST_PRELUDE_TYPES.contains(&first_type) {
                    let query = sym.replace('.', "::");
                    (
                        format!("Rust standard library: `{query}`"),
                        Url::parse_with_params(
                            "https://doc.rust-lang.org/std/index.html",
                            [("search", query.as_str())],
                        ),
                    )
                } else if let Some((krate, rest)) = sym.split_once("::") {
                    let krate = krate.replace('-', "_");
                    (
                        format!("docs.rs: `{sym}`"),
                        Url::parse_with_params(
                            &format!("https://docs.rs/{krate}/latest/{krate}/"),
                            [("search", rest)],
                        ),
                    )
                } else {
                    return None;
                }
            }
            "python" => (
                format!("Python documentation: `{sym}`"),
                Url::parse_with_params("https://docs.python.org/3/search.html", [("q", sym)]),
            ),
            _ => return None,
        };
        Some(DocLink {
            kind: DocLinkKind::ReferenceSearch,
            title,
            target: url.ok()?.to_string(),
            symbol: Some(sym.to_owned()),
        })
    }
}

/// Fixed strings that likely mark the definition of `symbol` in a code base
/// of the given language. The caller greps for them in the workspace.
pub fn definition_patterns(symbol: &str, language: &str) -> Vec<String> {
    let name = symbol.rsplit([':', '.']).next().unwrap_or(symbol);
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Vec::new();
    }
    match language.to_ascii_lowercase().as_str() {
        "rust" => vec![
            format!("fn {name}("),
            format!("fn {name}<"),
            format!("struct {name}"),
            format!("enum {name}"),
            format!("trait {name}"),
            format!("type {name}"),
            format!("macro_rules! {name}"),
        ],
        "python" => vec![format!("def {name}("), format!("class {name}")],
        "go" => vec![format!("func {name}("), format!("type {name} ")],
        "javascript" | "typescript" => vec![
            format!("function {name}("),
            format!("class {name}"),
            format!("const {name} ="),
        ],
        _ => vec![name.to_owned()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources() -> Vec<DocSource> {
        vec![DocSource {
            title: "RangeInclusive".into(),
            url: "https://doc.rust-lang.org/std/ops/struct.RangeInclusive.html".into(),
            topics: vec!["RangeInclusive".into(), "contains".into()],
        }]
    }

    #[test]
    fn curated_links_come_first() {
        let s = sources();
        let r = DocRouter {
            language: "rust",
            sources: &s,
        };
        let links = r.links(&["RangeInclusive::contains".into()]);
        assert_eq!(links[0].kind, DocLinkKind::Curated);
        assert_eq!(links.len(), 2);
    }

    #[test]
    fn std_and_crate_search_links() {
        let r = DocRouter {
            language: "rust",
            sources: &[],
        };
        let std = r.links(&["Vec::retain".into()]);
        assert_eq!(
            std[0].target,
            "https://doc.rust-lang.org/std/index.html?search=Vec%3A%3Aretain"
        );
        let krate = r.links(&["serde_json::from_str".into()]);
        assert_eq!(
            krate[0].target,
            "https://docs.rs/serde_json/latest/serde_json/?search=from_str"
        );
        assert!(r.links(&["parse_range".into()]).is_empty());
    }

    #[test]
    fn definition_patterns_by_language() {
        assert!(definition_patterns("crate::parser::parse_range", "rust")
            .contains(&"fn parse_range(".to_owned()));
        assert!(definition_patterns("obj.load", "python").contains(&"def load(".to_owned()));
        assert!(definition_patterns("a b", "rust").is_empty());
    }
}
