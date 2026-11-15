# Bowties Mental Model

## Purpose

This document explains the Bowties product mental model: what the core event abstraction is, why the UI is organized around bowties and roles, and how the app hides Event IDs from users while managing them automatically.

This model is the user-experience foundation for the Bowties tab. Understanding it is required when adding features, designing new interactions, or writing UX-facing documentation.

---

## Core Abstraction

LCC devices communicate by producing and consuming Event IDs — 8-byte identifiers written into configuration slots on each node. In raw LCC tools, the user must track these IDs manually.

Bowties hides Event IDs entirely. The user never sees or types an Event ID. Instead:

| User sees | App handles invisibly |
|---|---|
| A named element: "Button 1 on East Panel" | Which producer event slot holds the ID |
| A named element: "Signal 4 on Tower Ctrl" | Which consumer event slot to write to |
| A named connection: "Yard to Signal 4" | What Event ID value is used |
| `+ Add producer` / `+ Add consumer` | Finding the first free event slot |
| Edit / Remove on a card | Reading the current slot value, writing new or clearing |

The only time an Event ID surfaces is as a small secondary detail in the element picker — not as something the user must understand or act on.

---

## Bowtie Card Model

A **bowtie** represents one logical connection: a shared Event ID linking one or more producers to one or more consumers.

Each bowtie card shows:

- **Name** — a user-given label for the connection (e.g., "Yard to Signal 4")
- **Producers** — elements that produce the shared Event ID (labeled with node name, element name, CDI breadcrumb)
- **Consumers** — elements that consume the shared Event ID (same label structure)
- **`+ Add producer` / `+ Add consumer`** — to extend the connection

The Event ID "belongs to the bowtie". When creating a new connection, the producer's existing Event ID becomes the bowtie's identity. When adding an element to an existing bowtie, the newcomer's slot is written with the bowtie's existing Event ID.

---

## Role Classification

Each element in a bowtie carries a role classification:

- **Producer** — the node/element that fires the event (e.g., a button press)
- **Consumer** — the node/element that responds to the event (e.g., a signal output)

Role classification is stored in the layout file's bowtie metadata. The backend stores them per bowtie per element via the `role_classification` field. In the UI, roles are displayed on cards and used when filtering the element picker.

---

## What Drives The Catalog

The bowtie catalog is built by the backend after:

1. CDI reads complete for all discovered nodes.
2. The Identify Events exchange completes.

The backend then emits a `cdi-read-complete` event with a rebuilt `BowtieCatalog`. The frontend `bowtieCatalogStore` receives this and publishes the new catalog.

**The catalog is not manually maintained.** It is always derived fresh from the current bus state after reads complete. This means the catalog reflects only the current bus config — it does not include offline changes that have not yet been synced.

---

## Pending Edits And The Editable Preview

The frontend builds an editable preview by merging:

