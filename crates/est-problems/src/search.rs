//! Searching the judges for problems (discovery).
//!
//! A search is built from validated parts: tags from the judges' own lists,
//! a rating window or a difficulty. The judge returns problems; this module
//! picks the ones to offer — within the window, not imported yet, the most
//! solved first (a widely solved problem is a well-tested one).

use serde::{Deserialize, Serialize};

use crate::Platform;

/// Codeforces' problem tags.
pub const CODEFORCES_TAGS: &[&str] = &[
    "2-sat",
    "binary search",
    "bitmasks",
    "brute force",
    "chinese remainder theorem",
    "combinatorics",
    "constructive algorithms",
    "data structures",
    "dfs and similar",
    "divide and conquer",
    "dp",
    "dsu",
    "expression parsing",
    "fft",
    "flows",
    "games",
    "geometry",
    "graph matchings",
    "graphs",
    "greedy",
    "hashing",
    "implementation",
    "interactive",
    "math",
    "matrices",
    "meet-in-the-middle",
    "number theory",
    "probabilities",
    "schedules",
    "shortest paths",
    "sortings",
    "string suffix structures",
    "strings",
    "ternary search",
    "trees",
    "two pointers",
];

/// LeetCode's topic slugs (algorithmic ones; no SQL or shell).
pub const LEETCODE_TAGS: &[&str] = &[
    "array",
    "string",
    "hash-table",
    "dynamic-programming",
    "math",
    "sorting",
    "greedy",
    "depth-first-search",
    "breadth-first-search",
    "binary-search",
    "tree",
    "binary-tree",
    "binary-search-tree",
    "matrix",
    "two-pointers",
    "bit-manipulation",
    "heap-priority-queue",
    "stack",
    "queue",
    "prefix-sum",
    "graph",
    "simulation",
    "design",
    "counting",
    "backtracking",
    "sliding-window",
    "union-find",
    "linked-list",
    "ordered-set",
    "monotonic-stack",
    "monotonic-queue",
    "trie",
    "recursion",
    "divide-and-conquer",
    "memoization",
    "topological-sort",
    "segment-tree",
    "binary-indexed-tree",
    "geometry",
    "number-theory",
    "combinatorics",
    "shortest-path",
    "bitmask",
    "game-theory",
    "interactive",
    "string-matching",
    "rolling-hash",
    "hash-function",
    "minimum-spanning-tree",
    "strongly-connected-component",
    "eulerian-circuit",
    "biconnected-component",
    "quickselect",
    "bucket-sort",
    "counting-sort",
    "radix-sort",
    "merge-sort",
    "randomized",
    "probability-and-statistics",
    "reservoir-sampling",
    "line-sweep",
    "doubly-linked-list",
    "data-stream",
    "iterator",
    "enumeration",
    "brainteaser",
];

pub const MAX_TAGS: usize = 3;

/// A validated problem search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProblemQuery {
    pub platform: Platform,
    /// Tags as the platform spells them.
    pub tags: Vec<String>,
    /// Codeforces rating window.
    pub rating_min: Option<u32>,
    pub rating_max: Option<u32>,
    /// LeetCode: `easy`, `medium` or `hard`.
    pub difficulty: Option<String>,
}

impl ProblemQuery {
    /// Codeforces: known tags (`dynamic programming` and `dp-…` spellings
    /// are matched loosely), a target rating rounded to 100 becomes the
    /// window `[r − 100, r + 200]`.
    pub fn codeforces(tags: &[String], rating: Option<u32>) -> Self {
        let tags = pick_tags(tags, CODEFORCES_TAGS, |t| t.replace(['-', '_'], " "));
        let (rating_min, rating_max) = match rating {
            Some(r) => {
                let r = (r.clamp(800, 3500) + 50) / 100 * 100;
                (
                    Some(r.saturating_sub(100).max(800)),
                    Some((r + 200).min(3500)),
                )
            }
            None => (None, None),
        };
        Self {
            platform: Platform::Codeforces,
            tags,
            rating_min,
            rating_max,
            difficulty: None,
        }
    }

    /// LeetCode: known topic slugs; a difficulty among easy/medium/hard.
    pub fn leetcode(tags: &[String], difficulty: Option<&str>) -> Self {
        let tags = pick_tags(tags, LEETCODE_TAGS, |t| t.replace([' ', '_'], "-"));
        let difficulty = difficulty
            .map(|d| d.trim().to_ascii_lowercase())
            .filter(|d| matches!(d.as_str(), "easy" | "medium" | "hard"));
        Self {
            platform: Platform::Leetcode,
            tags,
            rating_min: None,
            rating_max: None,
            difficulty,
        }
    }
}

