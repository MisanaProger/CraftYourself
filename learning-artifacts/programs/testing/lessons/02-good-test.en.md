# Anatomy of a good test

A good test fails if and only if the behaviour it describes is broken — and its
failure tells you right away what broke.

## 1. Arrange — Act — Assert

A test has three parts: prepare the data, perform one action, check the result.

```rust
#[test]
fn discount_starts_at_exactly_1000() {
    // Arrange
    let cart = Cart::with_total(1000);
    // Act
    let price = cart.price_with_discount();
    // Assert
    assert_eq!(price, 950);
}
```

If a test has several “Act” steps, it is several tests.

## 2. One behaviour per test, and a telling name

The name answers “what must be true?”: `discount_starts_at_exactly_1000`,
`empty_input_returns_error` — not `test1` or `test_discount`. When such a test
fails, its name already explains the problem.

## 3. Test behaviour, not implementation

A test checks what is visible from outside: a return value, an error, a change
of state — not which private functions were called. Then the test survives
refactoring and fails only when something is really broken.

## 4. Deterministic and independent

- No current time, random numbers, network or execution order — fix them
  (a given time, a seed) or move them outside the test boundary.
- Tests do not depend on each other or on the order they run in. Each one
  prepares its own data.

A flaky test that sometimes fails is worse than none: people stop trusting it.

## 5. Precise assertions

`assert!(result.is_ok())` only says “not ok”. `assert_eq!(result, Ok(950))`
shows the expected and the actual value. Compare concrete values and add a
message when the value alone does not explain what went wrong.

## 6. Boundaries and equivalence classes

Inputs split into classes with the same behaviour (“total below 1000 — no
discount”). One value per class is enough, but **every boundary** is tested on
both sides: 999 and 1000, 4999 and 5000. Most bugs live on boundaries: `<`
instead of `<=`, off-by-one errors.

## 7. Regression test: red first

Found a bug? First write a test that reproduces it and **fails**. Only then fix
the code. The green run after the fix proves that exactly this was fixed, and
the test keeps the bug from coming back.

## What to avoid

- Logic in tests (`if`, loops computing the expectation): a test should be
  obviously correct.
- Copying the implementation into the test: it will repeat the same bug.
- Checking everything at once in one huge test.
- Tests without assertions.
