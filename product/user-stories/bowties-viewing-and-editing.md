# Bowties Viewing And Editing

## Purpose

This document captures the current user stories for discovering existing bowties and performing the bowtie editing workflows that are implemented now.

It is validated against current implementation in the bowtie components, stores, orchestrators, and backend support, plus the implemented portions of specs 006 and 009.

## Validation Sources

- `specs/006-bowties-event-discovery/spec.md`
- `specs/009-editable-bowties/spec.md`
- `docs/design/design-bowtieMvp.md`
- `app/src/lib/components/Bowtie/**`
- `app/src/lib/stores/bowtieMetadata.svelte.ts`
- `app/src/lib/stores/bowties.svelte.ts`
- `app/src/lib/stores/connectionRequest.svelte.ts`
- `app/src/lib/utils/eventIds.ts`
- current component, store, route, and orchestrator tests covering bowtie discovery and editable flows

## Current User Story 1: View Existing Connections

A user opens Bowties on a layout whose nodes have already been read and sees the existing producer-consumer connections represented as bowties.

### Current Behavior

- Bowties discovers existing connections from current node/configuration data.
- The bowties view is not treated as ready until the prerequisite read/discovery work has completed.
- Existing connections are shown as bowtie cards that group related producers and consumers around a shared event relationship.
- Empty-state guidance is shown when there are no current bowties to display.

### What The User Gets

- a connection-first view of current layout logic
- a readable representation of producers, consumers, and their grouping
- a way to understand existing connections without tracing raw event IDs manually

## Current User Story 2: Create A Connection From The Bowties Tab Or From Configuration

A user wants to create a new connection visually instead of manually copying event IDs.

### Current Behavior

- The user can start from the Bowties tab with the current new-connection flow.
- The user can also start from configuration with a config-first entry point that pre-fills one side of the connection flow.
- The supported workflow uses actual producer and consumer selections; it is not the later planning-state empty-bowtie workflow.
- The system determines and applies event IDs according to the implemented creation rules rather than asking the user to manage raw IDs directly.
- After creation, the new bowtie appears in the bowties view and the related configuration surfaces reflect the new relationship.

### What The User Gets

- a visual connection-creation workflow
- current support for both bowties-first and config-first entry points
- immediate reflection of the new relationship in the product's current UI surfaces

### Current Limits

- The intent-first empty planning bowtie workflow is not part of the current durable user story.

## Current User Story 3: Add Or Remove Elements On Existing Bowties

A user wants to refine an existing connection by adding more producers or consumers, or by removing one.

### Current Behavior

- Existing bowties support adding producers or consumers where the current picker rules allow it.
- Existing bowties support removing connected elements.
- The implemented flow respects the current bowtie identity and slot-selection rules rather than creating unrelated duplicate connections.
- The product shows incomplete or adjusted bowtie state according to the current supported behavior after add/remove changes.

### What The User Gets

- iterative connection editing without leaving the bowties workflow
- connection updates that stay synchronized with current configuration state

## Current User Story 4: Clarify Ambiguous Event Roles When Needed

A user encounters an element whose event role is not known automatically and still needs to use it in a bowtie.

### Current Behavior

- Ambiguous elements are not silently treated as fully known producer or consumer entries.
- The current product flow can prompt for role clarification when needed during bowtie creation or editing.
- The resulting role decision is carried into the current editable bowtie workflow.

### What The User Gets

- a path to continue connection work even when automatic role determination is incomplete
- clearer separation between known roles and user-resolved roles

## Current User Story 5: Save And Restore Current Bowtie Metadata Through Layout Files

A user wants bowtie names and related metadata to survive beyond the current session.

### Current Behavior

- Bowtie editing participates in the current layout-backed save/discard lifecycle.
- Layout persistence stores current bowtie metadata and related state in the current file model.
- Reopening a saved layout restores the current supported bowtie metadata state.
- Bowtie edits and related configuration changes share the current save/discard model rather than acting as two unrelated persistence systems.

### What The User Gets

- durable bowtie metadata through the current layout model
- one coherent save/discard experience across supported current editing surfaces

### Current Limits

- Inline rename, filter-bar workflows, and later polish items that remain incomplete are not treated here as current guaranteed product behavior.

## Supported Outcome

Today, a user can:

1. view current discovered bowties after required reads complete
2. create supported bowties from either the Bowties tab or configuration
3. add and remove supported connected elements
4. resolve ambiguous event roles in the current implemented flow
5. save and restore the current supported bowtie state through layout persistence

That is the durable current user story for Bowties viewing and editing.