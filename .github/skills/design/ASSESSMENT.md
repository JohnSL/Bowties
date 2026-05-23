# Assessment Methodology

How to evaluate a feature's architectural fitness and scale the output.

## Evaluation Criteria

### 1. Depth (deletion test)

Apply the deletion test to each proposed or touched module: if you deleted it, would complexity vanish (shallow — pass-through) or reappear across N callers (deep — earning its keep)?

- **Good**: Module hides significant behavior behind a small interface. Callers don't need to know how it works.
- **Concerning**: Module's interface is nearly as complex as its implementation. Callers do most of the real work.

### 2. Locality

Does the feature's proposed change concentrate change, bugs, and knowledge in one place?

- **Good**: A bug in this behavior can be found and fixed in one module.
- **Concerning**: Understanding this behavior requires reading across 3+ modules in different layers.

### 3. Seam placement

Are new interfaces at natural seams? Is something actually varying across the seam?

- **Good**: Two or more adapters exist (or will exist) at this seam — it's a real variation point.
- **Concerning**: Only one adapter. The seam is hypothetical — it adds interface complexity without leverage.

### 4. Leverage

Does the caller get significant behavior from a small interface?

- **Good**: One call triggers a complex multi-step workflow. The caller doesn't coordinate the steps.
- **Concerning**: The caller must orchestrate multiple calls in the right order to achieve one outcome.

### 5. Placement compliance

Does the module sit in the right layer per `product/architecture/code-placement-and-ownership.md`?

- Walk the decision questions in order: protocol behavior? → lcc-rs. Authoritative state? → backend. Multi-step workflow? → orchestrator. Etc.
- Flag modules that mix concerns from different layers.

### 6. ADR compliance

Does the approach conflict with a past architecture decision in `product/architecture/adr/`?

- Only flag when the conflict is real and the feature would violate the decision's intent.
- If the ADR should be revisited, say so explicitly with the reason.

### 7. Duplication

Does the plan propose logic that already exists in shared conventions (from `aiwiki/owners.md`)?

- Check: normalization helpers, formatting, fallback chains, key generation, enrichment patterns.
- Flag when new code would duplicate an existing canonical implementation.

### 8. Cross-layer coupling

Does the design create new dependencies between layers that don't currently exist?

- **Good**: Feature uses existing layer interfaces without adding new cross-layer calls.
- **Concerning**: Frontend component directly calls a backend command that bypasses the orchestration layer.

### 9. Testability

Can the proposed design be tested through its interface without mocking internals?

- **Good**: Tests exercise behavior through public interfaces. Dependencies are injected at system boundaries.
- **Concerning**: Testing requires mocking internal collaborators or reaching past the interface.

### 10. Existing debt

Is the touched module already shallow? Would this feature make it shallower?

- Look for: pass-through functions, thin wrappers, modules whose interface mirrors their implementation.
- Flag when adding to a module would increase its interface complexity without proportionally increasing its depth.

### 11. Deepening opportunities

Could we improve a touched module's depth or locality as part of this feature work?

- Look for: scattered logic that could be consolidated, multiple callers doing the same coordination, test workarounds that indicate a missing seam.
- Only flag when the improvement is narrow and testable within the feature's scope.

## Scaling Rules

| Signal | Output level | What to present |
|--------|-------------|-----------------|
| All modules clean, feature maps to existing patterns, no new modules | **Brief** | "Architecture confirmed, no concerns" + 2-3 sentence summary + slice definitions |
| 1-2 minor findings (small duplication, minor placement question) | **Standard** | Findings list with recommendations. User confirms. Slice definitions. |
| Existing debt found in touched modules | **Standard + Deepening** | Describe the debt, propose improvement, user decides include vs. defer. Slice definitions. |
| 3+ layers affected, new modules needed, design trade-offs | **Full** | Complete analysis with trade-offs. Module proposals evaluated for depth/leverage. ADR drafts if needed. Slice definitions with HITL labels on trade-off slices. |

## Plan.md Section Template

Append this section to `specs/<feature>/plan.md` after the user has reviewed findings:

```markdown
## Architecture Assessment

### Affected Modules

| Module | Layer | Impact | Notes |
|--------|-------|--------|-------|
| {module name} | {layer} | Modified / New / Touched | {brief note} |

### Assessment Summary

{Brief or detailed summary based on scaling level. Use domain vocabulary and architecture vocabulary.}

### Findings

{Only if Standard or higher. For each finding:}

**F{N}: {Title}**
- Category: {depth / locality / seam / placement / ADR / duplication / coupling / testability / debt / deepening}
- Affected: {module names}
- Concern: {description using architecture vocabulary}
- Decision: {include / defer / reject} — {reason}

### Vertical Slices

{Ordered list of slices. Each slice:}

**S{N}: {Slice title}**
- Type: HITL / AFK
- Layers: {which layers this slice touches}
- Blocked by: {S{M} or "None"}
- Test: {what the integration test proves}
- Acceptance: {independently verifiable outcome}

### Deferred Improvements

{References to GitHub issues proposed for deferred improvements (only those the user approved and that were created).}

### Architecture Decisions

{References to ADRs created, if any.}
```

## Findings Format

Each finding has:
- **ID**: F1, F2, F3... (sequential within the assessment)
- **Category**: one of the 11 evaluation criteria
- **Affected**: module name(s) in domain vocabulary
- **Concern**: description using architecture vocabulary (depth, locality, seam, leverage)
- **Recommendation**: include in slices / defer as idea / needs user decision
- **Decision**: filled in after user input
