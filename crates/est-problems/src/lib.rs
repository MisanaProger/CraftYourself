//! Problems from online judges (LeetCode, Codeforces, …) for a guided
//! analysis (разбор задачи).
//!
//! The learner pastes a link, the statement and the sample tests, or asks
//! EST to read them from the judge ([`html`] turns the judge's HTML into
//! the same input). This crate is pure: it recognises the link,
//! normalises and limits the statement and samples, and describes the
//! workspace the learner solves the problem in — a solution template in
//! the chosen language and a script that checks it against the samples in
//! the sandbox.
//!
//! Two kinds of problems differ in how they are checked:
//!
//! * **stdin/stdout** (Codeforces and most judges): `run_samples.sh` runs the
//!   program on every `samples/NN.in` and compares with `NN.out`;
//! * **function** (LeetCode): the learner implements a method; the examples
//!   become unit tests the learner writes.

use serde::{Deserialize, Serialize};

pub mod html;
pub mod search;

pub const MAX_STATEMENT_BYTES: usize = 24 * 1024;
pub const MAX_SAMPLES: usize = 10;
pub const MAX_SAMPLE_BYTES: usize = 64 * 1024;
const MAX_TITLE_CHARS: usize = 120;
const MAX_TAGS: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProblemError {
    Invalid(String),
}

impl std::fmt::Display for ProblemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProblemError::Invalid(w) => f.write_str(w),
        }
    }
}

impl std::error::Error for ProblemError {}

type Result<T> = std::result::Result<T, ProblemError>;

fn invalid(why: impl Into<String>) -> ProblemError {
    ProblemError::Invalid(why.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Codeforces,
    Leetcode,
    Other,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Codeforces => "codeforces",
            Platform::Leetcode => "leetcode",
            Platform::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "codeforces" => Some(Platform::Codeforces),
            "leetcode" => Some(Platform::Leetcode),
            "other" => Some(Platform::Other),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Platform::Codeforces => "Codeforces",
            Platform::Leetcode => "LeetCode",
            Platform::Other => "",
        }
    }

    /// LeetCode problems are methods to implement, not programs.
    pub fn is_function_style(self) -> bool {
        self == Platform::Leetcode
    }
}

/// A recognised problem link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProblemRef {
    pub platform: Platform,
    /// `4A` on Codeforces, `two-sum` on LeetCode.
    pub id: String,
    /// Canonical link.
    pub url: String,
}

fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Recognises Codeforces and LeetCode links; any other http(s) link is an
/// `Other` problem identified by its last path segment.
pub fn parse_url(raw: &str) -> Result<ProblemRef> {
    let url = url::Url::parse(raw.trim()).map_err(|e| invalid(format!("link: {e}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(invalid("link must be http(s)"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(invalid("link must not contain credentials"));
    }
    let host = url.host_str().unwrap_or("").to_ascii_lowercase();
    let host = host
        .strip_prefix("www.")
        .or_else(|| host.strip_prefix("m."))
        .unwrap_or(&host)
        .to_owned();
    let seg: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|x| !x.is_empty()).collect())
        .unwrap_or_default();
    let digits = |s: &str| !s.is_empty() && s.len() <= 7 && s.chars().all(|c| c.is_ascii_digit());
    let index =
        |s: &str| !s.is_empty() && s.len() <= 4 && s.chars().all(|c| c.is_ascii_alphanumeric());
    if matches!(
        host.as_str(),
        "codeforces.com" | "codeforces.ru" | "mirror.codeforces.com"
    ) {
        let (gym, contest, problem) = match seg.as_slice() {
            ["problemset", "problem", c, i, ..] | ["contest", c, "problem", i, ..] => {
                (false, *c, *i)
            }
            ["gym", c, "problem", i, ..] | ["problemset", "gymProblem", c, i, ..] => (true, *c, *i),
            _ => return Err(invalid("not a Codeforces problem link")),
        };
        if !digits(contest) || !index(problem) {
            return Err(invalid("not a Codeforces problem link"));
        }
        let problem = problem.to_ascii_uppercase();
        let url = if gym {
            format!("https://codeforces.com/gym/{contest}/problem/{problem}")
        } else {
            format!("https://codeforces.com/problemset/problem/{contest}/{problem}")
        };
        return Ok(ProblemRef {
            platform: Platform::Codeforces,
            id: format!("{contest}{problem}"),
            url,
        });
    }
    if matches!(host.as_str(), "leetcode.com" | "leetcode.cn") {
        let slug = match seg.as_slice() {
            ["problems", slug, ..] => slug.to_ascii_lowercase(),
            _ => return Err(invalid("not a LeetCode problem link")),
        };
        if !is_slug(&slug) {
            return Err(invalid("not a LeetCode problem link"));
        }
        return Ok(ProblemRef {
            platform: Platform::Leetcode,
            url: format!("https://{host}/problems/{slug}/"),
            id: slug,
        });
    }
    let id = slugify(seg.last().copied().unwrap_or(""));
    Ok(ProblemRef {
        platform: Platform::Other,
        id: if id.is_empty() { slugify(&host) } else { id },
        url: url.to_string(),
    })
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 60 {
            break;
        }
    }
    out.trim_end_matches('-').to_owned()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    pub input: String,
    pub output: String,
}