- **Catalog** — the backend-derived bowtie list from the last `cdi-read-complete`
- **Pending node tree modifications** — modified event slot values in `nodeTreeStore` (tracked by the Rust tree's `modified_value` field)
- **Pending metadata edits** — name, tag, and role-classification changes in `bowtieMetadataStore`

This merged view (`EditableBowtiePreview`) is what the UI renders. Saved edits update both the node config on the bus and the layout file.

**Owner:** `bowties.svelte.ts` — `editableBowtiePreview` derived getter

---

## Creating A Connection

1. User clicks `+ New Connection`.
2. The `NewConnectionDialog` opens with producer picker and consumer picker.
3. User selects one element on each side and optionally names the connection.
4. The app:
   - Inspects the consumer element's event slots and finds the first free one.
   - Writes the producer's Event ID to that consumer slot on the physical node.
   - Records a bowtie-create edit in `bowtieMetadataStore`.
   - The dialog closes and a bowtie card appears.

**Key rule:** Creating a connection requires CDI reads to have completed for both nodes involved, so that free-slot detection works correctly.

---

## Adding To An Existing Bowtie

When the user adds a producer or consumer to an existing bowtie:

- The element picker filters to elements with at least one free event slot in the correct role.
- The new element's slot is written with the bowtie's **existing** Event ID.
- The bowtie card updates to show the new element.

---

## Empty State

Before any connections exist (either no CDI reads, or no event slots wired), the Bowties tab shows an empty-state message guiding the user to start with `+ New Connection`.

Once CDI reads complete and connections exist, the tab renders the full bowtie card list.

---

## Offline Mode

In offline mode (layout open, bus disconnected), the Bowties tab shows the bowtie data last captured in the layout file. The user can make metadata edits (rename, re-tag, role reclassification). Event slot writes require a live bus connection and are queued as offline changes for later sync.

---

## Relationship To The Configuration Tab

The Configuration tab shows raw CDI structure — nodes, segments, groups, individual fields. Every event slot in the Configuration tab shows a "Used in: [connection name]" cross-reference when that slot is part of a known bowtie. Clicking the cross-reference switches to the Bowties tab and highlights the relevant card.

---

## Automatic Block Signals Preview

The Railroad tab's **Add facility** dialog also starts a transient Automatic Block Signals
preview. It does not create a facility, configuration draft, output claim, or
bus write. The modeler chooses an existing detected block, then names the Line
and its entrance signal and chooses a presentation orientation. The preview
states that the signal protects that block only: occupied expects Stop and clear
expects Clear. An unavailable observation remains unavailable rather than being
shown as a live state. Display, binding, confirmation, saving, and application
belong to later workflow steps.

Both Block Indicator and Automatic Block Signals require an initial name in Add facility.
That same name appears in the Line wizard and remains editable. The block picker
lists candidates alphabetically; this does not reorder a Line's travel order.
Missing hardware bindings still make the proposal incomplete but do not produce
a routine notice on pages without save/apply actions.

The preview is a wide five-step wizard, not an inline facility form. Blocks
and Signals are usable in S1; Display, Bindings and Confirm remain numbered
but visibly unavailable. The Blocks membership row reports unavailable
occupancy truthfully and can remove only the Line membership, never the shared
block or detector. The Signals card separates hypothetical Stop/Clear
expectations from live observation and uses a neutral oriented entrance marker,
not a red lamp that would imply an observed Stop.
Blocks uses a searchable, scrollable Add block selection dialog and membership
card, with the Line name label and input on one row. Signals retains the
storyboard's selected-signal name, Entire Line policy summary and separate
When-this-signal-shows table. Cancel/close reset the transient editor through
orchestration. Cancel stays flush left; Back/Next stay right. The track strip
shows block occupancy separately from the neutral entrance-signal selection tile
below it. On narrow screens, cards stack without horizontal overflow.

The Line wizard opts into contained dialog layout. On roomy windows, the
stepper, track/name context and footer stay stationary while the policy and
indication panes scroll independently. On narrow or short windows, one inner
workspace scrolls instead; the outer dialog body does not add a second scrollbar.
The rail labels share explicit line height and equal rail-edge gaps.

Automatic Block Signals is additive: Block Indicator facilities, lamps, blocks and detectors
remain unchanged. The Line references a shared block and owns only its entrance
signal. S1 supports one block; selecting more produces an explicit unsupported
count issue, not several fabricated terminal signals. A transient Line identity
scopes signal identities across editor requests, including renaming and diagram
reorientation. The shared result explicitly has `incompletePreview` capability
with staging, saving and application unavailable even when detection resolves.

---

## Sources

- `app/src/lib/stores/bowties.svelte.ts`
- `app/src/lib/stores/bowtieMetadata.svelte.ts`
- `app/src/lib/stores/connectionRequest.svelte.ts`
- `docs/design/design-bowtieMvp.md` (original design — offline mode, pending-edits, and cross-ref are extensions beyond MVP)
