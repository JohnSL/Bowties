# Import existing signal configuration as an Automatic Block Signals facility

- **Areas**: signaling, layout, backend, architecture
- **Origin**: Spec 020 (Automatic Block Signals), bench testing after slice E2a, 2026-11-22
- **Status**: deferred
- **Date**: 2026-11-22

A Signal LCC can already hold masts, Logic conditionals and Direct Lamp rows configured outside Bowties (by hand, JMRI, another tool, or earlier testing). Bowties currently shows these only as pins and slots in use (named and linked to Configuration on the Wiring step; slice E2c adds an explicit, undoable release). This idea adds the third option: recognize existing configuration that matches the Automatic Block Signals recipe and offer to adopt it as a facility, so the modeler keeps working wiring instead of rebuilding it.

## Prior Work

- Adoption is the compiler run in reverse: read the node's mast and Logic configuration, match it against the recipe the compiler emits, and build the facility record (blocks, direction, signals, outputs) plus its claimed slots. The recognizer belongs next to the pure compiler in `bowties-core/src/signaling/`, with the backend gathering facts as it does for compose (ADR-0015).
- Ownership facts already exist: `signal_outputs_from_configuration` (core `signaling/outputs.rs`) reports the mast or Direct Lamp row that drives each pin, and facilities record claimed slots, so "untracked" is computable today.
- Wait until the recipe set is stable (after spec 020 E3 Approach and E5 cross-board receivers). Each new recipe needs a recognizer, so building this early means repeated rework.

## Open Questions

- Partial or hand-modified matches: adopt with a warning, offer to normalize (staged as unsaved edits), or refuse?
- Which blocks a recognized mast protects: the occupancy event IDs in its Logic map back to detection channels, but a mast with no matching channel cannot be adopted as-is.
- Should adoption run on demand (per node) or be suggested when a Signal LCC's configuration is read?
- Signal identity: adopted signals get new IDs; is there any existing identity worth preserving?
