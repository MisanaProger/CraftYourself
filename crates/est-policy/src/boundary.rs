//! Documentation Boundary Policy (spec 4.3).
//!
//! Questions about what a concrete function / method / API does, which
//! arguments it takes, what it returns or what its limits are must be
//! answered by primary sources. The mentor pipeline therefore never sends
//! such a question to a model; it answers with links built by the
//! [`crate::docs::DocRouter`] instead.
//!
//! Conceptual questions (what is an iterator, why do data races happen) and
//! ordinary mentoring (debugging, hypotheses) are allowed.
//!
//! The classifier is deliberately conservative and rule based: it is cheap,
//! deterministic, auditable and cannot be talked out of its rules.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Largest documentation excerpt a learner may paste for term explanation.
/// Larger pastes would turn the mentor into a page re-teller.
pub const MAX_EXCERPT_CHARS: usize = 1500;

#[derive(Debug, Clone, Deserialize)]
pub struct MentorRequest {
    pub question: String,
    /// Documentation fragment the learner pasted to get terms explained.
    #[serde(default)]
    pub excerpt: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueryKind {
    /// Reference question about a concrete API. `symbols` may be empty when
    /// the question only says "this function".
    ApiReference { symbols: Vec<String> },
    /// Domain concept: allowed to be explained fully.
    Conceptual,
    /// Everything else: debugging, hypotheses, planning. Governed by help levels.
    Mentoring,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum BoundaryDecision {
    /// The question may go to the mentor model.
    Answer,
    /// Do not call a model: reply with links to primary sources.
    Redirect { symbols: Vec<String> },
    /// The learner pasted an excerpt: the model may explain terms and
    /// concepts in it, but must not paraphrase the whole excerpt.
    ExplainExcerpt { max_quote_chars: usize },
    /// Request refused (e.g. excerpt too large).
    Refuse { reason: String },
}

pub fn decide(kind: &QueryKind, req: &MentorRequest) -> BoundaryDecision {
    match (kind, req.excerpt.as_deref().map(str::trim)) {
        (_, Some(ex)) if !ex.is_empty() => {
            if ex.chars().count() > MAX_EXCERPT_CHARS {
                BoundaryDecision::Refuse {
                    reason: format!(
                        "paste at most {MAX_EXCERPT_CHARS} characters: pick the sentence or \
                         paragraph with the term you do not understand"
                    ),
                }
            } else {
                BoundaryDecision::ExplainExcerpt {
                    max_quote_chars: 200,
                }
            }
        }
        (QueryKind::ApiReference { symbols }, _) => BoundaryDecision::Redirect {
            symbols: symbols.clone(),
        },
        _ => BoundaryDecision::Answer,
    }
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).expect("valid regex"));
    };
}

// Always an API-reference question.
re!(
    API_STRONG,
    r"(?ix)
      какие\s+(аргументы|параметры|поля|значения|ошибки|исключения)
    | что\s+(возвращает|принимает)
    | (возвращает|возвращают|принимает|принимают)\b
    | сигнатур
    | в\s+каких\s+случаях\s+(паникует|падает|возвращает)
    | \b(what|which)\s+(arguments|parameters|params|fields|errors|exceptions)\b
    | \breturns?\b
    | \bsignature\b
    | \bdoes\s+\S+\s+panic\b
    | \b(accepts?|takes)\s+(as\s+)?(arguments?|parameters?)\b
    "
);

// An API question only when it names a symbol or an API noun.
re!(
    API_WEAK,
    r"(?ix)
      что\s+делает
    | как\s+(вызвать|использовать|вызывается|применять)
    | какие\s+ограничения
    | \bwhat\s+does\b
    | \bhow\s+(do\s+i|to)\s+(call|use)\b
    | \bwhat\s+are\s+the\s+limits\b
    | \bhow\s+does\s+.+\s+work\b
    | как\s+работает
    "
);

re!(
    API_NOUN,
    r"(?ix)
      функци | метод | макрос | трейт | структур | класс | конструктор | api\b
    | \bfunction | \bmethod | \bmacro | \btrait\b | \bstruct\b | \bclass\b | \bconstructor\b
    "
);

re!(
    CONCEPT,
    r"(?ix)
      что\s+так(ое|ие)
    | как\s+(работает|работают|устроен|устроена|устроены)
    | почему\s+(возникают|возникает|нужен|нужна|нужно)
    | зачем\s+нуж
    | в\s+ч[её]м\s+разница
    | объясни(те)?\s+(концепци|принцип|идею)
    | \bwhat\s+(is|are)\b
    | \bhow\s+does\b.*\bwork\b
    | \bwhy\s+(do|does|is|are)\b
    | \bdifference\s+between\b
    | \bexplain\s+the\s+(concept|idea|principle)\b
    "
);

// Strong symbol evidence: backticks, paths, calls, snake_case identifiers.
re!(BACKTICK, r"`([^`\n]{1,80})`");
re!(
    PATH,
    r"\b[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)+\b"
);
re!(
    CALL,
    r"\b([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)\(\)?"
);
re!(SNAKE, r"\b[a-z][a-z0-9]*(?:_[a-z0-9]+)+\b");
// Weak evidence: CamelCase type names such as HashMap.
re!(CAMEL, r"\b[A-Z][a-z0-9]+(?:[A-Z][a-z0-9]*)+\b");

