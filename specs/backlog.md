* Bug: Save regression — backend-owned snapshots should not round-trip through the frontend
  * Root cause: `save_layout_directory` accepts an optional `node_snapshots` vec from the
    frontend. After "Save As" sets `mode: offline_file`, subsequent "Save" passes
    `currentOfflineSnapshots` — which is empty because it was only populated when
    *opening* a layout, not after saving one. The backend receives `Some([])`, writes
    an empty companion directory via `save_directory_atomic`, and the atomic swap
    deletes all existing node YAML files and the CDI folder.
  * Fix approach (SRP / YAGNI — eliminate the data shuttle):
    1. **Backend** — remove the `node_snapshots` parameter from `save_layout_directory`.
       The backend resolves snapshots itself using two sources, in priority order:
       a. **Node registry** (live bus): if the bus is connected and the registry is
          non-empty, build fresh snapshots from `node_registry.get_all_handles()`.
          This is the existing "Save As after capture" path.
       b. **Existing companion dir** (re-save while offline or between captures):
          read the current `nodes/*.yaml` files back from disk. This preserves
          the on-disk state when no live registry is available.
       Source (a) takes priority so that a re-save while connected always picks up
       the latest live data.
    2. **Backend** — `write_companion_contents` must preserve the `cdi/` directory
       when no new CDI files are supplied. Currently `save_directory_atomic` does a
       full swap, so if `cdi_files` is empty the CDI folder vanishes. Options:
       a. Copy existing `cdi/*.xml` from the previous companion dir into the staging
          dir before the atomic swap (preferred — keeps atomic semantics).
       b. Or skip the atomic swap for the cdi subfolder and only swap nodes/changes.
    3. **Frontend** — remove `currentOfflineSnapshots` state, stop passing snapshots
       to `saveLayoutFile`. The API call becomes `saveLayoutFile(path, overwrite)`.
    4. **Frontend** — remove the `if (!layoutStore.isOfflineMode) captureLayoutSnapshot()`
       pre-save call; the backend handles this internally now.
    5. **Test**: add a Rust integration test that does write → re-write with empty
       `cdi_files` and verifies CDI files survive the second save.
  * Files affected:
    - `src-tauri/src/commands/layout_capture.rs` — `save_layout_directory`
    - `src-tauri/src/layout/io.rs` — `write_companion_contents`, `save_directory_atomic`
    - `src/lib/api/layout.ts` — `saveLayoutFile` signature
    - `src/routes/+page.svelte` — `saveCurrentCaptureToFile`, remove `currentOfflineSnapshots`
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