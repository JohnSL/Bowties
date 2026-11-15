# Single-Board Values: S4r Receiving S3r Track Speed

**Updated**: 2026-11-07
**Purpose**: Establish the Stop-versus-Approach/Clear comparison and same-board reception before finalizing the design.

**Recorded result, 2026-11-07**: The actual test used publisher Logic 1/2 instead of the template's 3/4, and receiver Logic 7 instead of 1, with Logic 8 blocked. The receiver's `On Matching Event` setting caused repeated Approach commands; changing only its trigger to `On Variable Change` stopped the flood. Visual checks and the 10:02:48-10:03:15 trace confirmed downstream Stop -> upstream Approach, and downstream Approach/Clear -> upstream Clear. The template below now uses the tested receiver trigger. The [two-board procedure](manual-two-board-test.md) preserves the actual working slots.

**Cross-board follow-up**: The [two-board record](manual-two-board-test.md) now confirms Stop/Clear reception after correcting the receiver to the publisher's actual hardware Link Address. Its initial saved publisher value did not match hardware readback. Always copy a freshly read generated mast address, not an edited/offline value; attempted publisher-address writes were not retained on the tested board. The complete cross-board Approach and restart checks remain open.

These are Bowties configuration values, organized by the **Mast**, **Rule**, **Appearance Lamp**, **Logic**, **Variable** and **Action** selections. No Bowties/reset tutorial or pre-known event IDs are required.

**Baseline**: Factory-reset Signal LCC **Blocks & Signals 1**, or equivalent unused test slots. Leave the working Tower-LCC and its detector configuration unchanged. Apply the listed values to hardware before measuring; unsaved/offline edits are not active configuration.

## Test arrangement

| Selection on Blocks & Signals 1 | Role |
|---|---|
| Rule to Aspect / Mast 1 | S4r, visible receiving probe |
| Rule to Aspect / Mast 2 | S3r, speed publisher |
| Conditionals / Logic 1 | Stand-alone Track Circuit probe |
| Conditionals / Logic 3 and Logic 4 | Known two-input group selecting S3r's rules |
| Track Circuits / Circuit 1 | Receives S3r's Link Address |

All other conditionals, including Logic 2, remain `Blocked`. Direct Lamp Control rows remain disabled with `Lamp Selection = Used by Mast`.

There is no direct event-send requirement. The existing **Bt** and **B2** detector buttons select S3r's three rules:

| Bt | B2 | S3r publishes |
|---|---|---|
| Occupied | Either | Stop |
| Clear | Occupied | Approach |
| Clear | Clear | Clear/Proceed |

Bt/B2 are test inputs, not a new final-Line topology. S4r's B3 occupancy is deliberately excluded so it cannot mask the receiving comparison. RG Approach means red+green; perceived yellow quality is not tested.

## 1. Rule to Aspect: select the Mast

Keep each mast `Unused` while entering its Rule/Appearance values; its final processing value is `Normal`.

| Mast selection | Mast Description | Lamp Fade | Link Address | Final Mast Processing |
|---|---|---|---|---|
| Mast 1 | `S4r TC probe` | None | Keep generated value | Normal |
| Mast 2 | `S3r TC publisher` | None | Keep generated value | Normal |

The field label is **Link Address**; its description is `(P) Track Circuit Link Address`. Do not replace it with a rule event.

## 2. Select the Mast, then the Rule

These are **Rule-level** values, not Appearance Lamp values. Keep each rule's generated `set aspect`, `aspect is set` and `aspect cleared` events unchanged.

| Mast | Rule selection | Rule Name | Track Speed |
|---|---|---|---|
| 1: S4r | Rule 1 | 0-Stop | Stop |
| 1: S4r | Rule 2 | 21-Approach | Approach |
| 1: S4r | Rule 3 | 29-Clear | Clear/Proceed |
| 2: S3r | Rule 1 | 0-Stop | Stop |
| 2: S3r | Rule 2 | 21-Approach | Approach |
| 2: S3r | Rule 3 | 29-Clear | Clear/Proceed |

Leave Rules 4-8 at reset defaults. S4r Rule 1 is available for later occupancy-priority work but is not selected by this probe.

## 3. Select the Mast, Rule, then Appearance Lamp

For each row below, select that **Mast -> Rule -> Appearance Lamp** before setting:

