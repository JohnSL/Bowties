# API Contracts: CDI XML Viewer

**Feature**: 001-cdi-xml-viewer  
**Date**: February 16, 2026

## Overview

This directory contains API contracts for the CDI XML viewer feature, defining the interface between the Tauri backend (Rust) and SvelteKit frontend (TypeScript).

## Contract Files

- **tauri-commands.ts** - TypeScript interface definitions for Tauri commands
- **rust-signatures.md** - Rust function signatures for reference

All contracts use JSON serialization via Serde (Rust) and type-safe invocation via Tauri's `invoke` API (TypeScript).

---

## Contract Stability

- **Version**: 1.0.0 (initial)
- **Stability**: Draft (subject to change during implementation)
- **Breaking Changes**: Require version bump and frontend updates

---

## Error Handling Convention

All Tauri commands return `Result<T, String>` in Rust, which maps to:
- **Success**: Promise resolves with value of type `T`
- **Error**: Promise rejects with error message as `string`

Frontend must handle both success and rejection cases.
