# Spec 012: Copilot Knowledge Base

## Summary

Build a structured knowledge base that gives AI coding agents fast, accurate orientation in the Bowties codebase. The knowledge base spans three tiers:

1. **product/glossary.md** — canonical domain vocabulary for humans and AI
2. **aiwiki/** — code-level navigation (WHERE + HOW) for AI: module ownership, workflow participation, shared conventions, test mapping
3. **product/architecture/adr/** — lightweight architecture decision records

Supporting infrastructure includes structured prior-work capture (`specs/ideas/`), updated skills and prompts, and hardened always-on instructions.

## Goals

- **Orientation speed**: AI discovers codebase structure via markdown, not code exploration
- **Architecture protection**: AI checks ownership, duplication, and placement before implementing
- **Prior-work reuse**: deferred ideas are discoverable and reusable when relevant work begins

## Precedence

`product/ + code` > `aiwiki/` > `specs/`

## Tracking

Progress tracked in [bootstrap-checklist.md](bootstrap-checklist.md).
