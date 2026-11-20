//! Which engagement level a request needs (architecture v0.3, §7.4).
//!
//! Rule based and conservative, like the documentation boundary classifier:
//! deterministic, auditable, and the model cannot argue its way around it.

use std::sync::LazyLock;

use est_domain::EngagementLevel;
use regex::Regex;

macro_rules! re {
    ($name:ident, $pat:expr) => {
        static $name: LazyLock<Regex> = LazyLock::new(|| Regex::new($pat).expect("valid regex"));
    };
}

// L4: the whole solution / doing the task.
re!(
    WANTS_FULL,
    r"(?ix)
      (полн\w*|цел\w*|готов\w*|вс[её]|подробн\w*)\s+(решени|реализаци|код|исправлени)
    | сделай\s+(за\s+меня|вс[её]|задачу)
    | реши\s+(за\s+меня|задачу|вс[её]|это)
    | напиши\s+(вс[её]|целиком|полностью)
    | \b(full|complete|whole|entire)\s+(solution|implementation|code|fix)\b
    | \bsolve\s+(it|this|the\s+task)\s+for\s+me\b
    | \bdo\s+it\s+for\s+me\b
    "
);

// L3: code fragments, pseudocode, an example of the fix.
re!(
    WANTS_CODE,
    r"(?ix)
      (покажи|напиши|дай|приведи)\s+(\S+\s+)?(код|пример|решени|реализаци|исправлени|фрагмент|псевдокод|функци|тест)
    | как\s+(это\s+)?(исправить|починить|реализовать|написать)
    | псевдокод
    | исправь
    | \b(show|write|give)\s+(me\s+)?(the\s+|a\s+|an\s+|some\s+)?(code|example|snippet|fix|solution|implementation|pseudocode|function|test)\b
    | \bhow\s+(do\s+i|to|should\s+i)\s+(fix|implement|write)\b
    | \bfix\s+(it|this)\b
    "
);

// L2: diagnosis, causes, explanation of an error.
re!(
    WANTS_DIAGNOSIS,
    r"(?ix)
      почему\s+(\S+\s+){0,3}(пада|не\s+работа|не\s+проход|ломается|ошибк|возвраща|паник|зависа)
    | в\s+ч[её]м\s+(причина|ошибка|проблема)
    | (объясни|разбери)\s+(\S+\s+)?(ошибк|причин|лог|трассир|стек|падение|поведение)
    | что\s+(не\s+так|означает\s+(эта\s+)?ошибк)
    | \bwhy\s+(does|is|do|did)\s+(it|this|my|the)\b.*\b(fail|crash|panic|break|wrong|error)
    | \bwhat('s|\s+is)\s+(wrong|the\s+cause)\b
    | \bexplain\s+(the|this)\s+(error|failure|stack|trace|panic)\b
    "
);

// L1: hints, where to look, where to start.
re!(
    WANTS_HINT,
    r"(?ix)
      подскаж | подсказк | намекни | намёк | намек
    | где\s+(искать|смотреть|посмотреть|находится)
    | с\s+чего\s+начать | куда\s+смотреть | в\s+каком\s+(файле|месте|модуле)
    | \bhint\b | \bwhere\s+(should\s+i\s+)?(look|start)\b | \bwhich\s+file\b
    "
);

/// The lowest level at which a mentoring request can be served.
pub fn required_level(question: &str) -> EngagementLevel {
    if WANTS_FULL.is_match(question) {
        EngagementLevel::Active
    } else if WANTS_CODE.is_match(question) {
        EngagementLevel::Demonstration
    } else if WANTS_DIAGNOSIS.is_match(question) {
        EngagementLevel::Explaining
    } else if WANTS_HINT.is_match(question) {
        EngagementLevel::Hinting
    } else {
        EngagementLevel::Socratic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use EngagementLevel::*;

    #[test]
    fn levels_from_requests() {
        for (q, level) in [
            (
                "Я проверил, что пустая строка приходит из конфигурации. Что дальше?",
                Socratic,
            ),
            ("Подскажи, где искать обработку пустой строки?", Hinting),
            ("С чего начать?", Hinting),
            ("Почему парсер падает на пустой строке?", Explaining),
            ("В чём причина этой ошибки?", Explaining),
            ("Да, покажи пример исправления", Demonstration),
            ("Как это исправить?", Demonstration),
            ("Напиши код проверки", Demonstration),
            ("Дай полное решение", Active),
            ("Сделай за меня", Active),
            ("Why does my test fail on empty input?", Explaining),
            ("show me the fix", Demonstration),
            ("give me the complete solution", Active),
            ("where should I look?", Hinting),
        ] {
            assert_eq!(required_level(q), level, "{q}");
        }
    }
}
