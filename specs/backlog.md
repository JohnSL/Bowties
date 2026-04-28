* Bowties offline derivation ownership
  * Root cause: offline bowtie visibility currently has frontend fallback logic that derives
    bowties from loaded trees when no live backend catalog is available. The canonical inclusion
    rules already live in the backend `build_bowtie_catalog`, so the frontend copy can drift and
    regress (for example, showing solitary events that are not real connections).
  * Fix approach:
    1. Move offline bowtie derivation into the backend so one authoritative rule set decides
       which bowties exist in both live and offline modes.
    2. Return an offline bowtie catalog, or a command to build one, from the backend during
       offline layout open instead of reconstructing connection membership in frontend stores.
    3. Reduce the frontend to rendering backend-owned results plus local display-only merge logic
       such as names, tags, and transient UI state.
    4. Add focused regression tests that prove live and offline bowtie inclusion/exclusion rules
       stay aligned.
  * Files affected:
    - `src-tauri/src/commands/bowties.rs` — authoritative bowtie catalog builder
    - `src-tauri/src/commands/layout_capture.rs` — offline open payload / backend-owned offline catalog
    - `src/lib/stores/bowties.svelte.ts` — remove duplicated inclusion logic after backend ownership exists
    - `src/lib/orchestration/offlineLayoutOrchestrator.ts` — consume backend-owned offline catalog
* LCC Traffic Monitor:
  * When we're getting text, show the actual text along side the bytes
  * Same for other data types
  * Have a check box that will show the byte data with the parsed results.
  * Show names for the message types
* Cache Location: The current location on my computer is `C:\Users\john_\AppData\Roaming\com.john.app\cdi_cache`. But that does match what we have in the architecture.md, which calls for `com.bowtiesapp.bowties` to be the directory.
* LCC Event Driver: Switch to always listening to LCC events, which we'll need for the event monitor anyway.
* Add app icon
* Dynamic SNIP & Config
  * If you modify SNIP information from LccPro, for example, the updates should appear right away
  * Same for if you save config from another app. The changes should appear immediately
* Release workflow publication polish
  * Root cause: the new skill-based `/release-publish` workflow now owns tag creation and release-notes generation, but the final GitHub draft-release publication step is still a manual paste-and-publish handoff.
  * Follow-up:
    1. Validate that the generated end-user markdown is consistently good enough to paste directly into the GitHub draft release without manual rewriting.
    2. If the manual publication step becomes a recurring pain point, decide later whether to add a verified GitHub CLI path without regressing the simpler manual workflow.
