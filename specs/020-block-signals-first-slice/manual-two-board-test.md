# Two-Board Values: S4r Receiving S2r Track Speed

**Updated**: 2026-11-07

**Recorded result, 2026-11-07**: Cross-board Stop/Clear reception passed after correcting board A Circuit 1 to the publisher's freshly read hardware Link Address, `02.01.57.10.09.96.07.30`. JMRI refresh showed this address; attempted writes of `02.01.57.10.09.96.04.68` to board B Mast 1 were not retained. The saved layout had `...04.68` in both fields, but this agreement did not establish the actual publisher address. S4r then showed green with Bt clear and yellow-looking red+green with Bt occupied, with matching command/acknowledgement evidence. This is a partial acceptance result: cross-board Approach publication/reception, receiver restart, the final trigger used, and sustained quiet-bus behavior remain unverified.

Evidence: [before correction](evidence/2026-11-07-two-board-before-link-correction.txt) and [after correction](evidence/2026-11-07-two-board-after-link-correction.txt). Both are mid-session JMRI captures without boot frames; node identities come from JMRI's decoded labels.
**Prerequisite**: The single-board S4r/S3r test passed on 2026-11-07. Its trace showed S3r speed publications followed by S4r's correct Approach/Clear commands and acknowledgements. Keep the actual working receiver on **Blocks & Signals 1: Mast 1, Logic 7, Circuit 1**, with Logic 8 blocked.

This changes the publisher's location, not the comparison being measured. Configuration values are grouped by the Bowties selectors; no Bowties/reset tutorial or direct rule-trigger tool is required.

**Baseline**: On **Signals 2**, use the unused Mast 1 and Logic 1/2 slots. The inspected snapshot has no active masts/conditionals or Direct Lamp Control claims on outputs #11/#14. Do not reset **Blocks & Signals 1**, change its unrelated configuration, or change the Tower-LCC. Apply the listed values to hardware before measuring.

## Arrangement

| Board | Selection | Role |
|---|---|---|
| A: Blocks & Signals 1 | Mast 1 / S4r, Logic 7, Circuit 1 | Keep the working receiver/probe |
| A: Blocks & Signals 1 | Logic 1/2 and Mast 2 / S3r | Old local publisher; disable only these when switching to board B |
| A: Blocks & Signals 1 | Logic 8 | Keep Blocked |
| B: Signals 2 | Mast 1 / S2r | New publisher, physical green #14 H4-Y, physical red #11 H3-R |
| B: Signals 2 | Logic 1 and Logic 2 | Same Bt/B2 input group used to select publisher speeds |

S2r is a wired test publisher, not a claim that it is S4r's immediate next signal. Bt/B2 again serve only to generate known Stop, Approach and Clear publications.

On board B the actual C7c dropdown labels output #14 **H4-Y**, although your physical S2r green is wired there. Select by the output number; the CDI's printed color is not the connected lamp color.

## Keep the proven board A receiver values

These are verification values, not a new receiver configuration:

| Selection on board A | Field | Keep |
|---|---|---|
| Mast 1 / S4r | Mast Processing | Normal |
| Logic 7 | Function | Last (Single) |
| Logic 7 | Logic Operation | V1 Only |
| Logic 7 / Variable #1 | Source | Track Circuit 1 |
| Logic 7 / Variable #1 | Trigger | **On Variable Change** |
| Logic 7 / Variable #1 | Track Speed | Stop |
| Logic 7 / Variable #2 | Trigger | None |
| Logic 7 / Action 1 | Condition | Immediate if True |
| Logic 7 / Action 1 | Action Event | Existing copy of **board A / Mast 1 / Rule 2 / set aspect** |
| Logic 7 / Action 2 | Condition | Immediate if False |
| Logic 7 / Action 2 | Action Event | Existing copy of **board A / Mast 1 / Rule 3 / set aspect** |
| Logic 7 / Actions 3-4 | Condition | none |
| Logic 7 | Action when true / false | Send then Exit Group |
| Logic 7 | Time Delay | 0 milliseconds |
| Logic 8 | Function | Blocked |

The Track Circuit probe's original `On Matching Event` setting produced repeated Approach commands. `On Variable Change` stopped the flood and retained the required three-state mapping on this board. Do not swap the two action events or change this trigger during the cross-board test. The detector-fed publisher group on board B still uses `On Matching Event`.

## 1. On board B, select Rule to Aspect / Mast 1

Keep processing `Unused` while configuring the rules; final value is `Normal`.

| Field | Value |
|---|---|
| Mast Description | `S2r cross-board publisher` |
| Lamp Fade | None |
| Link Address | Keep generated value |
| Final Mast Processing | Normal |

Read the generated Link Address back from hardware before copying it to board A. On the tested C7c board, writes to this mast field were not retained despite its CDI "Limited Write" wording. Do not use an edited/offline value as proof of the publisher's address. This test's confirmed hardware value is `02.01.57.10.09.96.07.30`; other masts/nodes may have different generated values.

