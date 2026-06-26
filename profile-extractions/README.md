# Profile Extractions

This folder holds the raw outputs of the profile extraction pipeline (skills `profile-0` through `profile-6`). Each subfolder corresponds to one board model and contains:

- `manual-outline.json` — structured outline of the board's PDF manual
- `event-roles.json` — producer/consumer role classifications
- `relevance-rules.json` — conditional relevance rules
- `section-descriptions.yaml` — section-level descriptions
- `field-descriptions.yaml` — field and option descriptions
- `recipes.yaml` — step-by-step configuration recipes
- `validation-report.json` — cross-reference validation results
- CDI XML and PDF manuals used as source evidence

These files are **not** the runtime profiles that the app ships. The production `.profile.yaml` files are assembled from these extraction outputs and live in [`app/src-tauri/profiles/`](../app/src-tauri/profiles/). The app's profile loader reads from that directory (see `app/src-tauri/src/profile/loader.rs`).
