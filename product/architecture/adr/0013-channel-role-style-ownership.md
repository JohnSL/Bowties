# Channel schema: role + style + ownership, with style-owned constraints

## Context

Spec 015 introduced the **Information Channel** as a typed, named representation of a single piece of layout-meaningful information, persisted in `channels.yaml`. Its schema was minimal: `{ id, name, channelType, hardwareRef }`, with `channelType` a flat string enum (`block-occupancy` only) and `hardwareRef` a single shape `{ nodeKey, connector, input }` describing one BOD-style pin.

Spec 018 introduces **Facilities** — named instances of behavior templates with slots that bind by role — and adds a second channel kind (consumer channels backed by Direct Lamp Control rows). That forces three orthogonal questions onto the channel schema:

1. **State-vocabulary contract.** A facility slot must bind by *what the channel does* (state vocabulary: `unknown`/`occupied`/`clear`, or `unknown`/`lit`/`unlit`), not by *how the hardware realises it*. The same slot must accept a future style that realises the same state contract (e.g., a future LED driven from a different subsystem also satisfying a `lamp-indicator` slot). Today's `channelType` collapses both into a single string.
2. **Hardware-shape realisation.** Producer and consumer event-leaf mapping, claimed-pin shape, and managed CDI-field constraints live with the hardware, not with the state vocabulary. `single-led-direct-lamp` claims a `lampRow`, not a `connectorInput`; `bod-block-detector-input` claims an `input`, not a `lampRow`. The single-shape `hardwareRef` field cannot describe both.
3. **Lifecycle authority.** Hardware-owned channels (BOD inputs from a selected daughter board) are created and destroyed by hardware-config choices; user-owned channels (lamp-indicator channels) are created and destroyed by explicit user action on a facility slot. The schema must carry this distinction so cascade behavior on hardware-config changes (and on facility-slot operations) is deterministic.

A fourth pressure surfaces in the same change: the BOD-family `validityRules` today live under daughter-board entries in `RR-CirKits.shared-daughterboards.yaml`. With channels carrying a style, the same constraint-source-of-truth question reappears for the new `single-led-direct-lamp` style on Direct Lamp Control rows (must lock `Lamp Selection` away from "Used by Mast"). Splitting the constraint source between the daughter-board entry (for BOD) and the style entry (for lamp) creates two parallel mechanisms doing the same job.

## Decision

The channel schema gains three fields and one shape change, and the constraint contract moves to be **owned by style** rather than by the daughter-board selection event:

- **`role` (string)** — the state-vocabulary contract a facility slot binds by. Declared in **Rust enums** with their state vocabularies (mirrored as TS string-literal unions on the frontend) so production code matches state values exhaustively at compile time. Examples: `block-occupancy`, `lamp-indicator`. Internally an interface; user-facing language is always "role".
- **`style` (string)** — the specific hardware-shape realisation of the role. Declared in **profile YAML** under each subsystem's style catalog. Carries: which role it realises, which pins it claims, the producer/consumer event-leaf mapping, the **Style Constraint Contract**, and a `userCreatable` marker (Add-channel-able vs auto-created by hardware-config). Examples: `bod-block-detector-input`, `single-led-direct-lamp`. Internally an implementation class realising the role's interface.
- **`ownership` (enum: `hardware-owned` | `user-owned`)** — the lifecycle classification. `hardware-owned` channels are auto-created by hardware-config and auto-deleted when that config is cleared or changed; user rename does not change ownership. `user-owned` channels are created via a facility slot's *Add channel* action and deleted when removed from their only slot (no ref-counting in this slice). The cascade is the same in both cases: when a channel is destroyed, any facility slot bound to it becomes empty, and a Wired facility returns to Incomplete via the existing slot-detach pipeline.
- **`binding` (discriminated union)** — replaces the single-shape `hardwareRef`. Discriminator is `kind`; variants today: `{ kind: 'connectorInput', nodeKey, connector, input }` and `{ kind: 'lampRow', nodeKey, rowOrdinal }`. The `kind` MUST match the style's declared binding shape (enforced by the channel-validation layer, not by the type system, because styles are declared in YAML).
- **Style Constraint Contract is owned by style.** The existing profile-driven relevance/validity renderer is unchanged — only the *source* moves. BOD-family `validityRules` migrate from daughter-board entries into the `bod-block-detector-input` style's `constraints:` block in the same change set, with the legacy `validityRules` removed (not left in place) so the resolver has a single code path. `single-led-direct-lamp` declares its own `constraints:` block (at minimum, fixes `Lamp Selection` away from "Used by Mast").

