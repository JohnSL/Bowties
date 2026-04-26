---
applyTo: "lcc-rs/**"
description: "Use when editing the Bowties lcc-rs protocol library. Prioritize protocol correctness, transport clarity, public API stability, and strong test coverage over app-specific convenience."
---

# lcc-rs

- Use `product/architecture/code-placement-and-ownership.md` when deciding whether logic belongs in the protocol library or should remain Bowties application code.
- Treat `lcc-rs` as a protocol library, not as Bowties application code.
- Keep protocol encoding, decoding, discovery, transport, and alias behavior owned by focused modules with clear public contracts.
- Do not leak Bowties UI assumptions or application workflow shortcuts into this library.
- When implementing or correcting protocol behavior, consult `OpenLCB_Java/` and `JMRI/` in this workspace as practical reference implementations.
- Apply SOLID by keeping protocol concerns separated and by giving each module one clear protocol or transport responsibility.
- Apply DRY by reusing canonical protocol helpers and shared types instead of adding parallel parsing or encoding paths.
- Apply YAGNI by avoiding app-specific convenience APIs that weaken the library boundary or public API clarity.
- Apply TDD by writing or updating focused library tests first when practical, especially for protocol frames, datagrams, alias behavior, transport interactions, and parsing edge cases.
- Prefer stronger test evidence here than in app glue code because regressions in this library can affect multiple consumers.