### Select the Rule

| Rule selection | Rule Name | Track Speed |
|---|---|---|
| Rule 1 | 0-Stop | Stop |
| Rule 2 | 21-Approach | Approach |
| Rule 3 | 29-Clear | Clear/Proceed |

Keep generated `set aspect`, `aspect is set` and `aspect cleared` events. Leave Rules 4-8 at defaults.

### Select the Rule, then Appearance Lamp

| Rule | Appearance selection | Lamp Selection | Lamp Phase (A-B) - Flash Rate | Appearance Effects | Effects Lamp |
|---|---|---|---|---|---|
| 1: Stop | Lamp 1 | #11 H3-R | Steady | None | Unused |
| 2: Approach | Lamp 1 | #11 H3-R | Steady | None | Unused |
| 2: Approach | Lamp 2 | #14 H4-Y | Steady | None | Unused |
| 3: Clear | Lamp 1 | #14 H4-Y | Steady | None | Unused |

For every other Appearance Lamp in Rules 1-3: **Lamp Selection = Unused**, **Appearance Effects = None**, **Effects Lamp = Unused**; leave phase at default. No Direct Lamp Control row may also claim #11 or #14; those outputs belong to this test mast. Leave unrelated outputs unchanged.

## 2. On board B, select Logic 1, then Logic 2

Keep both `Blocked` while entering values. Enable Logic 2 as `Last (Single)` before enabling Logic 1 as `Group`. All other board B conditionals remain `Blocked`.

### Logic-level fields

| Field | Logic 1 | Logic 2 |
|---|---|---|
| Description | `S2r Bt Stop priority` | `S2r B2 Approach else Clear` |
| Final Function | Group | Last (Single) |
| Logic Operation | V1 Only | V1 Only |
| Action when true | Send then Exit Group | Send then Exit Group |
| Action when false | Evaluate Next | Send then Exit Group |
| Time Delay | 0 milliseconds | 0 milliseconds |

Leave delay retrigger at its reset default.

### Select Variable #1 under each Logic