| Mast | Rule | Appearance selection | Lamp Selection | Lamp Phase (A-B) - Flash Rate | Appearance Effects | Effects Lamp |
|---|---|---|---|---|---|---|
| 1: S4r | 1: Stop | Lamp 1 | #10 H3-Y | Steady | None | Unused |
| 1: S4r | 2: Approach | Lamp 1 | #10 H3-Y | Steady | None | Unused |
| 1: S4r | 2: Approach | Lamp 2 | #9 H3-G | Steady | None | Unused |
| 1: S4r | 3: Clear | Lamp 1 | #9 H3-G | Steady | None | Unused |
| 2: S3r | 1: Stop | Lamp 1 | #15 H4-R | Steady | None | Unused |
| 2: S3r | 2: Approach | Lamp 1 | #15 H4-R | Steady | None | Unused |
| 2: S3r | 2: Approach | Lamp 2 | #16 H4-L | Steady | None | Unused |
| 2: S3r | 3: Clear | Lamp 1 | #16 H4-L | Steady | None | Unused |

For each configured rule, all other Appearance Lamps have **Lamp Selection = Unused**, **Appearance Effects = None**, **Effects Lamp = Unused**; leave their phase at its default. Thus Stop/Clear use Lamp 1 only; Approach uses Lamps 1 and 2.

S4r's physical red is wired to the output labeled `#10 H3-Y`.

## 4. Detector-event copy sources

Read the existing **Tower-LCC -> Port I/O -> Line** producer events. Do not edit them.

| Source name used below | Line selection | Producer Event selection | Field to copy |
|---|---|---|---|
| Bt occupied | Line 2: Bt | Event configured for Input On | Indicator / Indication (P) |
| Bt clear | Line 2: Bt | Event configured for Input Off | Indicator / Indication (P) |
| B2 occupied | Line 1: B2 | Event configured for Input On | Indicator / Indication (P) |
| B2 clear | Line 1: B2 | Event configured for Input Off | Indicator / Indication (P) |

The producer group may be labeled **Producer Events**, **Actions/Producers**, or the configured **Indication (P)** group, depending on Tower CDI revision. Select the producer entry by its existing `Input On`/`Input Off` setting, not an assumed Event slot number. In the existing BOD mapping these mean occupied/clear respectively.

Use those **producer Indicator/Indication** values, not the line's consumer `Command` events. This is copying existing working detector IDs; no numbers must be supplied in advance.

## 5. Publisher: select Logic 3, then Logic 4

Keep both `Function = Blocked` while entering the values; enable Logic 4 as `Last (Single)` before Logic 3 as `Group`.

### Logic-level fields

| Field | Logic 3 | Logic 4 |
|---|---|---|
| Description | `S3r Bt Stop priority` | `S3r B2 Approach else Clear` |
| Final Function | Group | Last (Single) |
| Logic Operation | V1 Only | V1 Only |
| Action when true | Send then Exit Group | Send then Exit Group |
| Action when false | Evaluate Next | Send then Exit Group |
| Time Delay | 0 milliseconds | 0 milliseconds |

Leave delay retrigger at its reset default; there is no delay.

### Select Variable #1 under each Logic

| Field | Logic 3 / Variable #1 | Logic 4 / Variable #1 |
|---|---|---|
| Trigger | On Matching Event | On Matching Event |
| Source | Use Variable #1's (C) Events | Use Variable #1's (C) Events |
| set true | Copy **Bt occupied** | Copy **B2 occupied** |
| set false | Copy **Bt clear** | Copy **B2 clear** |
| Track Speed | Leave default; event source does not use it | Leave default; event source does not use it |

Select **Variable #2** under each Logic: set **Trigger = None**; leave other fields at defaults. `V1 Only` excludes it.

### Select the Action under each Logic

| Logic | Action selection | Condition | Action Event: copy from |
|---|---|---|---|
| 3 | Action 1 | Immediate if True | This board / Mast 2 / Rule 1 / set aspect |
| 3 | Actions 2-4 | none | Leave defaults |
| 4 | Action 1 | Immediate if True | This board / Mast 2 / Rule 2 / set aspect |
| 4 | Action 2 | Immediate if False | This board / Mast 2 / Rule 3 / set aspect |
| 4 | Actions 3-4 | none | Leave defaults |

Logic 3's false branch evaluates Logic 4 without sending a diagnostic event. Use generated **set aspect** commands as the action destinations, not the `aspect is set` acknowledgements.

Before measurement, make each Bt/B2 detector send both states once, finishing with both clear. That initializes both stored variables; merely leaving an untouched button clear does not necessarily send its state.

