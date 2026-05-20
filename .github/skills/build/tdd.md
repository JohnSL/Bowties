# Test-Driven Development

Adapted from Matt Pocock's TDD methodology for Bowties.

## Philosophy

Tests verify **behavior through public interfaces**, not implementation details. Code can change entirely; tests shouldn't break unless behavior changed.

**Good tests** are integration-style: they exercise real code paths through public APIs. They describe _what_ the system does, not _how_. A good test reads like a specification — "user can see connected nodes" tells you exactly what capability exists. These tests survive refactors because they don't care about internal structure.

**Bad tests** are coupled to implementation. They mock internal collaborators, test private methods, or verify through external means. Warning sign: your test breaks when you refactor, but behavior hasn't changed.

See [tests.md](tests.md) for examples and [mocking.md](mocking.md) for mocking guidelines.

## Anti-Pattern: Horizontal Slicing

**DO NOT write all tests first, then all implementation.** This is "horizontal slicing" — treating RED as "write all tests" and GREEN as "write all code."

This produces bad tests:
- Tests written in bulk test _imagined_ behavior, not _actual_ behavior
- You end up testing _shapes_ (data structures, function signatures) rather than user-facing behavior
- Tests become insensitive to real changes — they pass when behavior breaks, fail when it's fine
- You outrun your headlights, committing to test structure before understanding implementation

**Correct approach**: Vertical slices via tracer bullets. One test → one implementation → repeat.

```
WRONG (horizontal):
  RED:   test1, test2, test3, test4, test5
  GREEN: impl1, impl2, impl3, impl4, impl5

RIGHT (vertical):
  RED→GREEN: test1→impl1
  RED→GREEN: test2→impl2
  RED→GREEN: test3→impl3
```

## Workflow

### 1. Planning

Before writing code for a slice:

- [ ] Confirm which behaviors to test (from slice acceptance criteria)
- [ ] Identify opportunities for [deep modules](deep-modules.md)
- [ ] Design interfaces for [testability](interface-design.md)
- [ ] List behaviors to test — not implementation steps

### 2. Tracer Bullet

Write ONE test that confirms ONE thing about the slice:

```
RED:   Write test for first behavior → test fails
GREEN: Write minimal code to pass → test passes
```

This is the tracer bullet — proves the vertical path works end-to-end.

### 3. Incremental Loop

For each remaining behavior in the slice:

```
RED:   Write next test → fails
GREEN: Minimal code to pass → passes
```

Rules:
- One test at a time
- Only enough code to pass the current test
- Don't anticipate future tests

### 4. Refactor

After all slice tests pass:

- [ ] Extract duplication
- [ ] Deepen modules (move complexity behind simple interfaces)
- [ ] Consider what new code reveals about existing code
- [ ] Run tests after each refactor step

**Never refactor while RED.** Get to GREEN first.

## Per-Cycle Checklist

```
[ ] Test describes behavior, not implementation
[ ] Test uses public interface only
[ ] Test would survive internal refactor
[ ] Code is minimal for this test
[ ] No speculative features added
```

## Bowties Testing Context

| Layer | Test framework | Test location |
|-------|---------------|---------------|
| Frontend (stores, utils, orchestrators) | Vitest | `app/src/**/*.test.ts` |
| Frontend (components) | Vitest + Testing Library | `app/src/**/*.test.ts` |
| Backend (Tauri commands, domain) | Rust `#[cfg(test)]` | Inline in source files |
| Protocol library (lcc-rs) | Rust `#[cfg(test)]` | Inline + `tests/` directory |

Run frontend tests: `cd app && npx vitest run`
Run backend tests: `cd app/src-tauri && cargo test`
Run protocol tests: `cd lcc-rs && cargo test`
