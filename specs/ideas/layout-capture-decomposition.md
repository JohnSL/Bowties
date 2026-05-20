# Decompose save_layout_directory Mega-Function

- **Areas**: architecture, cleanup, backend
- **Origin**: spec 013 architecture assessment (F6)
- **Status**: deferred
- **Date**: 2026-05-17

`save_layout_directory` in `commands/layout_capture.rs` (~150 lines) mixes five concerns in a single function: snapshot resolution (live vs offline), metadata merging, CDI validation/filtering, file I/O delegation, and state mutation (updating `active_layout`). It also has minimal test coverage (1 test) relative to its code path count.

## Prior Work

- **Assessment**: The function is deep (hides significant complexity behind a single IPC boundary), which is good. But its internal structure is flat — all five concerns are interleaved rather than composed from focused helpers. The `build_node_snapshot` private helper also inlines tree-walking logic (`collect_leaf_values`, `group_key`) that could be shared.
- **Decomposition targets**: (1) snapshot resolution → pure function, (2) metadata merge → already partially extracted, (3) CDI validation → predicate filter, (4) file I/O → already delegated to `layout::io`, (5) state mutation → separate step after I/O succeeds.
- **Constraint**: spec 013 S2 adds `save_layout_with_bus_writes` as a new command that *calls* `save_layout_directory` rather than extending it. This preserves the existing seam and makes decomposition safe to do independently.
- **Test coverage needed**: live-snapshot path, offline-only path, CDI-missing filtering, previous layout exists vs first save, metadata merge with conflicting connector selections.