Use the [existing detector copy sources](manual-track-circuit-test.md#4-detector-event-copy-sources); no Tower settings are changed.

| Field | Logic 1 / Variable #1 | Logic 2 / Variable #1 |
|---|---|---|
| Trigger | On Matching Event | On Matching Event |
| Source | Use Variable #1's (C) Events | Use Variable #1's (C) Events |
| set true | Copy Tower Line 2/Bt occupied producer event | Copy Tower Line 1/B2 occupied producer event |
| set false | Copy Tower Line 2/Bt clear producer event | Copy Tower Line 1/B2 clear producer event |
| Track Speed | Leave default; event source does not use it | Leave default; event source does not use it |

Select **Variable #2** under both Logics: **Trigger = None**, other fields unchanged.

### Select the Action

| Logic | Action selection | Condition | Action Event: copy from |
|---|---|---|---|
| 1 | Action 1 | Immediate if True | Board B / Mast 1 / Rule 1 / set aspect |
| 1 | Actions 2-4 | none | Leave defaults |
| 2 | Action 1 | Immediate if True | Board B / Mast 1 / Rule 2 / set aspect |
| 2 | Action 2 | Immediate if False | Board B / Mast 1 / Rule 3 / set aspect |
| 2 | Actions 3-4 | none | Leave defaults |

Use **board B's** generated rule commands, not board A's rule events. Initialize Bt/B2 by making each send both states once, finishing clear.

## 3. On board A, change Circuit 1's binding

| Selection | Field | Value |
|---|---|---|
| Track Circuits / Circuit 1 | Remote Mast Description | `S2r incoming speed` |
| Track Circuits / Circuit 1 | Link Address | Copy **board B / Rule to Aspect / Mast 1 / Link Address** |
| Conditionals / Logic 1: old S3r priority rung | Function | Blocked |
| Conditionals / Logic 2: old S3r final rung | Function | Blocked |
| Rule to Aspect / Mast 2: S3r | Mast Processing | Unused |

Leave board A **Mast 1/S4r = Normal**, **Logic 7 = Last (Single)** and **Logic 8 = Blocked**. Keep all receiver values listed above, especially `On Variable Change`. Do not change board A Logic 3-6 or its B3 indicator: they are not the local S3r publisher group. Circuit 1 stays on board A; it does not move to the publisher's board.

Board B Logic 1/2 are the new publisher group and remain enabled. The same slot numbers on board A identify the old publisher group and are blocked; do not confuse the two boards.

Only the publisher's **Link Address** goes into Circuit 1. Do not copy `set aspect` or `aspect is set` there.

Copy the publisher's **fresh hardware readback**, then apply and read back the receiver binding. The confirmed binding in this run was `02.01.57.10.09.96.07.30`. A receiver-only correction restored reception; changing the trigger is not a substitute for matching the actual speed-event family.

## 4. Repeat the four detector transitions

Begin with Bt/B2 known clear. First observe S2r; if its rule is wrong, stop and diagnose its input group before interpreting the remote probe.

| Step | Detector action | Expected board B S2r | Required board A S4r | Actual / monitor event |
|---|---|---|---|---|
| 1 | Set Bt occupied; B2 stays clear | Stop, red | Approach, red+green | S2r Red, S4r Geren |
| 2 | Set B2 occupied while Bt is occupied, then set Bt clear | Approach, red+green | Clear, green | S2r Red, S4r Geren |
| 3 | Set B2 clear; Bt stays clear | Clear, green | Clear, green | Green, Green |
| 4 | Set Bt occupied again; B2 stays clear | Stop, red | Approach, red+green | S2r Red, S4r Green |

The last column above preserves the original **before-correction** observations; it is not the final passing result. The locomotive's B1 -> B2 -> Bt -> B3 -> B4 journey does not reproduce step 2: B2 clears before Bt clears. Separate B2-only observations were inconsistent (yellow reported earlier, red later), so cross-board Approach acceptance is still open.

### Confirmed after correcting the receiver binding

| Time | Input / publisher evidence | Receiver evidence / observed appearance |
|---|---|---|
| 12:31:46.678 onwards | Bt clear; S2r Clear command `02.01.57.10.09.96.04.76`, acknowledgement `...04.77`, speed event `...07.37` | S4r Clear command `02.01.57.10.09.97.07.3E`, acknowledgement `...07.3F`; green |
| 12:31:47.542 onwards | Bt occupied; S2r Stop command `02.01.57.10.09.96.04.70`, acknowledgement `...04.71`, speed event `...07.30` | S4r Approach command `02.01.57.10.09.97.07.3B`, acknowledgement `...07.3C`; yellow-looking red+green |

The earlier 11:41 capture contains S2r Stop/Clear publications but no S4r mast commands; board A's events there belong to the separate B3 indicator. The corrected capture establishes reception, not a long steady-state or restart result.

Record detector events, S2r's rule/speed publications and S4r's action/aspect events in Bowties' Traffic Monitor. Compare with the same-board result; do not alter the working comparison to hide a cross-board failure.

Step 3 does not require a new visible S4r appearance because both non-Stop speeds require Clear. An unchanged green lamp alone does not prove fresh reception.

When inputs are steady, the repeated Approach-command flood must not return. Correct aspects without a quiet steady-state bus are not a passing result.

## 5. One receiver-restart observation

1. After step 4, leave **Bt occupied, B2 clear**: S2r Stop and S4r Approach.
2. Restart **board A only** using your usual method. Keep board B and Tower-LCC running, and do not change either detector yet.
3. Record S4r's state and initialization/publication evidence **before any new detector event**. Distinguish a retained/default appearance from evidence that current speed was acquired.
4. Set Bt clear, then occupied again while B2 stays clear. S2r must change Clear->Stop; S4r must change Clear->Approach.

Report whether initialization acquired current state automatically, remained unknown/retained, or needed a new source transition. No firmware expiry or forced startup Stop is assumed.

## Results

| Item | Result |
|---|---|
| Board A/B firmware/CDI revisions | Saved snapshots use rev-C7c CDI; exact firmware revisions not independently recorded |
| Board B publisher rules/speeds respond correctly to Bt/B2 | Stop/Clear confirmed; B2-only Approach retest outstanding |
| Board A Logic 7 retains Stop comparison, On Variable Change, true=Approach / false=Clear | Saved baseline has these values; final trigger used for successful capture not confirmed after the temporary On Matching Event experiment |
| Four cross-board transitions | Stop/Clear subset passed after binding correction; complete four-step sequence outstanding |
| No continuous command stream with steady inputs | No flood in the short corrected capture; sustained steady-state check outstanding |
| Receiver state/evidence before a new event after restart | Not recorded |
| Recovery after Bt clear/occupied | Normal transitions confirmed; post-restart recovery not recorded |
| Monitor evidence / saved configurations | Linked before/after traces above; refresh and save the layout from hardware to replace stale `...04.68` publisher/receiver values |

All copied rule commands and the publisher Link Address must be non-zero generated values. If a required field is zero, report it; do not invent an event ID.

This validates the same comparison across boards. S4r still has no B3 occupied->Stop priority rung, so this is not complete prototype-style occupancy protection.

## Later acceptance, not additional pre-design experiments

The [implementation plan](plan.md#physical-and-usability-acceptance) still covers full detector-driven Line policy, all occupancy combinations, three-signal chains, swapped hosts/A-B-A, both diagram orientations, and unavailable observations.

Application tests remain **A1** draft/apply/readback/reopen, **A2** inter-node/mid-node interruption and recovery, **A3** rebind/removal with shared detectors preserved, and **A4** invalid/stale proposals. They are not required to finish this small hardware-discovery test.
