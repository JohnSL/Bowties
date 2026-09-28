# Signal-LCC Clean-Room Test Procedure

Use this procedure after factory-resetting the Signal-LCC node. Its purpose is to reproduce the two-input conditional test without restoring any earlier mast, rule, or conditional configuration.

The decisive question is whether a factory-clean Signal-LCC can run this priority ladder:

```text
B3 occupied                  -> Stop
B3 clear and Bt occupied     -> Approach
B3 clear and Bt clear        -> Clear
```

Do not add another mast, a Track Circuit, or unrelated conditionals until this ladder passes.

## Test Hardware And Events

Make all Signal-LCC changes on node **02.01.57.10.09.97**, formerly named **Blocks & Signals 1**. Do not change the Tower-LCC configuration; it remains the producer of the detector events.

| Item | Value |
|---|---|
| S4r green lamp | Output **#9 H3-G** |
| S4r red lamp | Output **#10 H3-Y** (the CDI label says Y; the connected lamp is red) |
| B3 indicator | Output **#12 H3-L** |
| B3 occupied | `02.01.57.40.02.D7.04.36` |
| B3 clear | `02.01.57.40.02.D7.04.37` |
| Bt occupied | `02.01.57.40.02.D7.04.12` |
| Bt clear | `02.01.57.40.02.D7.04.13` |

The event IDs below were used before the reset. A factory reset may regenerate or clear event fields. **Read every field back before testing.** If the node now shows different non-zero `set aspect` event IDs, record and use those values consistently instead of the old values shown here.

| S4r command | Pre-reset event ID |
|---|---|
| Stop | `02.01.57.10.09.97.07.38` |
| Approach | `02.01.57.10.09.97.07.3B` |
| Clear | `02.01.57.10.09.97.07.3E` |

## Evidence To Capture

For every phase:

1. Save or export the node configuration.
2. Record the node firmware and CDI revision.
3. Capture the complete LCC monitor trace from immediately before the input event through the resulting aspect events.
4. Record the visible lamps, not only the event trace.
5. Change only the fields named by the phase.

Use fresh configuration slots after the reset: **Mast 1**, **Logic 1**, **Logic 2**, and an unused Direct Lamp Control row. Do not recreate the previous S2/S3 configuration.

## Phase 0 — Verify The Reset Baseline

1. Reconnect to node `02.01.57.10.09.97` in JMRI/LccPro.
2. Confirm that Mast 1 is `Unused`, Logic 1 and Logic 2 are `Blocked`, and no Direct Lamp Control row owns outputs #9, #10, or #12.
3. Record whether Mast 1's event fields contain non-zero generated values or all zeros.
4. Restart the node once and reconnect.
5. Send each detector event manually and confirm that it appears in the LCC monitor:
   - B3 occupied and clear.
   - Bt occupied and clear.
6. Confirm that none of those events produces an S4r aspect command yet.

Stop if detector events do not appear. That is an input/transport problem, not a conditional problem.

## Phase 1 — Prove The Physical Outputs

Before assigning outputs to a mast, use Direct Lamp Control temporarily to identify them.

1. Select an unused Direct Lamp Control row.
2. Assign it to output **#9 H3-G**, trigger its Lamp On and Lamp Off events, and confirm that S4r green responds.
3. Repeat for output **#10 H3-Y**, confirming that the physical S4r red lamp responds.
4. Optionally repeat for output **#12 H3-L**, confirming the B3 indicator.
5. Return each tested row's `Lamp Selection` to **Used by Mast** before continuing.

Stop if the physical lamps do not match the output map.

## Phase 2 — Build And Prove S4r Mast 1

Keep **Mast Processing = Unused** while entering the rules.

### Mast 1

| Field | Value |
|---|---|
| Mast Description | `S4r clean-room test` |
| Mast Processing | `Unused` while editing |
| Lamp Fade | None |
| Link Address | Keep the node-generated non-zero value; record it |

### Rule 1 — Stop

| Field | Value |
|---|---|
| Rule Name | `0-Stop` |
| Track Speed | Stop |
| set aspect | Use the node-generated value, or `02.01.57.10.09.97.07.38` if the field must be restored |
| Appearance Lamp 1 | **#10 H3-Y**, Steady |
| Appearance Lamps 2–4 | Unused |
| Appearance Effects | None |
| Effects Lamp | Unused |

### Rule 2 — Approach

This test assumes that energizing the red and green lamps together produces the yellow appearance.

| Field | Value |
|---|---|
| Rule Name | `21-Approach` |
| Track Speed | Approach |
| set aspect | Use the node-generated value, or `02.01.57.10.09.97.07.3B` if the field must be restored |
| Appearance Lamp 1 | **#9 H3-G**, Steady |
| Appearance Lamp 2 | **#10 H3-Y**, Steady |
| Appearance Lamps 3–4 | Unused |
| Appearance Effects | None |
| Effects Lamp | Unused |

