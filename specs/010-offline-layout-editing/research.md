# Research: Offline Layout Editing

**Feature Branch**: `010-offline-layout-editing`  
**Date**: 2026-04-04

## R-001: Canonical On-Disk Format for Captured Layouts

**Decision**: Use YAML (UTF-8) for all persisted layout artifacts: `manifest.yaml`, `nodes/<NODE_ID>.yaml`, `bowties.yaml`, `offline-changes.yaml`, and optional `cdi-bundle/` index files.

**Rationale**: The spec explicitly requires YAML as canonical format (FR-027), deterministic serialization (FR-028), and git-friendly isolated diffs (FR-036, FR-030). YAML is already used in Bowties (`serde_yaml_ng`) and supports readable maps with stable ordering when encoded from `BTreeMap`.

**Alternatives considered**:
- JSON: rejected because FR-027 mandates YAML.
- SQLite: rejected as not human-readable and poor for git diffs.
- Single monolithic file: rejected because FR-036 requires per-node diff isolation.

## R-002: Directory Save Atomicity for Multi-File Layout Writes

**Decision**: Use staging-and-swap directory commits: write to `.<layout>.tmp/`, fsync files, fsync directory, then atomically rename/swap to target layout directory.

**Rationale**: FR-006 and edge-case crash behavior require no partial valid output. Multi-file save needs stronger guarantees than file-level atomic rename. A staged directory swap preserves either old or new state.

**Alternatives considered**:
- In-place overwrite of individual files: rejected due to partial-write risk.
- Zip-first then extract: rejected due to complexity and poor diff ergonomics.

## R-003: Snapshot Schema for Node Files

**Decision**: Store per-node snapshot YAML by canonical node ID filename (`nodes/<NODE_ID>.yaml`) with sections for `snip`, `cdiRef`, `capture`, `values`, and `producerIdentifiedEvents`.

**Rationale**: Supports FR-002, FR-003, FR-005, FR-030 and cleanly separates identity, schema reference, and value payloads. Stable node-ID naming avoids churn on rename and keeps git history meaningful.

**Alternatives considered**:
- Filename by user-visible name: rejected because renames create noisy file churn.
- Embed all nodes in manifest: rejected because FR-036 expects node-local diffs.

## R-004: Offline Change Representation and Sync Classification

**Decision**: Persist offline changes separately from baseline values as row-based records keyed by `changeId` and identity tuple (`nodeId`, `space`, `offset`, optional `bowtieId`), including `baselineValue`, `plannedValue`, and mutable sync status.

**Rationale**: FR-009, FR-012, FR-014, and FR-016 require triage into conflict/clean/already-applied without losing captured baseline. Separate change rows allow selective apply, retries, and explicit per-row failure reporting (FR-017a).

**Alternatives considered**:
- Overwrite baseline with planned values: rejected because conflict detection becomes impossible.
- Keep only UI-memory pending state: rejected because offline sessions must survive restart.

## R-005: Bus-to-Layout Matching and Mode Gate

**Decision**: Implement weighted node-ID overlap classification at connect-time with fixed thresholds from clarifications: `likely same >=80%`, `uncertain 40-79%`, `likely different <40%`.

**Rationale**: FR-013b and FR-016b require deterministic pre-sync gating before full value comparisons complete. This lets UI present an explicit target/bench decision when confidence is low.

**Alternatives considered**:
- Exact set equality only: rejected because powered-subset operation is common.
- User always decides manually: rejected as too much friction when confidence is high.

## R-006: Handling Read-Only Fields in Sync

**Decision**: Use profile-driven suppression when profile marks a field read-only; otherwise show as normal row. If bus write reply explicitly returns read-only, clear pending row and restore latest read value.

**Rationale**: Required by FR-016a and FR-017b. This avoids persistent false-dirty rows while preserving transparency for unprofiled nodes.

**Alternatives considered**:
- Global heuristic suppression (e.g., never-write addresses): rejected due to protocol variability.
- Keep read-only rows pending forever: rejected due to poor UX and violation of FR-017b.

## R-007: CDI Portability Strategy

**Decision**: Persist stable CDI references in node snapshots by default; add explicit export/import CDI bundle flow for portable offline use.

**Rationale**: Matches FR-003 and FR-004 while minimizing capture size and duplication. Optional bundles solve missing-cache workflows on other machines.

**Alternatives considered**:
- Always inline full CDI XML in node files: rejected due to file bloat and noisy diffs.
- No portability support: rejected because FR-004 requires explicit export/import.

## R-008: Save/Apply Failure Strategy

**Decision**: Continue applying independent rows on non-fatal failures; mark failed rows with error details and keep pending for retry.

**Rationale**: Required by clarification and FR-017a. This maximizes useful progress during noisy bus conditions and prevents one bad row from blocking all others.

**Alternatives considered**:
- Fail-fast on first error: rejected because it reduces throughput and violates FR-017a.
- Auto-retry indefinitely: rejected due to blocking and unclear operator control.

## R-009: Source-of-Truth Boundaries for Offline vs Online State

**Decision**: Treat layout files as source of truth for baseline + pending offline changes when offline; once online reads complete, build an in-memory sync session for classification and resolution, then commit back to layout files.

**Rationale**: Aligns with FR-006, FR-012, FR-013, and FR-033. This keeps explicit user action as the only trigger for bus writes and ensures deterministic restart behavior.

**Alternatives considered**:
- Immediate writes on connect: rejected because FR-033 forbids automatic push.
- Keep sync session only in memory: rejected because apply retries and auditability need persisted row state.