/// What the learner pasted (or what was read from the judge for the
/// learner to review).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProblemInput {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    pub statement: String,
    #[serde(default)]
    pub samples: Vec<Sample>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Codeforces rating (800–3500).
    #[serde(default)]
    pub rating: Option<u32>,
    /// LeetCode difficulty: `easy`, `medium`, `hard`.
    #[serde(default)]
    pub difficulty: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Problem {
    pub platform: Platform,
    pub id: String,
    pub url: Option<String>,
    pub title: String,
    /// Markdown; Codeforces' `$$$…$$$` formulas become `$…$`.
    pub statement: String,
    pub samples: Vec<Sample>,
    pub tags: Vec<String>,
    pub rating: Option<u32>,
    pub difficulty: Option<String>,
}

/// Codeforces writes formulas as `$$$x$$$` (inline) and `$$$$$$x$$$$$$`
/// (display); the interface renders `$x$` and `$$x$$`.
pub fn normalize_math(text: &str) -> String {
    text.replace("$$$$$$", "\u{1}")
        .replace("$$$", "$")
        .replace('\u{1}', "$$")
}

fn tidy(text: &str) -> String {
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.join("\n").trim_matches('\n').to_owned()
}

/// Checks and normalises a pasted problem.
pub fn normalize(input: &ProblemInput) -> Result<Problem> {
    let reference = match input
        .url
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        Some(u) => Some(parse_url(u)?),
        None => None,
    };
    let statement = normalize_math(&tidy(&input.statement.replace('\r', "")));
    if statement.chars().filter(|c| !c.is_whitespace()).count() < 20 {
        return Err(invalid("paste the problem statement"));
    }
    if statement.len() > MAX_STATEMENT_BYTES {
        return Err(invalid(format!(
            "the statement is longer than {} KB",
            MAX_STATEMENT_BYTES / 1024
        )));
    }
    let title = input
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            statement
                .lines()
                .map(|l| l.trim().trim_start_matches('#').trim())
                .find(|l| !l.is_empty())
                .map(str::to_owned)
        })
        .unwrap_or_default()
        .chars()
        .take(MAX_TITLE_CHARS)
        .collect::<String>();
    let mut samples = Vec::new();
    for s in &input.samples {
        let (i, o) = (
            tidy(&s.input.replace('\r', "")),
            tidy(&s.output.replace('\r', "")),
        );
        if i.is_empty() && o.is_empty() {
            continue;
        }
        if i.len() > MAX_SAMPLE_BYTES || o.len() > MAX_SAMPLE_BYTES {
            return Err(invalid(format!(
                "a sample is longer than {} KB",
                MAX_SAMPLE_BYTES / 1024
            )));
        }
        if samples.len() == MAX_SAMPLES {
            return Err(invalid(format!("at most {MAX_SAMPLES} samples")));
        }
        samples.push(Sample {
            input: i,
            output: o,
        });
    }
    let mut tags: Vec<String> = Vec::new();
    for t in &input.tags {
        let t: String = t
            .trim()
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-'))
            .take(40)
            .collect();
        if !t.is_empty() && !tags.contains(&t) && tags.len() < MAX_TAGS {
            tags.push(t);
        }
    }
    let (platform, id, url) = match reference {
        Some(r) => (r.platform, r.id, Some(r.url)),
        None => {
            let id = slugify(&title);
            (
                Platform::Other,
                if id.is_empty() { "problem".into() } else { id },
                None,
            )
        }
    };
    Ok(Problem {
        title: if title.is_empty() { id.clone() } else { title },
        platform,
        id,
        url,
        statement,
        samples,
        tags,
        rating: input.rating.filter(|r| (800..=3500).contains(r)),
        difficulty: input
            .difficulty
            .as_deref()
            .map(|d| d.trim().to_ascii_lowercase())
            .filter(|d| matches!(d.as_str(), "easy" | "medium" | "hard")),
    })
}

