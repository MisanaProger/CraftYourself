//! Test runner output → pass/fail counts. Best effort: `None` when no known
//! summary line is found (the exit code still decides pass/fail).

use std::sync::LazyLock;

use est_domain::TestSummary;
use regex::Regex;

static CARGO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"test result: \w+\. (\d+) passed; (\d+) failed; (\d+) ignored").expect("regex")
});
static PYTEST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^=+ (.*\b(passed|failed|error)\b.*) in [\d.]+s").expect("regex")
});
static COUNT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(\d+) (passed|failed|skipped|errors?|xfailed|xpassed)").expect("regex")
});
static CTEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"tests passed, (\d+) tests? failed out of (\d+)").expect("regex"));
/// `run_samples.sh` of an online-judge problem (est-problems).
static SAMPLES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"est-samples: (\d+) passed, (\d+) failed").expect("regex"));
/// Python's unittest: `Ran 4 tests in 0.001s` then `OK (skipped=1)` or
/// `FAILED (failures=1, errors=1, skipped=1)`.
static UNITTEST_RAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^Ran (\d+) tests? in [\d.]+s").expect("regex"));
static UNITTEST_END: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^(OK|FAILED)(?: \(([^)]*)\))?\s*$").expect("regex"));
static KV: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(failures|errors|skipped|expected failures|unexpected successes)=(\d+)")
        .expect("regex")
});
static JEST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*Tests:?\s+(.*)$").expect("regex"));

pub fn summarize_tests(output: &str) -> Option<TestSummary> {
    if let Some(c) = SAMPLES.captures_iter(output).last() {
        return Some(TestSummary {
            passed: c[1].parse().unwrap_or(0),
            failed: c[2].parse().unwrap_or(0),
            ignored: 0,
        });
    }
    if let (Some(ran), Some(end)) = (
        UNITTEST_RAN.captures_iter(output).last(),
        UNITTEST_END.captures_iter(output).last(),
    ) {
        let total: u32 = ran[1].parse().unwrap_or(0);
        let mut s = TestSummary::default();
        for c in KV.captures_iter(end.get(2).map_or("", |m| m.as_str())) {
            let n: u32 = c[2].parse().unwrap_or(0);
            match &c[1] {
                "failures" | "errors" | "unexpected successes" => s.failed += n,
                _ => s.ignored += n,
            }
        }
        s.passed = total.saturating_sub(s.failed + s.ignored);
        return Some(s);
    }
    // cargo test: one line per test binary; sum them.
    let mut cargo = TestSummary::default();
    let mut found = false;
    for c in CARGO.captures_iter(output) {
        found = true;
        cargo.passed += c[1].parse::<u32>().unwrap_or(0);
        cargo.failed += c[2].parse::<u32>().unwrap_or(0);
        cargo.ignored += c[3].parse::<u32>().unwrap_or(0);
    }
    if found {
        return Some(cargo);
    }
    if let Some(line) = PYTEST.captures_iter(output).last() {
        let mut s = TestSummary::default();
        for c in COUNT.captures_iter(&line[1]) {
            let n: u32 = c[1].parse().unwrap_or(0);
            match &c[2] {
                "passed" | "xpassed" => s.passed += n,
                "failed" | "error" | "errors" => s.failed += n,
                _ => s.ignored += n,
            }
        }
        return Some(s);
    }
    if let Some(c) = CTEST.captures_iter(output).last() {
        let failed: u32 = c[1].parse().unwrap_or(0);
        let total: u32 = c[2].parse().unwrap_or(0);
        return Some(TestSummary {
            passed: total.saturating_sub(failed),
            failed,
            ignored: 0,
        });
    }
    if let Some(line) = JEST.captures_iter(output).last() {
        let mut s = TestSummary::default();
        let mut any = false;
        for c in COUNT.captures_iter(&line[1]) {
            any = true;
            let n: u32 = c[1].parse().unwrap_or(0);
            match &c[2] {
                "passed" => s.passed += n,
                "failed" => s.failed += n,
                _ => s.ignored += n,
            }
        }
        if any {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(passed: u32, failed: u32, ignored: u32) -> Option<TestSummary> {
        Some(TestSummary {
            passed,
            failed,
            ignored,
        })
    }

    #[test]
    fn runners() {
        let cargo = "running 3 tests\ntest result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out\n\
                     test result: ok. 4 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out";
        assert_eq!(summarize_tests(cargo), s(6, 1, 1));
        assert_eq!(
            summarize_tests("===== 3 passed, 1 failed, 2 skipped in 0.12s ====="),
            s(3, 1, 2)
        );
        assert_eq!(
            summarize_tests("========= 5 passed in 1.02s =========="),
            s(5, 0, 0)
        );
        assert_eq!(
            summarize_tests("80% tests passed, 1 tests failed out of 5"),
            s(4, 1, 0)
        );
        assert_eq!(
            summarize_tests("Tests:       1 failed, 3 passed, 4 total"),
            s(3, 1, 0)
        );
        assert_eq!(summarize_tests("      Tests  3 passed (3)"), s(3, 0, 0));
        assert_eq!(summarize_tests("compiling…\nerror[E0425]"), None);
        assert_eq!(
            summarize_tests(
                "sample 01: ok\nsample 02: wrong answer\nest-samples: 1 passed, 1 failed"
            ),
            s(1, 1, 0)
        );
        let unittest = "test_a ... ok\n----------------------------------------------------------------------\n\
                        Ran 4 tests in 0.002s\n\nFAILED (failures=1, errors=1, skipped=1)\n";
        assert_eq!(summarize_tests(unittest), s(1, 2, 1));
        assert_eq!(
            summarize_tests("Ran 2 tests in 0.000s\n\nOK (skipped=1)\n"),
            s(1, 0, 1)
        );
    }
}
