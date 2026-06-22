* LCC Traffic Monitor:
  * When we're getting text, show the actual text along side the bytes
  * Same for other data types
  * Have a check box that will show the byte data with the parsed results.
  * Show names for the message types
* SPROG USB-LCC CDI read timeouts (Issue #14): Discovery works but multi-frame datagram reads
  time out. Suspected causes: byte-at-a-time serial reads (vs JMRI's 128-byte buffered reads),
  per-frame `flush()` adding latency, or datagram ACK timing issues under load. Needs diagnostic
  frame logging for the user to capture a trace, and/or switching to chunked buffered reads.
* Cache Location: The current location on my computer is `C:\Users\john_\AppData\Roaming\com.john.app\cdi_cache`. But that does match what we have in the architecture.md, which calls for `com.bowtiesapp.bowties` to be the directory.
* LCC Event Driver: Switch to always listening to LCC events, which we'll need for the event monitor anyway.
* Add app icon
* Dynamic SNIP & Config
  * If you modify SNIP information from LccPro, for example, the updates should appear right away
  * Same for if you save config from another app. The changes should appear immediately
* Cascade profile rules for ConfigEditor
  * Root cause: ConfigEditor starts as a pass-through (no cascade logic). When a controlling field
    like a daughter board selector changes, dependent fields may need corrective default writes.
    Today this is handled manually or not at all.
  * Fix approach: author cascade rules in `.profile.yaml` alongside existing relevance rules, using
    the same extraction pipeline. ConfigEditor reads these rules and applies synchronous cascade
    corrections within `applyEdit()`.
  * Prerequisite met: edit layer refactor (changes module + ConfigEditor) is complete.
* Release workflow publication polish
  * Root cause: the new skill-based `/release-publish` workflow now owns tag creation and release-notes generation, but the final GitHub draft-release publication step is still a manual paste-and-publish handoff.
  * Follow-up:
    1. Validate that the generated end-user markdown is consistently good enough to paste directly into the GitHub draft release without manual rewriting.
    2. If the manual publication step becomes a recurring pain point, decide later whether to add a verified GitHub CLI path without regressing the simpler manual workflow.
* Connector daughterboard Signal-LCC authoring evidence
  * Root cause: The current implementation ships Signal-LCC aux-port selection and persistence support, but the workspace still does not contain equivalent Signal-LCC CDI/manual path evidence for aux-port-governed sections, so those profiles intentionally leave `affectedPaths` empty.
  * Follow-up:
    1. Acquire concrete Signal-LCC CDI or manual path evidence for aux-port-governed sections and line modes.
    2. Author Signal-LCC affected paths and any carrier-specific overrides once those concrete paths are verified.
* Mixed-use BOD4-CP sampled/output half constraints
  * Root cause: Connector rules now support slot-relative `lineOrdinals`, so Bowties can constrain the detector half of BOD4/BOD4-CP accurately. The remaining gap is richer cross-field modeling for the BOD4-CP sampled/output half (local lines 5-8), where the manual allows multiple valid steady, pulse, and sample combinations depending on the attached device.
  * Follow-up:
    1. Capture concrete Tower-LCC-compatible mappings for the BOD4-CP local lines 5-8 output modes and corresponding sampled input modes.
    2. Extend repair/constraint authoring if needed so Bowties can express output-function and input-function combinations for the BOD4-CP shared lines without hiding valid steady-output use cases.
