# User Stories

## Purpose

This folder holds validated user-story documents for Bowties as the product behaves now.

These documents are not feature plans. They describe durable current user-visible workflows that have been cross-checked against active specs, current code, and current tests.

## Validation Rules

- Use `specs/**` as source material only.
- Confirm the story against current implementation before documenting it here.
- Do not promote speculative or incomplete stories into `product/user-stories/`.
- When a story is only partially implemented, document only the current supported scope or wait until the behavior is complete.

## Current User-Story Set

- `connect-discover-read-configuration.md` — current online workflow from connection through discovery, configuration reading, and configuration browsing
- `bowties-viewing-and-editing.md` — current workflow for viewing discovered connections and creating or editing supported bowties
- `offline-capture-edit-sync.md` — current offline capture, edit, save, reconnect, and sync loop

## Deferred Until More Fully Implemented

These areas should not yet be documented here as complete product user stories:

- full online configuration editing as a standalone current workflow if it depends on incomplete acceptance paths
- intent-first empty bowtie planning workflows
- inline bowtie rename and filter-bar workflows that are still listed as remaining work
- guided-configuration workflows that are not yet current end-user product behavior

## Maintenance Rules

- Keep each user-story document focused on what a user can do now.
- Name the entry points, visible states, and key results of the workflow.
- Call out current limitations when they affect what users can reliably do.
- Update the corresponding document when intentional product behavior changes.