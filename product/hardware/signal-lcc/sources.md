# Sources

Every external source cited in this folder, with URLs. Community messages are on the **LayoutCommandControl** Groups.io list: <https://groups.io/g/layoutcommandcontrol>

Reading archived messages is public. Downloading files from the **Files** area requires a free Groups.io login.

## Primary (In-Repo) Sources

| Source | Path | Notes |
|---|---|---|
| Signal-LCC CDI, rev C7c | `docs/ref/RR-CirKits_Inc__Signal-LCC_rev-C7c.cdi.xml` | Strongest evidence: segment/field/enum definitions as published by the node |
| Signal-LCC manual | `docs/ref/SignalLCC-manual-e.pdf` | Sections 7 (Logic, pp. 25–31) and 8 (Masts, pp. 31–33). Marked DRAFT beyond early sections |
| Manual-derived field prose | `profile-extractions/signal-lcc/field-descriptions.yaml` | Per-field descriptions with manual section/page citations |
| Manual-derived section prose | `profile-extractions/signal-lcc/section-descriptions.yaml` | Per-segment/group descriptions |
| Structure profile | `profile-extractions/signal-lcc/RR-CirKits_Inc._Signal-LCC.profile.yaml` | Event roles and relevance rules |

## Vendor Documentation

| Source | URL |
|---|---|
| RR-CirKits clinics index (Dick Bronson) | <https://rr-cirkits.com/Clinics/Clinics.html> |
| Signaling with LCC, Salt Lake City 2019 — part A | <https://rr-cirkits.com/Clinics/Salt%20Lake%20City%202019/NMRA-2019-Signaling%20with%20LCC-A.pdf> |
| Signaling with LCC, Salt Lake City 2019 — part B | <https://rr-cirkits.com/Clinics/Salt%20Lake%20City%202019/NMRA-2019-Signaling%20with%20LCC-B.pdf> |
| Signaling with LCC, MER 2021 — part A | <https://rr-cirkits.com/Clinics/MER%202021/MER-2021-Signaling%20with%20LCC-A.pdf> |
| Signaling with LCC, MER 2021 — part B | <https://rr-cirkits.com/Clinics/MER%202021/MER-2021-Signaling%20with%20LCC-B.pdf> |
| Signal LCC-32H SSSB physical wiring (Peter Ely, 2024) | <https://rr-cirkits.com/Clinics/PMRRM%20LCC%20Signal-32%202024-06-19.pdf> |
| Manuals index | <https://rr-cirkits.com/manuals/> |
| Firmware updates index | <https://rr-cirkits.com/firmware/> |

## Community Reference Documents

Maintained by Mark Granville in **Files → LCC Topics**: <https://groups.io/g/layoutcommandcontrol/files/LCC%20Topics>

