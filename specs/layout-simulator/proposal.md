# Proposal: a desktop track simulator for testing Bowties

**Status:** Draft for discussion. The simulator replaces the model railroad on the test bench, not the LCC nodes.

## What an operator can test

Load a track plan, place a train, choose its direction, and move it into a detected block. The simulator changes the detector input at a real LCC node. Bowties and the nodes interact as they would on a physical railroad. If a node drives a signal lamp, the desktop app shows the lamp output the interface board actually senses. If the lamp stays green when the operator expects red, the screen stays green: exposing that discrepancy is the point of the test. An unread output is unknown, not an unlit lamp.

The aim is to repeat this exercise on different track arrangements for block signaling, interlockings, and CTC without rebuilding a physical railroad for each one.

## Train movement and occupancy

Step 3 of 5, **Appearance**, in the *Automatic Block Signals* storyboard illustrates one possible interaction for the track diagram:

- **Move a train:** Place it in A and choose its direction toward adjacent block B. On the first **Step**, the front enters B while the rear remains in A. Both detector inputs stay occupied until the operator acts again. On the second **Step**, the rear clears A and the train is wholly in B.
- **Place standing cars:** Click a block to add or remove standing cars independently of the moving train. Standing cars occupy track but have no direction of travel.
- **Line a hand-thrown turnout:** Click it to select the main or the siding. The train follows the lined track without a second route choice. Passing through the turnout can take multiple steps; it need not be a single jump onto the selected track. Lining a hand-thrown turnout is an operator action, distinct from a switch-machine command from a node.

A detected block stays occupied until the last train or standing car leaves it. For example, place standing cars and an eastbound train in A, then step the train into B. The detector inputs for **both A and B** should indicate occupied: the cars still occupy A. Once the train is fully in B, removing the cars from A clears A without clearing B.

The simulator borrows the storyboard's movement interaction, not its signal preview. The bench app displays only the lamp outputs read from the real nodes.

## The test bench

| Direction | Example | Path |
| --- | --- | --- |
| Simulator to node | A train occupies a block; an operator presses a push button | Desktop app to ESP32 over USB, then over I2C to an interface board wired to a node input |
| Node to simulator | A node energizes a lamp or commands a switch machine | Node output to interface board, then back through the ESP32 to the desktop app |

A switch-machine command and the simulated turnout reaching its commanded position are distinct. The proposal still needs to define how turnout travel and position feedback work.

## Track plans and wiring

Start with a fixed collection of file-defined track plans. Each plan defines a **virtual layout**: where trains can go, which sections report occupancy, and how the simulated devices map to physical interface-board pins. The bench wiring is part of the plan. For example, the simulator needs to know that *Block B detector → interface-board output 3*. A wiring note can tell the operator where to connect that output, but driving it does not require an LCC node ID, event ID, or configuration value.

The desktop simulator connects by USB to the ESP32. Its I2C interface boards drive and sense the real nodes' terminals in place of layout devices. Bowties still communicates with those nodes over LCC, just as it would on a physical railroad. The simulator's track plan is distinct from a Bowties Layout, which stores LCC configuration.

YAML is a candidate, not a requirement. AI could draft plans in the same format a person edits; running a test should not depend on AI. Before driving hardware, the app should flag mistakes such as a detector mapped to a sense-only connection.

## Switching layouts on the same bench

Loading a different track plan changes what the simulator drives and senses over USB. It does **not** reconfigure the LCC nodes. Even with the same boards and cables, a node configured for one layout may interpret a pin differently on another.

The proposed companion feature belongs in **Bowties**: restore a saved Bowties Layout by writing its configuration values back to every LCC node in that layout. For a changeover, the operator would stop the old simulation, check the wiring against the new plan, restore the corresponding layout in Bowties, and then run the new track plan. The simulator does not read the Bowties Layout, issue LCC commands, or decide whether the restore succeeded.

## Suggested first bench test

Use two adjacent detected blocks, no standing cars, and one signal with readable lamp outputs. Start the train in A. Check the occupied detector inputs before moving (**A only**), after the first click (**A and B**), and after the second click (**B only**). At each pause, observe the signal. Repeat with the node deliberately configured to drive a different lamp. The displayed lamp must follow the hardware in both runs without changing the track plan.

## Decisions still open

- Whether to offer dragging as another way to move a train along the lined track, and how a drag would expose the same block-overlap state for testing.
- Which intermediate positions an operator needs when stepping through a turnout.
- Separately from an intentional layout change, what the interface boards should drive or release if USB disconnects unexpectedly.