## 6. Select Track Circuits / Circuit 1

| Field | Value |
|---|---|
| Remote Mast Description | `S3r incoming speed` |
| Link Address | Copy this board / Rule to Aspect / Mast 2 / Link Address |

The source is the mast's **Link Address**, not any individual rule event.

## 7. Probe: select Logic 1

Keep `Function = Blocked` while entering values; final value is `Last (Single)`.

### Logic-level fields

| Field | Value |
|---|---|
| Description | `S4r TC1 Stop probe` |
| Final Function | Last (Single) |
| Logic Operation | V1 Only |
| Action when true | Send then Exit Group |
| Action when false | Send then Exit Group |
| Time Delay | 0 milliseconds; leave retrigger at reset default |

### Select Variable #1

| Field | Value |
|---|---|
| Trigger | On Variable Change |
| Source | Track Circuit 1 |
| Track Speed | Stop |
| set true / set false | Leave defaults; not used with Track Circuit source |

Select **Variable #2**: **Trigger = None**, other fields unchanged.

### Select the Action

| Action selection | Condition | Action Event: copy from |
|---|---|---|
| Action 1 | Immediate if True | This board / Mast 1 / Rule 2 / set aspect |
| Action 2 | Immediate if False | This board / Mast 1 / Rule 3 / set aspect |
| Actions 3-4 | none | Leave defaults |

All copied commands and the publisher Link Address must be non-zero generated values. If a required value is zero after reset, stop and report that field; do not invent an ID or substitute an unrelated event.

## 8. Measure four transitions

With both detector inputs initialized clear, measure the following. Observe S3r first: if it does not select the expected rule, stop and diagnose the publisher setup rather than interpret S4r's result.

| Step | Detector action | Expected S3r | Required S4r | Actual / monitor event |
|---|---|---|---|---|
| 1 | Set Bt occupied; B2 stays clear | Stop, red | Approach, red+green | |
| 2 | Set B2 occupied while Bt is occupied, then set Bt clear | Approach, red+green | Clear, green | |
| 3 | Set B2 clear; Bt stays clear | Clear, green | Clear, green | |
| 4 | Set Bt occupied again; B2 stays clear | Stop, red | Approach, red+green | |

The intermediate B2 change in step 2 must leave S3r at Stop until Bt clears. No direct rule trigger is used. Record the detector events, S3r rule/speed publication and S4r action/aspect events in Bowties' Traffic Monitor.

Step 3 need not change S4r's rule: downstream Approach and Clear both require upstream Clear. Do not infer current reception solely from its unchanged green lamp.

If these rows pass, continue with the [two-board values](manual-two-board-test.md). No eight-speed survey or complete Line is needed before design.

## 9. One follow-up only if the Stop comparison fails

1. Select **Logic 1 -> Variable #1**: change only **Track Speed** to **Approach**.
2. Repeat step 8, keeping the same Action Event copies.
3. If S4r is **Clear for S3r Stop**, and **Approach for S3r Approach and Clear**, reverse the two output actions:
   - Select **Logic 1 -> Action 1**: copy `Mast 1 -> Rule 3 -> set aspect` into **Action Event**.
   - Select **Logic 1 -> Action 2**: copy `Mast 1 -> Rule 2 -> set aspect` into **Action Event**.
4. Repeat step 8; its required S4r results must now hold.
5. Otherwise return both observations and the trace without guessing additional settings. A constant probe could mean comparison, reception or triggering failed.

Keep the publisher's rule Track Speeds unchanged. The measured selector/action mapping is the design input.

## Results

| Item | Result |
|---|---|
| Signal-LCC firmware/CDI revision | |
| S3r publishes all three expected rules/speeds from Bt/B2 inputs | |
| Logic 1 selector and true/false action copies used | |
| Four-transition results and optional follow-up | |
| Monitor evidence / saved configuration | |

Retain this setup for the two-board experiment. Full B3 occupancy priority and full-Line acceptance come later.

**Evidence basis**: [validated conditional group](../../product/hardware/signal-lcc/conditionals.md), [mast/conditional field values](../../product/hardware/signal-lcc/clean-room-test-procedure.md), [Track Circuit address binding](../../product/hardware/signal-lcc/track-circuits.md), and the [BOD occupied/clear input mapping](../../app/src-tauri/profiles/RR-CirKits.shared-daughterboards.yaml).
