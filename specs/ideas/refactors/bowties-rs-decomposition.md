# Decompose bowties.rs and Add Catalog Builder Tests

- **Areas**: architecture, cleanup, backend, testing
- **Origin**: spec 013 architecture assessment (F7)
- **Status**: deferred
- **Date**: 2026-05-17

`commands/bowties.rs` (1,962 lines, 0 tests) mixes three distinct concerns: (1) bowtie catalog building (~700 lines — the core algorithm), (2) protocol exchange for event role queries (~130 lines), and (3) layout file YAML commands (`load_layout`, `save_layout`, `get_recent_layout`, etc. ~200 lines). The catalog builder is the intellectual core of the app with zero test coverage.

## Prior Work

- **Assessment**: The catalog builder implements a multi-phase algorithm (pre-walk CDI, config-primary discovery, protocol-primary discovery, well-known events, role resolution via profile/CDI/protocol, card emission). It is genuinely deep — a single `build_bowtie_catalog` call triggers complex behavior. But the file's breadth (3 unrelated concerns) makes it hard to navigate and test.
- **Decomposition targets**: (1) Extract layout YAML commands to a dedicated `commands/layout_yaml.rs` or fold into `layout_capture.rs`. (2) Extract `query_event_roles` protocol exchange to `commands/discovery.rs` or a focused protocol module. (3) Keep catalog builder as `commands/bowties.rs` (or rename to `commands/catalog_builder.rs`).
- **Test priority**: The catalog builder should be tested with in-memory node proxies and synthetic CDI trees. Key test scenarios: config-primary events (identical eventid slots), protocol-primary events (identify-events exchange), well-known events, role classification fallback (profile → CDI heuristic → protocol → ambiguous), metadata merge on rebuild.
- **Constraint**: spec 013 S2 and S9 call `build_bowtie_catalog_command` and `merge_layout_metadata` without modifying them. Decomposition and testing can proceed independently after spec 013 lands.
