# Running tests and reading their output

Tests are the fastest way to learn whether code does what you think it does.
They are of little use, though, if you cannot run the part you need and see
exactly what broke.

## Running everything, a part, a single test

Every ecosystem can run all tests and narrow the run down:

| Goal | Rust (cargo) | Python (pytest) | C/C++ (CTest) |
|---|---|---|---|
| All tests | `cargo test` | `python -m pytest` | `ctest --test-dir build` |
| One package / module | `cargo test -p globset` | `python -m pytest tests/test_parser.py` | `ctest -R parser` |
| By test name | `cargo test parse_range` | `python -m pytest -k parse_range` | `ctest -R '^parse_range$'` |
| Show `println!`/`print` output | `cargo test -- --nocapture` | `python -m pytest -s` | `ctest --output-on-failure` |
| Stop at the first failure | `cargo test -- --test-threads=1`, read the first failure | `python -m pytest -x` | `ctest --stop-on-failure` |

Filtering by name is the key debugging move: you run only the test that
reproduces the problem and get an answer in seconds instead of minutes.

In Craft Yourself tests run in the **sandbox**: the project's code executes in
an isolated container with no network, with memory and time limits, and it
cannot modify the files in your workspace. The “Run tests” button on the Tests
tab and the `est test` command do the same thing.

## Reading the output

Here is a typical `cargo test` output:

```text
running 3 tests
test tests::parses_simple_range ... ok
test tests::parses_inclusive_range ... FAILED
test tests::rejects_garbage ... ok

failures:

---- tests::parses_inclusive_range stdout ----
thread 'tests::parses_inclusive_range' panicked at src/lib.rs:21:9:
assertion `left == right` failed
  left: [1, 2]
 right: [1, 2, 3]

test result: FAILED. 2 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

Read it in this order:

1. **The summary line** (`test result`) — how many passed, failed, were ignored.
2. **The failing test's name** — which behaviour is broken.
3. **The location** (`src/lib.rs:21:9`) — the line of the assertion, not of the bug.
4. **Expected vs actual** — `left` (what you got) and `right` (what you
   expected). The difference between them is your first clue.

A **compilation** error is not a failing test: the tests did not run at all.
Fix the build first.

## Green does not mean “correct”

When all tests pass, you only know that what is checked works. A test that
asserts nothing is always green. So make it a habit to check once that a test
**can fail**: temporarily break the code or the expectation and see that the
test notices.

## Try it

1. Find out how the project runs its tests (the Tests tab shows the command).
2. Run all tests, then a single one by name.
3. Temporarily change the expected value in any test, run it and read the
   output using the steps above. Then change it back.