| Document | Attached to |
|---|---|
| LCC Topics Understanding SignalLCC Conditionals.pdf | [#14034](https://groups.io/g/layoutcommandcontrol/message/14034), [#8704](https://groups.io/g/layoutcommandcontrol/message/8704) |
| LCC Topics Understanding SignalLCC & Track Circuits.pdf | [#8704](https://groups.io/g/layoutcommandcontrol/message/8704) |
| LCC Topics Understanding JMRI Tables & CDI Search | referenced in [#7872](https://groups.io/g/layoutcommandcontrol/message/7872) |

## Video

| Source | URL |
|---|---|
| The LCC Channel (Detlef Kurpanek) — signal configuration in episodes 11–16 | <https://www.youtube.com/@the-lcc-channel/videos> |

## Books

- Dana Zimmerli, *Layout Command Control: Principles* (2025 edition) — repeatedly recommended in the group as the user-level companion to the vendor manuals.

## Messages By Topic

### Rule to Aspect, masts and bring-up

| # | Author | Subject |
|---|---|---|
| [15213](https://groups.io/g/layoutcommandcontrol/message/15213) | Allan Gartner | Signal LCC checkout — terminal numbering, how to test |
| [15214](https://groups.io/g/layoutcommandcontrol/message/15214) | Bob Jacobsen | `Set Aspect` → `More…` → `Trigger` test procedure |
| [15216](https://groups.io/g/layoutcommandcontrol/message/15216) | Dick Bronson | Lamp labels are positional; clockwise terminals; CA/CC jumper; 5 masts + spare |
| [13986](https://groups.io/g/layoutcommandcontrol/message/13986) | Ken Cameron | Mast turns all lamps off then sets pattern; direct lamp makes you manage both edges |
| [14006](https://groups.io/g/layoutcommandcontrol/message/14006) | Robert Heller | Multi-head mast convention at a turnout |
| [9878](https://groups.io/g/layoutcommandcontrol/message/9878) | Ken Cameron | Prove the electrical part before Rule to Aspect |
| [10449](https://groups.io/g/layoutcommandcontrol/message/10449) / [10450](https://groups.io/g/layoutcommandcontrol/message/10450) | Daniel / Ken Cameron | `aspect is set` / `aspect cleared` producers cannot be disabled |
| [10527](https://groups.io/g/layoutcommandcontrol/message/10527), [10533](https://groups.io/g/layoutcommandcontrol/message/10533), [10560](https://groups.io/g/layoutcommandcontrol/message/10560) | Daniel, John Joyce | `31-Dark` as an aspect; dark signals still publish a track speed |
| [13212](https://groups.io/g/layoutcommandcontrol/message/13212) | Clifford Anderson | "Link to Previous" and head Position semantics |
| [Tutorial thread](https://groups.io/g/layoutcommandcontrol/topic/tutorial_on_setting_up_a/29696659) | Paul Davidson / Dick Bronson | Make masts work before logic; JMRI all-zero event ID bug; paste event IDs |

### Conditionals and logic

| # | Author | Subject |
|---|---|---|
| [14034](https://groups.io/g/layoutcommandcontrol/message/14034) | Mark Granville | Conditionals explained; links the reference PDF |
| [14035](https://groups.io/g/layoutcommandcontrol/message/14035) | Dick Bronson | Variables store the state the logic needs; LCC uses reliable transport |
| [14036](https://groups.io/g/layoutcommandcontrol/message/14036) | Detlef Kurpanek | Node keeps a local map of event states |
| [15220](https://groups.io/g/layoutcommandcontrol/message/15220) | Tom Patterson | Held-signal problem statement |
| [15221](https://groups.io/g/layoutcommandcontrol/message/15221) | Dick Bronson | Needs a flip-flop on direction enable |
| [15223](https://groups.io/g/layoutcommandcontrol/message/15223) | Ken Cameron | Held/release event pair, evaluated first |
| [15224](https://groups.io/g/layoutcommandcontrol/message/15224) | Tom Patterson | **"The conditionals for the signal look for a true, and once found, exit."** |
| [15225](https://groups.io/g/layoutcommandcontrol/message/15225) | Dick Bronson | Direction as an independent variable; fleeting; capability is not the limit |
| [7873](https://groups.io/g/layoutcommandcontrol/message/7873) | Dick Bronson | Never look further than the next signal |
| [7879](https://groups.io/g/layoutcommandcontrol/message/7879) | Ken Cameron | Clear as fall-through, saving a slot |
| [7880](https://groups.io/g/layoutcommandcontrol/message/7880) | Dick Bronson | `V1 AND Then V2` exists for direction detection |
| [7876](https://groups.io/g/layoutcommandcontrol/message/7876), [7877](https://groups.io/g/layoutcommandcontrol/message/7877) | Donavan Pantke | APB, tumble-down, direction circuits |
| [8704](https://groups.io/g/layoutcommandcontrol/message/8704) | Mark Granville | Circuits plug into conditionals, not masts; don't reuse one source for both variables |
| [7955](https://groups.io/g/layoutcommandcontrol/message/7955) | Bob Gamble | JMRI CDI cannot copy or reorder logic entries; 32H power-cycle; lamp order |

### Track circuits

| # | Author | Subject |
|---|---|---|
| [8694](https://groups.io/g/layoutcommandcontrol/message/8694) | Stephen Scharfstein | Track circuit setup confusion, full worked question |
| [8699](https://groups.io/g/layoutcommandcontrol/message/8699) | Rob Heikens | Step-by-step cascade; most-restrictive-first; set Action Condition |
| [8700](https://groups.io/g/layoutcommandcontrol/message/8700) | Ken Cameron | Check Track Speed first; expected event trace; mast pushes, doesn't consume |
| [8702](https://groups.io/g/layoutcommandcontrol/message/8702) | Stephen Scharfstein | Chained advance aspects worked example |
| [8703](https://groups.io/g/layoutcommandcontrol/message/8703) | Rob Heikens | Each block needs its own circuit and its own conditional group |
| [8977](https://groups.io/g/layoutcommandcontrol/message/8977) | Ken Cameron | Copy Link Address into the Circuit |
| [9295](https://groups.io/g/layoutcommandcontrol/message/9295) | Mark Granville | A circuit is only valid in the node where defined |
| [9297](https://groups.io/g/layoutcommandcontrol/message/9297) | Mark Granville | Only masts facing the same direction are "next" |
| [9316](https://groups.io/g/layoutcommandcontrol/message/9316) | Ken Cameron | CW/CCW chains; what a signal's logic must include |
| [9319](https://groups.io/g/layoutcommandcontrol/message/9319) | Mark Granville | One circuit per "next" mast; circuit number unrelated to mast number |
| [9321](https://groups.io/g/layoutcommandcontrol/message/9321) | Ken Cameron | Circuit needs at diverging turnouts |
| [9324](https://groups.io/g/layoutcommandcontrol/message/9324) | David Harris | Producer/consumer framing; quotes manual §8.3 |

### Direct lamp control and indicators

| # | Author | Subject |
|---|---|---|
| [13985](https://groups.io/g/layoutcommandcontrol/message/13985) | Mike Chapman | "All I want to do is light lamps from turnout position" |
| [13991](https://groups.io/g/layoutcommandcontrol/message/13991) | Balazs Racz | Signalling is genuinely a specialist domain; where to find resources |
| [13992](https://groups.io/g/layoutcommandcontrol/message/13992) | Robert Heller | Signal type and rulebook variation |
| [13997](https://groups.io/g/layoutcommandcontrol/message/13997) | David Harris | Concrete first-aspect walkthrough |
| [14009](https://groups.io/g/layoutcommandcontrol/message/14009) | Mike Chapman | Position-indicating LEDs were the easy case |
| [Atlas signal thread](https://groups.io/g/layoutcommandcontrol/topic/signal_lcc_board_and_atlas/98184931) | Tom Cherepko, David Harris, Drew H | Direct Lamp Control test procedure; **return to `Used by Mast`**; CA/CC jumper location |

### I/O, firmware and hardware

| # | Author | Subject |
|---|---|---|
| [7523](https://groups.io/g/layoutcommandcontrol/message/7523) | Dick Bronson | Diagnostic order: LCC Monitor → consumer LED → function **and** action |
| [7531](https://groups.io/g/layoutcommandcontrol/message/7531) | Dick Bronson | Buttons are producers, lamps are consumers; line vs message terminology |
| [7542](https://groups.io/g/layoutcommandcontrol/message/7542) | Rob Heikens | Rev-G I/O lines 1–2 output bug, fixed in C7a |
| [7547](https://groups.io/g/layoutcommandcontrol/message/7547) | Rob Heikens | C7a compatibility and version-numbering rationale |
| [966](https://groups.io/g/layoutcommandcontrol/message/966) | Dick Bronson | LED current draw; external lamp power header and voltage requirement |
| [15073](https://groups.io/g/layoutcommandcontrol/message/15073) | Dick Bronson | Power wiring practice: bus to nearby terminals, ferruled tails |
| [7863](https://groups.io/g/layoutcommandcontrol/message/7863), [7864](https://groups.io/g/layoutcommandcontrol/message/7864), [7867](https://groups.io/g/layoutcommandcontrol/message/7867) | Dick Bronson | **32H only:** SSSB is one-way, gender matters, boost is 5 V without data, ~30 ft between drivers |
| [32H head config thread](https://groups.io/g/layoutcommandcontrol/topic/signal_lcc_32h_only_the_first/99817405) | Dick Bronson | **32H only:** full head configuration walkthrough; Enabled vs Linked to Previous; linked heads share the first head's rules |

### Bowties

| # | Author | Subject |
|---|---|---|
| [15406](https://groups.io/g/layoutcommandcontrol/message/15406) | John Socha-Leialoha | Bowties block-detection setup with no manual event ID handling |