impl Problem {
    /// `Codeforces 4A · Watermelon`.
    pub fn task_title(&self) -> String {
        match self.platform {
            Platform::Other => self.title.clone(),
            p => format!("{} {} · {}", p.label(), self.id, self.title),
        }
    }

    pub fn difficulty_level(&self) -> est_domain::Difficulty {
        use est_domain::Difficulty::*;
        match (self.rating, self.difficulty.as_deref()) {
            (Some(r), _) if r < 1300 => Beginner,
            (Some(r), _) if r < 2000 => Intermediate,
            (Some(_), _) => Advanced,
            (None, Some("easy")) => Beginner,
            (None, Some("hard")) => Advanced,
            _ => Intermediate,
        }
    }

    /// Directory name of the problem's practice project.
    pub fn workspace_name(&self, lang: Lang) -> String {
        format!(
            "problem-{}-{}-{}",
            self.platform.as_str(),
            slugify(&self.id),
            lang.as_str()
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    Python,
    Cpp,
    Rust,
}

impl Lang {
    pub fn parse(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "python" | "py" => Ok(Lang::Python),
            "cpp" | "c++" => Ok(Lang::Cpp),
            "rust" | "rs" => Ok(Lang::Rust),
            other => Err(invalid(format!(
                "language `{other}` is not supported: python, cpp or rust"
            ))),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Lang::Python => "python",
            Lang::Cpp => "cpp",
            Lang::Rust => "rust",
        }
    }
}

/// Texts in the learner's language (`ru`, `en`, `uz`).
fn t(ui: &str, ru: &'static str, en: &'static str, uz: &'static str) -> &'static str {
    match ui {
        "en" => en,
        "uz" => uz,
        _ => ru,
    }
}

pub fn learning_goals(ui: &str) -> Vec<String> {
    vec![
        t(
            ui,
            "Разобрать условие и ограничения",
            "Understand the statement and the constraints",
            "Shart va cheklovlarni tahlil qilish",
        )
        .into(),
        t(
            ui,
            "Выбрать подход и оценить сложность",
            "Choose an approach and estimate its complexity",
            "Yondashuvni tanlash va murakkablikni baholash",
        )
        .into(),
        t(
            ui,
            "Проверить решение на примерах и граничных случаях",
            "Check the solution on samples and edge cases",
            "Yechimni misollar va chegaraviy holatlarda tekshirish",
        )
        .into(),
    ]
}

pub fn acceptance_criteria(p: &Problem, ui: &str) -> Vec<String> {
    let first = if p.platform.is_function_style() {
        t(
            ui,
            "Примеры из условия оформлены как тесты, и тесты проходят",
            "The examples from the statement are written as tests, and the tests pass",
            "Shartdagi misollar test sifatida yozilgan va testlar o‘tadi",
        )
    } else {
        t(
            ui,
            "Решение проходит все примеры из условия (тесты в песочнице)",
            "The solution passes every sample from the statement (tests in the sandbox)",
            "Yechim shartdagi barcha misollardan o‘tadi (sandboxdagi testlar)",
        )
    };
    vec![
        first.into(),
        t(
            ui,
            "Сложность алгоритма укладывается в ограничения задачи",
            "The algorithm's complexity fits the problem's limits",
            "Algoritm murakkabligi masala cheklovlariga mos keladi",
        )
        .into(),
        t(
            ui,
            "Разобраны граничные случаи: минимальные и максимальные входные данные",
            "Edge cases are covered: the smallest and the largest inputs",
            "Chegaraviy holatlar ko‘rib chiqilgan: eng kichik va eng katta kirish ma’lumotlari",
        )
        .into(),
        t(
            ui,
            "Ты можешь объяснить, почему решение верно",
            "You can explain why the solution is correct",
            "Yechim nima uchun to‘g‘ri ekanini tushuntira olasiz",
        )
        .into(),
    ]
}

/// The command that checks the solution in the sandbox.
pub fn test_command(p: &Problem, lang: Lang) -> String {
    if !p.platform.is_function_style() {
        return "sh run_samples.sh".into();
    }
    match lang {
        Lang::Python => "python3 -m unittest -v".into(),
        Lang::Cpp => "sh run_tests.sh".into(),
        Lang::Rust => "cargo test --offline".into(),
    }
}

fn problem_md(p: &Problem, ui: &str) -> String {
    let mut s = format!("# {}\n\n", p.task_title());
    if let Some(u) = &p.url {
        s.push_str(&format!("<{u}>\n\n"));
    }
    s.push_str(&p.statement);
    s.push('\n');
    if !p.samples.is_empty() {
        s.push_str(&format!(
            "\n## {}\n",
            t(ui, "Примеры", "Samples", "Misollar")
        ));
        for (i, x) in p.samples.iter().enumerate() {
            s.push_str(&format!(
                "\n### {}\n\n```\n{}\n```\n\n```\n{}\n```\n",
                i + 1,
                x.input,
                x.output
            ));
        }
    }
    s
}

const RUN_SAMPLES: &str = r#"#!/bin/sh
# Runs the solution on every samples/NN.in and compares its output with
# samples/NN.out. Trailing spaces and blank lines at the end are ignored.
# Add your own cases as new pairs of files.
set -u
norm() {
  awk '{ sub(/[ \t\r]+$/, ""); line[NR] = $0 }
       END { n = NR; while (n > 0 && line[n] == "") n--; for (i = 1; i <= n; i++) print line[i] }' "$1"
}
@BUILD@
passed=0
failed=0
for input in samples/*.in; do
  [ -e "$input" ] || continue
  name=$(basename "$input" .in)
  expected="samples/$name.out"
  if timeout 10 @RUN@ < "$input" > /tmp/est-actual 2> /tmp/est-stderr; then
    if [ "$(norm /tmp/est-actual)" = "$(norm "$expected")" ]; then
      passed=$((passed + 1))
      echo "sample $name: ok"
      continue
    fi
    echo "sample $name: wrong answer"
  else
    echo "sample $name: runtime error, time limit or crash (exit $?)"
    head -n 20 /tmp/est-stderr
  fi
  failed=$((failed + 1))
  echo "  input:";    head -n 20 "$input"    | sed 's/^/    /'
  echo "  expected:"; head -n 20 "$expected" | sed 's/^/    /'
  echo "  got:";      head -n 20 /tmp/est-actual | sed 's/^/    /'
done
echo "est-samples: $passed passed, $failed failed"
if [ $((passed + failed)) -eq 0 ]; then
  echo "no samples: add samples/01.in and samples/01.out"
  exit 1
fi
[ "$failed" -eq 0 ]
"#;

/// Files of the problem's practice project, path → contents.
pub fn workspace_files(p: &Problem, lang: Lang, ui: &str) -> Vec<(String, String)> {
    let mut files = vec![("PROBLEM.md".to_owned(), problem_md(p, ui))];
    if p.platform.is_function_style() {
        match lang {
            Lang::Python => {
                files.push(("solution.py".into(), PY_CLASS.into()));
                files.push(("test_solution.py".into(), PY_TESTS.into()));
            }
            Lang::Cpp => {
                files.push(("solution.cpp".into(), CPP_CLASS.into()));
                files.push(("run_tests.sh".into(), CPP_RUN_TESTS.into()));
            }
            Lang::Rust => {
                files.push(("Cargo.toml".into(), cargo_toml(false)));
                files.push(("src/lib.rs".into(), RUST_LIB.into()));
                files.push((".gitignore".into(), "/target\n".into()));
            }
        }
        return files;
    }
    for (i, s) in p.samples.iter().enumerate() {
        files.push((format!("samples/{:02}.in", i + 1), format!("{}\n", s.input)));
        files.push((
            format!("samples/{:02}.out", i + 1),
            format!("{}\n", s.output),
        ));
    }
    let (build, run) = match lang {
        Lang::Python => {
            files.push(("main.py".into(), PY_MAIN.into()));
            ("", "python3 main.py")
        }
        Lang::Cpp => {
            files.push(("main.cpp".into(), CPP_MAIN.into()));
            (
                "g++ -std=c++17 -O2 -pipe -o /tmp/est-solution main.cpp || exit 1",
                "/tmp/est-solution",
            )
        }
        Lang::Rust => {
            files.push(("Cargo.toml".into(), cargo_toml(true)));
            files.push(("src/main.rs".into(), RUST_MAIN.into()));
            files.push((".gitignore".into(), "/target\n".into()));
            (
                "cargo build --release --quiet --offline || exit 1",
                "cargo run --release --quiet --offline",
            )
        }
    };
    files.push((
        "run_samples.sh".into(),
        RUN_SAMPLES.replace("@BUILD@", build).replace("@RUN@", run),
    ));
    files
}

fn cargo_toml(bin: bool) -> String {
    let target = if bin {
        ""
    } else {
        "\n[lib]\npath = \"src/lib.rs\"\n"
    };
    format!(
        "[package]\nname = \"solution\"\nversion = \"0.1.0\"\nedition = \"2021\"\n{target}\n[dependencies]\n"
    )
}

const PY_MAIN: &str = r#"import sys


def main() -> None:
    data = sys.stdin.read().split()
    # Read the input from `data`, solve, print the answer.


if __name__ == "__main__":
    main()
"#;

const CPP_MAIN: &str = r#"#include <bits/stdc++.h>
using namespace std;

int main() {
    ios::sync_with_stdio(false);
    cin.tie(nullptr);
    // Read the input, solve, print the answer.
    return 0;
}
"#;

const RUST_MAIN: &str = r#"use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut tokens = input.split_ascii_whitespace();
    let mut out = io::BufWriter::new(io::stdout().lock());
    // Read the input from `tokens`, solve, write the answer to `out`.
    let _ = (&mut tokens, &mut out);
    out.flush().unwrap();
}
"#;

const PY_CLASS: &str = r#"from typing import List, Optional


class Solution:
    # Copy the method signature from the problem page and implement it.
    pass
"#;

const PY_TESTS: &str = r#"import unittest

from solution import Solution


class SolutionTest(unittest.TestCase):
    # Turn every example from PROBLEM.md into a test, then add edge cases.
    def test_example_1(self):
        self.skipTest("write this test from the first example")


if __name__ == "__main__":
    unittest.main()
"#;

const CPP_CLASS: &str = r#"#include <bits/stdc++.h>
using namespace std;

class Solution {
public:
    // Copy the method signature from the problem page and implement it.
};

int main() {
    Solution s;
    // Turn every example from PROBLEM.md into an assert, then add edge cases:
    // assert(s.method(args) == expected);
    (void)s;
    cout << "all checks passed\n";
    return 0;
}
"#;

const CPP_RUN_TESTS: &str = r#"#!/bin/sh
# Builds solution.cpp with assertions enabled and runs its checks.
set -e
g++ -std=c++17 -O2 -pipe -o /tmp/est-tests solution.cpp
timeout 10 /tmp/est-tests
"#;

const RUST_LIB: &str = r#"pub struct Solution;

impl Solution {
    // Copy the method signature from the problem page and implement it.
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    // Turn every example from PROBLEM.md into a test, then add edge cases.
    #[test]
    #[ignore = "write this test from the first example"]
    fn example_1() {}
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_links() {
        for (link, id, url) in [
            (
                "https://codeforces.com/problemset/problem/4/a",
                "4A",
                "https://codeforces.com/problemset/problem/4/A",
            ),
            (
                "http://www.codeforces.com/contest/1850/problem/C?locale=ru",
                "1850C",
                "https://codeforces.com/problemset/problem/1850/C",
            ),
            (
                "https://codeforces.ru/gym/104/problem/B1",
                "104B1",
                "https://codeforces.com/gym/104/problem/B1",
            ),
        ] {
            let r = parse_url(link).unwrap();
            assert_eq!(
                (r.platform, r.id.as_str(), r.url.as_str()),
                (Platform::Codeforces, id, url)
            );
        }
        let r = parse_url("https://leetcode.com/problems/two-sum/description/?envType=x").unwrap();
        assert_eq!(
            (r.platform, r.id.as_str(), r.url.as_str()),
            (
                Platform::Leetcode,
                "two-sum",
                "https://leetcode.com/problems/two-sum/"
            )
        );
        let r = parse_url("https://atcoder.jp/contests/abc300/tasks/abc300_a").unwrap();
        assert_eq!((r.platform, r.id.as_str()), (Platform::Other, "abc300-a"));
        for bad in [
            "ftp://codeforces.com/problemset/problem/4/A",
            "https://codeforces.com/blog/entry/1",
            "https://codeforces.com/problemset/problem/x/A",
            "https://leetcode.com/contest/weekly",
            "https://user:pw@leetcode.com/problems/a/",
            "not a link",
        ] {
            assert!(parse_url(bad).is_err(), "{bad}");
        }
    }

    fn cf() -> ProblemInput {
        ProblemInput {
            url: Some("https://codeforces.com/problemset/problem/4/A".into()),
            title: None,
            statement: "# Watermelon\r\n\r\nGiven weight $$$w$$$ ($$$1 \\le w \\le 100$$$), say YES if it splits into two even parts.   \n\n$$$$$$w = a + b$$$$$$\n".into(),
            samples: vec![
                Sample { input: "8 \r\n".into(), output: "YES\n\n".into() },
                Sample { input: "  ".into(), output: "".into() },
            ],
            tags: vec!["Math".into(), "brute force".into(), "math".into(), "<b>".into()],
            rating: Some(800),
            difficulty: None,
        }
    }

    #[test]
    fn normalises_a_codeforces_problem() {
        let p = normalize(&cf()).unwrap();
        assert_eq!(p.task_title(), "Codeforces 4A · Watermelon");
        assert!(p.statement.contains("weight $w$ ($1 \\le w \\le 100$)"));
        assert!(p.statement.contains("$$w = a + b$$") && !p.statement.contains("   \n"));
        assert_eq!(
            p.samples,
            vec![Sample {
                input: "8".into(),
                output: "YES".into()
            }]
        );
        assert_eq!(p.tags, ["math", "brute force", "b"]);
        assert_eq!(p.difficulty_level(), est_domain::Difficulty::Beginner);
        assert_eq!(p.workspace_name(Lang::Cpp), "problem-codeforces-4a-cpp");
        assert!(
            normalize(&ProblemInput {
                statement: "short".into(),
                ..cf()
            })
            .is_err()
        );
        let many = ProblemInput {
            samples: vec![
                Sample {
                    input: "1".into(),
                    output: "1".into()
                };
                11
            ],
            ..cf()
        };
        assert!(normalize(&many).is_err());
    }

    #[test]
    fn workspaces() {
        let p = normalize(&cf()).unwrap();
        let files = workspace_files(&p, Lang::Python, "en");
        let names: Vec<_> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "PROBLEM.md",
                "samples/01.in",
                "samples/01.out",
                "main.py",
                "run_samples.sh"
            ]
        );
        let script = &files[4].1;
        assert!(
            script.contains("timeout 10 python3 main.py < \"$input\"") && !script.contains('@')
        );
        assert!(files[0].1.contains("## Samples"));
        assert_eq!(test_command(&p, Lang::Rust), "sh run_samples.sh");

        let lc = normalize(&ProblemInput {
            url: Some("https://leetcode.com/problems/two-sum/".into()),
            title: Some("Two Sum".into()),
            difficulty: Some("Easy".into()),
            rating: None,
            ..cf()
        })
        .unwrap();
        assert_eq!(lc.difficulty_level(), est_domain::Difficulty::Beginner);
        let names: Vec<_> = workspace_files(&lc, Lang::Rust, "ru")
            .into_iter()
            .map(|(n, _)| n)
            .collect();
        assert_eq!(
            names,
            ["PROBLEM.md", "Cargo.toml", "src/lib.rs", ".gitignore"]
        );
        assert_eq!(test_command(&lc, Lang::Python), "python3 -m unittest -v");
        assert!(acceptance_criteria(&lc, "ru")[0].contains("тесты"));
        assert!(Lang::parse("java").is_err());
    }
}
