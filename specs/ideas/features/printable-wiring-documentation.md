# Printable Wiring Documentation

- **Areas**: layout, signaling, documentation
- **Origin**: Spec 020 (Automatic Block Signals), conversation of 2026-11-25 about the Railroad tab's facility card
- **Status**: deferred
- **Date**: 2026-11-25

A view, possibly its own "Wiring" tab, that documents how the layout is wired and can be printed as a record to keep next to the layout. For each board it lists what drives each output: the signals Bowties manages with their lamp pins, and the masts and lamps configured on the board that no facility manages. This is the information a modeler needs when tracing a wire or replacing a board, which is a different question from "what is this facility?" that the Railroad tab's facility card answers.

## Prior Work

### Where this came from

The Spec 020 storyboard (`specs/proposals/signaling-ux/signaling-facilities-storyboard.html`) once drew an "After Done — Railroad panel" frame that was never reviewed. It grouped content by Signal LCC board:

- A header per board: board name and Node ID.
- A table of the signals Bowties manages on that board: name, role, pins (for example `#3 H1-R / #1 H1-G`), and the facility that uses them.
- A line, "In use outside Bowties", listing each mast or Direct Lamp row that drives a pin but is not claimed by a facility, with its pins and a link to that row in the Config tab. The line was omitted when nothing was left over.

On 2026-11-25 the Railroad tab's facility card moved to a read-only track diagram instead (see the storyboard's "After Done — Railroad tab" frame), and that board table was set aside for this idea.

### Data that already exists

- Each signal's channel records its board and red and green outputs (`signalLccRgOutputs` binding); `FacilityCard.svelte` already formats pin labels with `signalOutputLabel`.
- The Wiring step's pin grid already classifies every pin on a board as this signal's, another facility's, or in use outside Bowties, so the "outside Bowties" list has an existing source.

### Open questions

- A tab of its own, a print view of the Railroad tab, or an export (PDF or similar)?
- Group by board (what's on this board?), by facility, or offer both?
- Should it cover other facility types (Block Indicator lamps, inputs for occupancy detectors) from the start, or begin with signals only?
- How should a board that has not been read yet appear: omitted, or listed with its pins unknown?