### Rule 3 — Clear

| Field | Value |
|---|---|
| Rule Name | `29-Clear` |
| Track Speed | Clear/Proceed |
| set aspect | Use the node-generated value, or `02.01.57.10.09.97.07.3E` if the field must be restored |
| Appearance Lamp 1 | **#9 H3-G**, Steady |
| Appearance Lamps 2–4 | Unused |
| Appearance Effects | None |
| Effects Lamp | Unused |

Leave Rules 4–8 unused. Do not leave any enabled mast rule with an all-zero event ID.

### Direct aspect test

1. Set **Mast Processing = Normal**.
2. In JMRI/LccPro, use the `set aspect` field's **More… → Trigger** action for each rule.
3. Verify:

| Trigger | Expected appearance |
|---|---|
| Stop | #10 on, #9 off |
| Approach | #9 and #10 on |
| Clear | #9 on, #10 off |

For each transition, verify that the previous appearance is extinguished automatically and that the trace contains the rule's `aspect is set` event. Save the configuration and trace.

Stop if any direct aspect test fails. Conditionals cannot repair a mast or event-ID problem.

## Phase 3 — Prove One Stand-Alone Conditional

This phase proves B3 and Logic 1 without grouping.

Keep Logic 1 `Blocked` while entering its fields.

### Logic 1

| Field | Value |
|---|---|
| Description | `S4r - B3 occupied Stop else Clear` |
| Function | `Blocked` while editing; then `Last (Single)` |
| Logic Operation | V1 Only |
| Variable 1 Trigger | **On Matching Event** |
| Variable 1 Source | Use Variable #1's (C) Events |
| Variable 1 set true | B3 occupied: `02.01.57.40.02.D7.04.36` |
| Variable 1 set false | B3 clear: `02.01.57.40.02.D7.04.37` |
| Variable 2 Trigger | None |
| Action when true | Send then Exit Group |
| Action when false | Send then Exit Group |
| Delay | 0 milliseconds |
| Action 1 | Immediate if True → S4r Stop `set aspect` event |
| Action 2 | Immediate if False → S4r Clear `set aspect` event |
| Actions 3–4 | none |

After writing the fields:

1. Set Logic 1 to `Last (Single)`.
2. Restart the node and reconnect.
3. Publish B3's current state once.
4. Send B3 occupied, then B3 clear.
5. Confirm Stop, then Clear, including the expected action and aspect events in the trace.

Stop if this phase fails. Save the configuration before proceeding.

## Phase 4 — Run The Decisive Two-Entry Group Test

Keep the proven B3 fields in Logic 1, but convert it into the first rung. Configure Logic 2 as the final rung.

### Logic 1 — B3 priority rung

| Field | Value |
|---|---|
| Function | `Blocked` while editing; later `Group` |
| Action when true | Send then Exit Group |
| Action when false | **Send then Evaluate Next** |
| Action 1 | Immediate if True → S4r Stop `set aspect` event |
| Action 2 | **Immediate if False → a fresh, otherwise-unused diagnostic event ID** |
| Actions 3–4 | none |

The false diagnostic event proves that evaluation passed from Logic 1 to Logic 2. Record the chosen event ID in the results table below.

### Logic 2 — Bt final rung

| Field | Value |
|---|---|
| Description | `S4r - Bt occupied Approach else Clear` |
| Function | `Blocked` while editing; later `Last (Single)` |
| Logic Operation | V1 Only |
| Variable 1 Trigger | **On Matching Event** |
| Variable 1 Source | Use Variable #1's (C) Events |
| Variable 1 set true | Bt occupied: `02.01.57.40.02.D7.04.12` |
| Variable 1 set false | Bt clear: `02.01.57.40.02.D7.04.13` |
| Variable 2 Trigger | None |
| Action when true | Send then Exit Group |
| Action when false | Send then Exit Group |
| Delay | 0 milliseconds |
| Action 1 | Immediate if True → S4r Approach `set aspect` event |
| Action 2 | Immediate if False → S4r Clear `set aspect` event |
| Actions 3–4 | none |

### Activate And Initialize

1. Set Logic 2 to `Last (Single)`.
2. Set Logic 1 to `Group`.
3. Keep Mast 1 `Normal`.
4. Complete the node update/reset operation if prompted.
5. Restart the node deliberately and reconnect.
6. Publish the **current state of both B3 and Bt**. The node must know both stored values before transition testing begins.

### Transition Matrix

Run the tests in this order and preserve one continuous trace:

| Step | B3 | Bt | Expected S4r | Expected branch evidence |
|---|---|---|---|---|
| 1 | Clear | Clear | Clear | Logic 1 false diagnostic, then Clear command |
| 2 | Clear | Occupied | Approach | Logic 1 false diagnostic, then Approach command |
| 3 | Occupied | Occupied | Stop | Stop command; no Approach or Clear command |
| 4 | Occupied | Clear | Stop | Stop command; no Approach or Clear command |
| 5 | Clear | Clear | Clear | Logic 1 false diagnostic, then Clear command |
| 6 | Clear | Occupied | Approach | Logic 1 false diagnostic, then Approach command |

