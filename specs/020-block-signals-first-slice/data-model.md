# Data Model: One-Direction Automatic Block Signals

**Status**: Proposed extensions to existing facilities/layout contracts, not implemented types.
**Owner**: `bowties-core` owns serializable domain values and deterministic transitions; the backend supplies authoritative snapshots and lifecycle evidence.

## Automatic Block Signals facility

Persist within the existing facilities document and draft delta pipeline.

| Field | Meaning / rule |
|---|---|
| `id` | Stable facility identity; reuse existing facility ID convention |
| `name` | Editable nonempty display name, using existing naming validation |
| `diagramDirection` | `eastward` or `westward`; metadata only |
| `members` | Nonempty ordered list of `{ blockId, signalId }` in travel order |
| `signals` | Exactly one owned signal record for each member |
| `generatedOwnership` | Proven per-node resources and generated configuration attributable to this Line |
| `recipeEvidence` | Versioned recipe/profile compatibility reference sufficient to interpret generated resources |

Use existing Block/facility references rather than copying detector configuration. No duplicate block membership or signal identity is allowed within one Line. A signal is retained by protected-block membership; reorder does not recreate it. Removing the final block requires Line removal.

## Entrance signal

| Field | Meaning / rule |
|---|---|
| `id` | Stable signal identity, allocated once |
| `name` | Editable name; default creation names are not reopen defaults |
| `protectedBlockId` | Must equal its membership reference |
| `binding.nodeId` | Existing canonical uppercase dotless real NodeID; incompatible/offline unresolved hosts cannot be applied |
| `binding.redOutput` | Explicit physical output address, resolved using profile bindings |
| `binding.greenOutput` | Separate physical output address on the same host; must differ from red |
| `binding.display` | Supported two-driver RG mapping: red=Stop, red+green=Approach, green=Clear; no subjective color-approval field |

Downstream signal, internal/terminal role and expected aspect are derived, not separately editable persisted graphs. The next signal is `members[i + 1]`, regardless of orientation or host.

## Block detection input

Resolve from existing named blocks into `{ blockId, detectionReference, occupiedInput, clearInput, compatibility }` using the existing detection owner. These are compiler facts, not a second saved detector definition.

Missing, ambiguous or incompatible detection blocks application. Unknown occupancy is not equivalent to clear.

## Supported recipe evidence

Identifies supported node/profile/CDI/firmware scope, technically compatible output wiring, tested RG output activation and speed publications/comparisons, exact generated row roles and resource requirements. Perceived blended color is not recipe evidence or a validation gate.

Semantic rule/Track Speed and lamp appearance are separate: Approach publishes Approach whether or not a modeler likes the red-plus-green blend. The first mapping uses two drivers; no yellow output or flashing substitute is required.

Keep human bench records separate from the machine-consumed, versioned recipe. During implementation, translate recorded selectors through actual CDI values; labels/ordinal position alone are not wire encodings. Evidence is provided by the backend-supported recipe, not a client-controlled `validated=true` flag.

## Generated resources and ownership

`GeneratedResource` contains `{ nodeId, kind, resolvedPath, ownerLineId, ownerSignalId, purpose, expectedConfiguration }`.

Kinds cover mast/rule rows, conditionals, receiving circuits, driver assignments and generated event/link references as actually used by the tested recipe. Record old and new hosts when rebinding. Track Circuit Link Address resolution must follow the device's real allocation/publication semantics.

Ownership is an extension of existing facility/node state, not an independent global registry. Availability includes unrelated current configuration, existing Line claims and conflicting drafts. Preview claims are hypothetical; saved pending proposals are not proof of applied hardware ownership.

Release requires proven ownership and a complete reference check. Shared block detectors and unrelated driver/logic uses are never released.

## Line change proposal

| Field | Meaning |
|---|---|
| `proposalId`, `revision` | Backend-issued identity for an immutable review |
| `operation` | `create`, `edit`, `remove` |
| `facilityId`, `before`, `after` | Effective facility change; absent side for create/remove |
| `baseline` | Layout/draft generation plus per-node snapshot fingerprints |
| `facilityDeltas` | Existing layout-delta shape extended for Line records |
| `nodeChanges` | Ordered per-node patches, owned allocations/releases and reference changes |
| `issues` | Actionable validation failures/warnings with node, signal, field/resource and remedy |
| `hardwareChangeRequired` | Derived from actual hardware projection, never from orientation alone |

A patch records `{ changeId, nodeId, path, beforeValue, afterValue, owner, purpose }`, using existing typed config value/path primitives. Missing preimages are validation errors for destructive work.

Proposal generation is deterministic for equivalent normalized input; generated proposal IDs/timestamps are excluded from parity comparison. Stable existing signal/resource IDs remain significant.

## Apply/readback evidence

Extend current backend sync/save results and recovery ownership to retain:

- Operation/proposal identity and reviewed revision.
- Per-node status: `pending`, `writing`, `written`, `verified`, `failed`, `unknown`.
- Per-patch outcome: acknowledged write, explicit failure, not attempted, or unknown; preimage/intended value and actual readback when available.
- Verification status: `pending`, `matches`, `diverged`, `unavailable`, with actual values/errors.
- Safe recovery action and affected old/new node references.

`written` is not `verified`. A failed node may contain acknowledged earlier writes; `unknown` is not untouched. Report the aggregate as partial/failed/unverified until all required patches and releases are verified. Metadata-only saves may complete without a hardware operation.

Recovery records survive save/reopen. Do not discard the ownership needed to finish or investigate a partially applied removal. Retry requires fresh readback and a revalidated proposal.

## Observation versus expectation

Observation: `{ value, source, availability, observedAt, connectionGeneration }`, with `availability` distinguishing current, unavailable and stale according to existing observation/freshness conventions.

Expectation: `{ aspect, inputs, reason }` computed from explicitly supplied input states. If required input is unavailable, expectation is unavailable/indeterminate, not a fabricated live aspect.

Neither layer promises firmware stale-input detection or automatic fail-safe behavior.

## State transitions

```text
editor input -> validated preview -> staged draft -> saved pending changes
                                \-> cancel/discard (no committed hardware claims)
saved pending changes -> writing -> written -> readback verified
                           \-> partial/failed/unknown -> fresh readback -> new recovery proposal
verified Line -> metadata edit -> save (no writes for orientation only)
verified Line -> operational edit/removal -> review -> same draft/apply lifecycle
```

Reading/reopening reconciles saved intended state, node observations and operation evidence rather than overwriting any of them with example defaults.

## Compatibility and migration

Retain existing facilities, block references and ID conventions. Old layouts without Lines load unchanged. Extend effective facilities/deltas, IO and save/open results coherently. Manifest schema version 4 and legacy layout-file `"1.0"` are distinct; use the existing migration policy if required.

Test unknown variant/schema handling explicitly. Malformed Line membership, orphan signals, unresolved ownership and conflicting bindings produce visible errors, not silent repair or omission.
