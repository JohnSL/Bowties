# ADR-0019 — JMRI as primary compatibility target

**Status**: Accepted
**Date**: 2026-07-23
**Related**: [ADR-0016](0016-per-peer-session-actor.md), [ADR-0018](0018-cdi-download-exchange-ownership.md)

## Context

The LCC/OpenLCB standards documents (`markdown/standards/S-*.md` and
`TN-*.md`) define the wire protocol, but they leave meaningful latitude
in what is allowed vs. required. The GridConnect spec, for example,
permits a `\r\n` line terminator after the trailing `;`; JMRI has never
emitted it; a real-world SPROG adapter developed a state-machine
brittleness against `\r\n` that surfaced only when Bowties began emitting
the spec-legal-but-JMRI-rare bytes. That fix (documented under the
"SPROG-debug serial scaffolding retired" entry in
`aiwiki/architecture-health.md`) is the case study for a broader
pattern: every LCC library in the wild has been debugged against JMRI's
subset of the standard, so "the standard permits X" is a necessary but
not sufficient reason for us to do X.

Two concrete divergences already known:

- **`TerminateDueToError` (TDE) emission.** The standard permits emission
  on our-side abort, and ADR-0016 / ADR-0018 codified a "peer-cleanup
  contract" that emits TDE exactly once when the session terminates an
  exchange on our fault while the wire is live. OpenLCB Java (the
  library backing JMRI/LccPro) **never emits TDE anywhere**, in any
  path. A user's write against a custom node has been failing with
  `OptionalInteractionRejected` (OIR) in a pattern that the reference
  implementation would never produce, and Bowties' TDE emission may be
  a contributor to the peer's state-machine confusion.
- **Retry on OIR-with-temporary-flag.** JMRI's SNII handler retries when
  bit `0x2000` of the OIR code is set; Bowties terminates on any OIR
  regardless of the flag.

Reasoning from the written standard alone would not have surfaced either
divergence.

## Decision

**JMRI (via `OpenLCB_Java`) is the primary compatibility target for
Bowties.** Where JMRI's on-wire behaviour differs from what the written
standard permits, we mirror JMRI unless we have a specific,
documented reason not to.

This applies to what we **emit** (bytes on the wire, frame timing,
optional protocol features) and to what we **expect** on receipt
(silence-window durations, retry patterns, tolerance of optional
frames). It does **not** override standards conformance for correctness
— we still parse per the standard, and we still reject genuinely
malformed input — but among behaviours the standard permits, we prefer
the subset JMRI uses.

The standard remains the source of truth for message *semantics* (what
an MTI means, what an error code means, what a payload byte layout is).
JMRI wins on *behavioural policy* (whether to emit optional frames,
when to retry, how to time interactions).

## Consequences

- Existing ADRs that established behaviour diverging from JMRI are
  candidates for review. In particular, ADR-0016's "peer-cleanup
  contract" (invariants #7 and #8) and ADR-0018's TDE-on-timeout for
  CDI need to be re-evaluated under this principle; the outcome may be
  a dated extension to those ADRs (or their supersession) that removes
  TDE emission from Bowties entirely.
- New protocol behaviour must include a JMRI comparison. The design
  step for any change under `lcc-rs/` should cite what JMRI does at the
  equivalent seam. If JMRI does nothing (i.e. the behaviour is
  Bowties-specific), that's a decision to record explicitly, not a
  default.
- The running list of known JMRI-vs-standard divergences we mirror lives
  in `aiwiki/architecture-health.md` under "JMRI-alignment audit
  candidates" so a reader sees which behaviours are deliberately
  ecosystem-shaped rather than standard-derived.
- Users who report "Bowties doesn't work with X, but JMRI does" are
  reporting a bug against this ADR, not a peer-side deficiency.

## Considered options

- **Reference the standard alone.** Rejected: the SPROG `\r\n` incident
  and the current OIR/TDE thread both demonstrate that "spec-legal but
  JMRI-atypical" is a real compatibility hazard we can't detect except
  by end-user reports.
- **Mirror JMRI without conditions.** Rejected as the sole rule: JMRI
  has its own bugs and quirks, and we should not adopt them uncritically
  (e.g. JMRI's "TODO: handle write errors and report to user somehow"
  in `MemorySpaceCache` is not a UX pattern we should copy). The
  principle applies to *on-wire behaviour toward peers*, not to
  internal architecture, error surfacing, or UX.
- **Do nothing until a specific compatibility bug lands.** Rejected: the
  cost of the reactive approach is user pain plus per-incident forensic
  work. Adopting the principle proactively lets us align behaviour
  during design rather than after a field report.

## Invariants

1. **JMRI-comparison in `lcc-rs` design.** Any new behaviour in
   `lcc-rs/` that emits on-wire bytes or drives peer state transitions
   must record what the equivalent behaviour is in OpenLCB Java
   (`OpenLCB_Java/src/**`), either in the associated ADR extension or
   in the PR description. Audit hint: grep `lcc-rs/src/**` for
   `send`/`emit`/`transmit` call sites added since 2026-07-23 and
   confirm each has a JMRI-comparison note in an ADR, an `aiwiki/`
   entry, or a code comment.

2. **Known divergences listed.** The set of behaviours where Bowties
   deliberately mirrors a JMRI-specific choice over the written standard
   lives in one place: `aiwiki/architecture-health.md` under the
   "JMRI-alignment audit candidates" section (or a resolved-list
   subsection once items are completed). Audit hint: grep for
   "JMRI-alignment" in `aiwiki/`; every item should be either open,
   resolved with a dated note, or explicitly rejected with a reason.
