---
applyTo: "docs/**,specs/**,aiwiki/**"
description: "Use when editing Bowties documentation, planning artifacts, or AI navigation docs. Validate the artifact without running application tests for content-only changes."
---

# Non-Source Content

Files in this scope document, plan, or explain Bowties; they do not by themselves change executable behavior.

- When the baseline-relative task changes are confined to this scope, do not run Bowties application tests, builds, type checks, coverage, or runtime diagnostics.
- Run artifact-specific validation only when it exists and applies, such as a link check, schema check, customization diagnostic, or targeted rendering check.
- A file's extension is not enough to classify it as non-source. Generated shipping artifacts, profiles, dependency/configuration files, and other runtime-consumed content follow `product/quality/testing-strategy.md`.
- If the task also changes source, tests, build scripts, dependency manifests, generated runtime assets, or runtime configuration, select validation for the complete task change set using `product/quality/testing-strategy.md`.
- When application tests are not applicable, report: `Tests not run: changed files contain no source, test, dependency, build, generated runtime asset, or runtime configuration changes.`