The pre-018 `channelType` field is **retired** in the same family of changes (channel schema lands in the BOD-retrofit slice; the legacy per-input channel inventory is removed in the cleanup slice). No migration code is shipped — Spec 018 is pre-1.0 and the user is responsible for opening only post-018 layouts (FR-009).

`lcc-rs` learns none of these concepts — roles, styles, ownership, facilities, and constraint contracts are Bowties-only app abstractions atop event semantics (Constitution Principle IV).

## Considered options

- **Both role and style in YAML.** Rejected: roles encode state vocabularies that production code matches exhaustively (`match channel.role.state { Unknown | Occupied | Clear => ... }`). Declaring them in YAML forces a runtime registry lookup at every match site and loses the compile-time exhaustiveness check.
- **Both role and style in code.** Rejected: styles encode hardware metadata (which pins, which leaves, which CDI fields) that already lives in profiles. Putting styles in code duplicates that metadata and forces a code change every time a new board family ships.
- **Keep `channelType` and `hardwareRef`; add the new fields alongside.** Rejected: accumulates dead state; future readers cannot tell which field to trust; a single change set retiring `channelType` is cleaner than a multi-release deprecation given the pre-1.0 context.
- **Keep the constraint contract on daughter-board entries; add a parallel contract on styles only for new styles.** Rejected: two parallel mechanisms enforcing the same kind of decision is the canonical DRY violation. Re-applying the same restriction from two sources is idempotent today but adding a non-idempotent rule later (e.g., hiding from one source, restricting from another) would surface a divergence at the worst possible time.
- **Build a new constraint renderer specific to channels.** Rejected: duplicates the existing relevance/validity renderer; breaks visual consistency users already have for daughter-board constraints.
- **Explicit role-style registry file (`channel-roles.yaml`).** Rejected as premature: Spec 018 has exactly one style per role. The registry shape becomes worth building when the second style realises the same role (e.g., a `2-led-bicolor-aspect` and a `3-led-direct-aspect` both realising a future `signal-aspect-3-color` role).

## Consequences

**Positive**
- Facility slots bind by role across any style that realises it; multi-style roles (e.g., a future `signal-aspect-3-color` served by several LED arrangements) land without changing the slot-binding mechanism.
- Producer and consumer channels share a single schema and a single Channels-panel render path; no separate "lamp channel" parallel persistence.
- Constraint enforcement has a single source per channel (the channel's style), and the existing renderer carries it without modification.
- `lcc-rs` stays a pure protocol library — facility, role, style, ownership, and constraint-contract code lives in `bowties-core` and the frontend per Constitution IV.
- Cascade behavior on hardware-config change is uniform: hardware-owned channels disappear with their backing config, user-owned channels disappear with their owning slot, and either path uses the existing slot-detach pipeline to free Wired facilities.

**Negative**
- The channel-validation layer must enforce `style.declaredBindingKind === binding.kind` at runtime because styles live in YAML; a wrong YAML entry will not be caught by the Rust type system.
- The `channelType` retirement is a breaking change to the on-disk schema. Acceptable because Spec 018 ships no migration (FR-009) and the user manages pre-018 layouts manually.
- Pre-existing layouts that had renamed BOD-family channels under Spec 015 lose those names on the cleanup-slice retirement of the legacy per-input inventory. Acceptable for the same reason.

## Status

Accepted (2026-06-27) — Spec 018 design assessment.