fn push_unique(out: &mut Vec<String>, s: &str) {
    let s = s.trim().trim_end_matches("()").trim_end_matches('(');
    if !s.is_empty() && !out.iter().any(|x| x == s) {
        out.push(s.to_owned());
    }
}

/// Extracts code symbols mentioned in a question. Returns (strong, weak).
pub fn extract_symbols(text: &str) -> (Vec<String>, Vec<String>) {
    let mut strong = Vec::new();
    for c in BACKTICK.captures_iter(text) {
        push_unique(&mut strong, &c[1]);
    }
    let without_ticks = BACKTICK.replace_all(text, " ");
    for m in PATH.find_iter(&without_ticks) {
        push_unique(&mut strong, m.as_str());
    }
    for c in CALL.captures_iter(&without_ticks) {
        push_unique(&mut strong, &c[1]);
    }
    for m in SNAKE.find_iter(&without_ticks) {
        if !strong.iter().any(|s| s.contains(m.as_str())) {
            push_unique(&mut strong, m.as_str());
        }
    }
    let mut weak = Vec::new();
    for m in CAMEL.find_iter(&without_ticks) {
        if !strong.iter().any(|s| s.contains(m.as_str())) {
            push_unique(&mut weak, m.as_str());
        }
    }
    (strong, weak)
}

pub fn classify(question: &str) -> QueryKind {
    let (strong, weak) = extract_symbols(question);
    let api_strong = API_STRONG.is_match(question);
    let api_weak = API_WEAK.is_match(question);
    let api_noun = API_NOUN.is_match(question);
    let concept = CONCEPT.is_match(question);

    let all_symbols = || {
        strong
            .iter()
            .chain(weak.iter())
            .cloned()
            .collect::<Vec<_>>()
    };

    if api_strong && (api_noun || !strong.is_empty() || !weak.is_empty()) {
        return QueryKind::ApiReference {
            symbols: all_symbols(),
        };
    }
    if api_weak && (!strong.is_empty() || (api_noun && !concept)) {
        return QueryKind::ApiReference {
            symbols: all_symbols(),
        };
    }
    if api_weak && api_noun && !weak.is_empty() && !concept {
        return QueryKind::ApiReference {
            symbols: all_symbols(),
        };
    }
    if concept {
        return QueryKind::Conceptual;
    }
    if api_strong {
        // "What does it return?" without any subject: still reference-style.
        return QueryKind::ApiReference {
            symbols: Vec::new(),
        };
    }
    QueryKind::Mentoring
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_api(q: &str) -> bool {
        matches!(classify(q), QueryKind::ApiReference { .. })
    }

    #[test]
    fn spec_examples_are_api_reference() {
        // Spec 4.3, list of forbidden reference questions.
        assert!(is_api("Что делает функция?"));
        assert!(is_api("Какие аргументы принимает метод?"));
        assert!(is_api("Как работает `tokio::spawn`?"));
        assert!(is_api("Какие значения возвращает структура?"));
        assert!(is_api(
            "Какие ограничения имеет конкретная функция библиотеки?"
        ));
    }

    #[test]
    fn spec_examples_are_conceptual() {
        // Spec 4.3, list of allowed conceptual questions.
        for q in [
            "Что такое итератор?",
            "Как работает владение памятью?",
            "Что такое хеширование?",
            "Как устроена многопоточность?",
            "Почему возникают гонки данных?",
            "Как работает HashMap?",
            "What is a lifetime?",
        ] {
            assert_eq!(classify(q), QueryKind::Conceptual, "{q}");
        }
    }

    #[test]
    fn english_api_questions() {
        assert!(is_api("What does `Vec::retain` do?"));
        assert!(is_api("what arguments does parse_range take"));
        assert!(is_api("What does HashMap::entry return?"));
        assert!(is_api("how do I call serde_json::from_str?"));
        assert!(is_api("does unwrap() panic on None"));
    }

    #[test]
    fn mentoring_questions() {
        for q in [
            "My test fails with index out of bounds, where should I start looking?",
            "Я думаю, ошибка в обработке последнего элемента. Как проверить гипотезу?",
            "Почему мой тест не проходит?",
        ] {
            assert!(!is_api(q), "{q}");
        }
    }

    #[test]
    fn extracts_symbols() {
        let QueryKind::ApiReference { symbols } =
            classify("What does `Vec::retain` return and what does parse_range() take?")
        else {
            panic!()
        };
        assert!(symbols.contains(&"Vec::retain".to_owned()));
        assert!(symbols.contains(&"parse_range".to_owned()));
    }

    #[test]
    fn redirects_api_questions() {
        let req = MentorRequest {
            question: "Что возвращает `str::find`?".into(),
            excerpt: None,
        };
        let d = decide(&classify(&req.question), &req);
        assert_eq!(
            d,
            BoundaryDecision::Redirect {
                symbols: vec!["str::find".into()]
            }
        );
    }

    #[test]
    fn excerpt_handling() {
        let short = MentorRequest {
            question: "Что значит слово 'amortized' здесь?".into(),
            excerpt: Some("Push has amortized O(1) complexity.".into()),
        };
        assert!(matches!(
            decide(&classify(&short.question), &short),
            BoundaryDecision::ExplainExcerpt { .. }
        ));

        let long = MentorRequest {
            question: "Перескажи".into(),
            excerpt: Some("x".repeat(MAX_EXCERPT_CHARS + 1)),
        };
        assert!(matches!(
            decide(&classify(&long.question), &long),
            BoundaryDecision::Refuse { .. }
        ));
    }
}