fn pick_tags(tags: &[String], known: &[&str], spell: impl Fn(&str) -> String) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = spell(t.trim().to_ascii_lowercase().as_str());
        let t = match t.as_str() {
            "dynamic programming" => "dp".to_owned(),
            "dp" if known.contains(&"dynamic-programming") => "dynamic-programming".to_owned(),
            _ => t,
        };
        if known.contains(&t.as_str()) && !out.contains(&t) && out.len() < MAX_TAGS {
            out.push(t);
        }
    }
    out
}

/// A problem returned by a judge's search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundProblem {
    pub platform: Platform,
    /// `4A` on Codeforces, the slug on LeetCode — as in [`crate::ProblemRef`].
    pub id: String,
    pub url: String,
    pub title: String,
    pub tags: Vec<String>,
    pub rating: Option<u32>,
    pub difficulty: Option<String>,
    /// How many people solved it (Codeforces) or the acceptance rate in
    /// percent (LeetCode): higher first.
    pub popularity: Option<u64>,
    #[serde(default)]
    pub paid_only: bool,
}

/// Picks up to `limit` problems: inside the rating window, free, not in
/// `exclude` (ids already imported), the most popular first.
pub fn select(
    found: Vec<FoundProblem>,
    query: &ProblemQuery,
    exclude: &[String],
    limit: usize,
) -> Vec<FoundProblem> {
    let mut picked: Vec<FoundProblem> = found
        .into_iter()
        .filter(|p| p.platform == query.platform && !p.paid_only)
        .filter(|p| !exclude.contains(&p.id))
        .filter(|p| match (query.rating_min, query.rating_max) {
            (None, None) => true,
            (min, max) => p
                .rating
                .is_some_and(|r| min.is_none_or(|m| r >= m) && max.is_none_or(|m| r <= m)),
        })
        .filter(|p| {
            query.difficulty.is_none() || p.difficulty.as_deref() == query.difficulty.as_deref()
        })
        .collect();
    picked.sort_by_key(|p| std::cmp::Reverse(p.popularity.unwrap_or(0)));
    let mut seen = std::collections::HashSet::new();
    picked.retain(|p| seen.insert(p.id.clone()));
    picked.truncate(limit);
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cf(id: &str, rating: Option<u32>, solved: u64) -> FoundProblem {
        FoundProblem {
            platform: Platform::Codeforces,
            id: id.into(),
            url: format!("https://codeforces.com/problemset/problem/{id}"),
            title: id.into(),
            tags: vec!["dp".into()],
            rating,
            difficulty: None,
            popularity: Some(solved),
            paid_only: false,
        }
    }

    #[test]
    fn validates_queries() {
        let q = ProblemQuery::codeforces(
            &[
                "Dynamic Programming".into(),
                "binary-search".into(),
                "magic".into(),
                "dp".into(),
            ],
            Some(1234),
        );
        assert_eq!(q.tags, ["dp", "binary search"]);
        assert_eq!((q.rating_min, q.rating_max), (Some(1100), Some(1400)));
        assert_eq!(
            ProblemQuery::codeforces(&[], Some(100)).rating_min,
            Some(800)
        );
        let q = ProblemQuery::leetcode(
            &["Hash Table".into(), "dp".into(), "sql".into()],
            Some("MEDIUM"),
        );
        assert_eq!(q.tags, ["hash-table", "dynamic-programming"]);
        assert_eq!(q.difficulty.as_deref(), Some("medium"));
        assert_eq!(ProblemQuery::leetcode(&[], Some("insane")).difficulty, None);
    }

    #[test]
    fn selects_popular_problems_in_the_window() {
        let q = ProblemQuery::codeforces(&["dp".into()], Some(1200));
        let found = vec![
            cf("1A", Some(1200), 10),
            cf("2A", Some(1300), 500),
            cf("3A", Some(2000), 9000),
            cf("4A", None, 9000),
            cf("5A", Some(1100), 100),
            cf("2A", Some(1300), 500),
        ];
        let picked = select(found, &q, &["5A".into()], 5);
        let ids: Vec<_> = picked.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["2A", "1A"]);
    }
}