While B3 is occupied, repeat Bt clear/occupied several times. S4r must remain Stop. Because triggers use `On Matching Event`, repeating an unchanged current-state event should still force a visible reevaluation.

### Interpret The Result

- **All six steps pass:** the two-entry group works on a factory-clean node. Compare every active conditional's Function value and group boundary before attributing an earlier failure to stored state or update sequencing.
- **Both stand-alone tests pass but the group emits nothing:** the factory-clean node reproduces the group activation failure. Preserve the configuration export and complete trace before changing anything.
- **Logic 1 false diagnostic appears but no Logic 2 command appears:** traversal reaches the second rung; investigate Logic 2's function, event fields, and action settings.
- **Stop is sent when B3 is occupied but a later Approach/Clear follows:** priority short-circuiting is not behaving as configured; preserve the exact trace.
- **Only one input direction works:** verify both true and false event IDs and publish both current states after restart.

Do not try reserved Function values. The C7c value displayed as `Last (Single)` by JMRI and Bowties is the correct value.

`Send then Evaluate Next` was used on Logic 1's false branch only to emit the diagnostic event before continuing. A bench differential test changed that field to `Evaluate Next` and repeated the matrix successfully; behavior remained the same except that the diagnostic event disappeared. Do not use `Send then Exit Group` there; it prevents Logic 2 from running whenever B3 is clear.

Additional differential tests changed Logic 1 and then Logic 2 from `On Matching Event` to `On Variable Change`. The complete matrix continued to pass after each change. Both trigger modes are therefore valid for real occupied/clear transitions in this experiment; `On Matching Event` remains useful when deliberately repeating an unchanged state event to force reevaluation.

When comparing or restoring an older export, verify every active entry's `Function`, not only the new group's first rung. The saved configuration initially had `S4r - B3 occupied - Stop` set to `Blocked`; correcting that to `Group` was necessary but insufficient. JMRI Logic 2, 4 and 6 also contained reserved raw Function value `3` where valid `Last (Single)` terminators were required. The later S4r group began working only after all three earlier values were changed to `Last (Single)`.

For the C7c CDI, treat any Function value outside `Blocked`, `Group` and `Last (Single)` as a configuration error. Also reject or warn about a run of `Group` entries that does not end at a valid `Last (Single)` before the conditional table ends. An invalid earlier boundary can silence a later valid group, so checking only the group under test is insufficient.

## Phase 5 — Reintroduce The Independent B3 Indicator

Only after Phase 4 passes, configure an unused Direct Lamp Control row:

| Field | Value |
|---|---|
| Lamp Description | `B3` |
| Lamp On | B3 occupied: `02.01.57.40.02.D7.04.36` |
| Lamp Off | B3 clear: `02.01.57.40.02.D7.04.37` |
| Lamp Fade | None |
| Lamp Selection | **#12 H3-L** |
| Lamp Phase / Flash Rate | Steady |

Repeat the matrix. When B3 is occupied, both the B3 indicator and S4r Stop lamp must remain illuminated. This is the regression test for the original signal-versus-indicator failure. The indicator is intentionally outside the conditional group so short-circuiting cannot suppress it.

## Phase 6 — Add Realistic Cascade Later

Do not start this phase until the direct two-detector group is proven.

1. Configure a second mast protecting Bt.
2. Verify that mast independently by directly triggering every aspect.
3. Verify that every rule has the correct `Track Speed`.
4. Configure a Track Circuit and observe its speed events.
5. Replace Logic 2's direct Bt occupancy input with the downstream restriction reported through the Track Circuit.
6. Repeat the transition matrix.

This isolates Track Circuit behavior from the conditional-engine test instead of introducing both at once.

## Results Record

| Item | Recorded value |
|---|---|
| Test date | |
| Firmware / CDI revision | |
| Node name after reset | |
| Mast 1 link address | |
| Stop `set aspect` | |
| Approach `set aspect` | |
| Clear `set aspect` | |
| Logic 1 false diagnostic event | |
| Phase 1 outputs | Pass / Fail |
| Phase 2 direct mast | Pass / Fail |
| Phase 3 stand-alone Logic 1 | Pass / Fail |
| Phase 4 grouped Logic 1–2 | Pass / Fail |
| Phase 5 independent indicator | Pass / Fail |
| Configuration export path | |
| Trace path | |
| Notes | |

## Safe Stop

To make the experiment inert without erasing the evidence:

1. Set Mast 1 to `Unused`.
2. Set Logic 1 and Logic 2 to `Blocked`.
3. Set the B3 Direct Lamp Control row to `Used by Mast` if output #12 must be released.
4. Export the configuration before making any unrelated changes.
