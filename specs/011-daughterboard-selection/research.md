# Phase 0 Research

## Decision 1: Extend the existing structure-profile schema instead of creating a separate board-compatibility subsystem

- **Decision**: Add connector-slot declarations, reusable daughterboard references, carrier-specific overrides, and repair metadata to the existing `.profile.yaml` structure-profile model.
- **Rationale**: The current backend already loads one structure profile per manufacturer/model and resolves profile-declared CDI paths during `get_node_tree`. Extending that schema keeps profile-driven behavior in one authoritative system, fits the existing loader and resolver seams, and preserves the spec rule that compatibility and repair behavior must come only from authored profiles.
- **Alternatives considered**:
  - Separate user-maintained compatibility lookup files: rejected because the spec explicitly forbids an extra lookup outside the profile system.
  - Hardcoded board rules in frontend/backend code: rejected because it would duplicate profile logic and make carrier expansion expensive.

## Decision 2: Persist per-node connector selections in saved layout/project metadata, not on the node and not only in offline change rows

- **Decision**: Store connector selections as saved per-node layout metadata, restored when the layout/project reopens. Treat any resulting config edits as ordinary staged changes in the existing offline/pending-change flow.
- **Rationale**: The spec clarifies that connector selections are per-node-instance assumptions, not device-authored configuration. Current layout persistence already stores user-managed node-specific metadata in YAML, while offline changes capture planned config writes. This split cleanly models hardware assumptions separately from node memory writes.
- **Alternatives considered**:
  - Writing selections back to device memory: rejected because the selections describe the installation context rather than device-owned facts.
  - Storing selections only in offline change rows: rejected because offline changes model pending writes, not durable layout assumptions that must restore before any edit session begins.
  - Session-only frontend state: rejected because the spec requires persistence across reopened saved contexts.

## Decision 3: Resolve static connector topology in the backend, but evaluate active selections and staged repairs in frontend workflow owners

- **Decision**: The backend should parse and attach connector-slot metadata, affected-path mappings, and reusable daughterboard references to the config tree or companion payloads. The frontend should own active per-node selection state, option filtering, and auto-staged compatible follow-up config changes through stores/orchestrators.
- **Rationale**: Backend ownership is appropriate for profile parsing, CDI-path resolution, and persisted layout metadata. Frontend ownership is appropriate for visible page state, staged edits, and immediate UI filtering without extra network round-trips. This aligns with `product/architecture/code-placement-and-ownership.md`.
- **Alternatives considered**:
  - Backend-only live filtering and staged-repair orchestration: rejected because it would over-centralize visible UI behavior and require more IPC churn for every selection change.
  - Frontend parsing raw YAML profiles directly: rejected because it would duplicate authoritative parsing and bypass existing profile cache and CDI resolution.

## Decision 4: Model daughterboards as reusable authored definitions with optional carrier overrides

- **Decision**: Introduce reusable daughterboard definitions that carrier boards reference per slot, plus optional override blocks for carrier-specific behavior differences.
- **Rationale**: The spec explicitly chooses reusable daughterboard profiles to avoid a carrier/daughterboard cross-product explosion while allowing targeted variations. This matches the RR-CirKits Tower and Signal families, which share compatible aux-port cards but may still need carrier-specific affected-line mapping or repair defaults.
- **Alternatives considered**:
  - Duplicating every daughterboard definition inside each carrier profile: rejected because it scales poorly and violates the spec’s reuse goal.
  - Forcing one universal definition with no overrides: rejected because the user explicitly identified uncertainty about carrier-specific differences.
  - Pair-specific fully separate profiles: rejected because it reintroduces combinatorial growth.

## Decision 5: Auto-stage compatible repairs using profile-authored fallback rules first, then compatible defaults/empty states

- **Decision**: When a connector selection invalidates current settings, Bowties should stage compatible replacements or resets automatically. Resolution order is: profile-authored fallback/repair rule if present, otherwise compatible default or empty state derived from the field definition.
- **Rationale**: The user explicitly wants Bowties to stage the necessary changes rather than require manual repair knowledge. Putting first priority on profile-authored repair rules keeps board-specific behavior in profiles and lets the UI remain generic.
- **Alternatives considered**:
  - Warning only: rejected because it pushes repair burden onto the user.
  - Blocking the selection change itself: rejected because it breaks the staged-edit model.
  - Hardcoded global reset strategy: rejected because different daughterboards may need different compatible outcomes.

## Decision 6: Initial implementation scope targets RR-CirKits Tower-LCC and Signal families; SPROG IO-LCC remains deferred

- **Decision**: Plan around RR-CirKits Tower-LCC and Signal LCC-32H/Signal LCC-S/Signal LCC-P carrier boards plus the explicit initial RR-CirKits daughterboard/card set. Keep SPROG IO-LCC out of the committed implementation scope until connector compatibility is confirmed from product documentation.
- **Rationale**: The workspace and RR-CirKits product descriptions provide direct evidence that these RR-CirKits carrier boards use aux-port modules compatible with connector-based configuration rules. SPROG IO-LCC is plausible but not yet sufficiently evidenced for committed implementation planning.
- **Alternatives considered**:
  - Keeping scope limited to Tower-LCC only: rejected because the feature schema should not need immediate redesign for closely related Signal LCC carriers.
  - Committing SPROG IO-LCC immediately: rejected because the needed modular behavior is not yet confirmed by a reliable manual/source in the workspace.