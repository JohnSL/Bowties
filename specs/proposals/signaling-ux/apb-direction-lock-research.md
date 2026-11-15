# APB direction-lock pattern — external example

Status: **research lead, not a Bowties design decision.** This may inform a later
Automatic Block Signals / APB design; it does not change the first signaling slice or the
current facility-first storyboard.

Source: [Layout Command Control, "STL Syntax Question," messages #15883 and
#15885 (October 15, 2026)](https://groups.io/g/layoutcommandcontrol/topic/121780013).
The question asks how to recognize travel from a turnout block into its
neighbor, rather than travel in the opposite direction. The reply describes
an STL example used to set an APB *direction-of-traffic lock*.

## Candidate pattern

1. Retain the turnout block's previous occupancy in an `M` variable.
2. Detect its false-to-true transition with `FP`; this yields a one-scan pulse
   and does not pulse again until the turnout block has cleared and reoccupied.
3. During that pulse, test occupancy of the **entry-side neighboring block**
   and ensure the opposing direction lock is not active. If both conditions
   hold, set the direction lock.
4. Signal logic uses the lock to hold opposing signals at Stop.

This is a sequence-sensitive rule, not a combinational AND of two occupied
blocks: the neighbor must already be occupied when the turnout occupancy
rises. The order and identity of the entry neighbor determine which movement
is recognized. The original question also describes a different working
approach: arm a flag when the turnout is occupied and its neighbor is not,
then wait for the neighbor to become occupied, disarming if the turnout
clears. These patterns observe **different occupancy edges** and must not be
treated as interchangeable without checking the intended direction.

## Questions before adopting it

- For each governed direction, which block is the entry neighbor and which
  occupancy edge should establish direction, including trains entering with
  both blocks already occupied?
- What releases or reverses the lock, and how are conflicting locks,
  out-of-order events, detection gaps, and startup with occupied blocks
  handled? The thread does not specify those policies.
- Can the chosen hardware express and safely apply this stateful logic, and
  how would Bowties generate and test it without exposing STL in the default
  UX? The proposal currently treats APB direction as section-level state and
  defers APB coordination beyond the first slice.

Use the example as a test-case prompt for a later APB design, not as a ready
implementation recipe.
