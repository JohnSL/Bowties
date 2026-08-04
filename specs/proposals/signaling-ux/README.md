# Signaling UX — Proposal Set

Status: **in progress.** The mockups exist; the proposal prose does not yet.

This directory captures where we want to take the Bowties UX for **signalling** — building signals
that a user describes in railroad terms, with Bowties owning every event ID, CDI field and logic rung.

It is deliberately *not* a Signal-LCC proposal or a Tower LCC+Q proposal. Hardware is an
implementation detail Bowties chooses on the user's behalf.

## Contents

| File | Status | Purpose |
|---|---|---|
| [signaling-ux-mockups.html](signaling-ux-mockups.html) | **done, iterating** | Self-contained HTML mockups. Open in a browser. No build step; safe to email to reviewers. |
| `signaling-ux-proposal.md` | **not written** | The prose proposal. Deliberately deferred until the mockups settle — see "Working method". |
| This README | living | Handoff context, decisions, open questions. |

Follows the convention set by [../app-ux-vision/](../app-ux-vision/README.md): proposal markdown plus
one self-contained mockups HTML, using Fluent v9 tokens matching
[../../018-block-indicator-facility/mockups.html](../../018-block-indicator-facility/mockups.html).

## Why this exists

Spec [020-abs-signaling](../../020-abs-signaling/) was built on an assumption that turned out to be
wrong: that a facility could be **per signal head**. Real signalling requires building a **mast** —
on a multi-head mast the heads are read *together* as one indication, not one head per route. That
invalidated the slice plans in `slices-rule-to-aspect.md` and `slices-lamp-selection.md`.

Separately, people the author consulted were **sceptical that a guided approach could scale** to the
diversity of real signalling. ABS is too simple to answer that, so the mockups lead with hard cases.

020 will be revised to align with this proposal once it settles.

## Working method

**Mockups before prose.** Drawing screens changes the model; writing prose first means defending it
instead of learning from it. Write `signaling-ux-proposal.md` only after the mockups stop moving.

**Reviewer audience is both** prototype-signalling experts and Signal-LCC/LCC hardware experts. The
mockup's review brief splits its questions accordingly.

**Signalling data in the mockups is illustrative** and marked as such, so reviewers critique the
*model* separately from the *data*.

---

## Domain facts established (do not re-derive)

Primary source is the node knowledge base at
[product/hardware/signal-lcc/](../../../product/hardware/signal-lcc/README.md) — read
`rule-to-aspect.md`, `conditionals.md`, `track-circuits.md` and `bowties-ux-implications.md` before
touching this proposal.

### Signal-LCC — `Rule to Aspect`

```
Mast (×8)
├── Mast Processing   Unused | Normal | Linked to Previous
├── Mast Description  32 chars
├── Link Address      (P) publishes this mast's current Track Speed
├── Lamp Fade         None | Incandescent
└── Rule (×8)
    ├── Rule Name     fixed 32-name enum, 0-Stop … 31-Dark
    ├── Track Speed   8 values: Stop, Restricting/Tumble Down, Slow, Medium,
    │                 Limited, Approach, Approach-Medium, Clear/Proceed
    ├── set aspect    (C)  ← what logic fires
    ├── aspect is set (P)
    ├── aspect cleared(P)
    ├── Appearance (×4)   Lamp Selection (#1 H1-G … #16 H4-L) + Lamp Phase/Flash
    ├── Appearance Effects
    └── Effects Lamp
```

Load-bearing facts:

- **`Rule Name` is a label only.** The node derives no behaviour from it. Behaviour comes from
  `Appearance` (which lamps) and `Track Speed` (what cascades). **This is why no rulebook is baked
  in** and why a PRR position light is the same shape as a NORAC colour light.
- **`Linked to Previous` gives a mast more than 8 aspects (16). It is NOT how you add a head.**
  Extra heads are simply more lamps in the same rule's 4-slot `Appearance` list.
- **Heads are a Bowties fiction.** The node has 16 flat lamp drivers and no concept of a head.
- **`31-Dark` is a real aspect** occupying a rule slot and publishing a speed — often `Clear`, so an
  approach-lit signal doesn't hold down the signal behind it. Never treat dark as "off".
- **Lamp order is green top, yellow middle, red bottom**, matching the driver naming
  `H1-G`, `H1-Y`, `H1-R`, `H1-L`.
- **Cross-segment invariant nobody enforces:** `Lamp Selection = 0` means *Unused* in a mast but
  *Used by Mast* in Direct Lamp Control. Bowties must own driver ownership.
- The `Rules` segment (ABS/APB/CTC presets) exists in the CDI but is **not implemented in firmware**.

### Signal-LCC ceilings

8 masts · 8 aspects per mast (16 chained) · **4 lit lamps per aspect** · 16 lamp drivers ·
32 conditionals · 8 track circuits (Rx only).

**Lamp drivers bind before mast slots do** — five 3-lamp signals exhaust the drivers with 3 mast
slots idle.

### Tower LCC (plain, rev C6)

No `Rule to Aspect`, no `Direct Lamp Control`, no `Brightness`. Conditionals + Track Receiver +
Track Transmitter only.

### Tower LCC+Q (rev-A, v1.15)

CDI cached at `%APPDATA%\com.lcc.bowties\cdi_cache\RR-CirKits__Inc__Tower-LCC_Q_v1_15.cdi.xml`.

| Segment | Shape |
|---|---|
| Port I/O | 16 lines |
| Logic Inputs | 16 × 8 = **128 input bits**, each a True/False consumer pair |
| Logic Outputs | 16 × 8 = **128 output bits**, each a True/False producer pair |
| Conditionals | 16 blocks, each a `MultiLine` **256-byte string** = STL source |
| Track Receivers | 16 |
| Track Transmitters | 16 |
| Syntax Messages | `Build Successful` / `Syntax Error(s)` events + 3 × 64-char messages |

- **No lamp drivers, no masts, no aspects.** So **a mast can only ever live on a Signal-LCC**, and
  complex-case signals are necessarily dual-node: display on Signal-LCC, logic on Tower LCC+Q.
- It **compiles STL on board and reports build results over the bus** — Bowties would be a *code
  generator* here, not a field-writer.
- 128 output bits ≈ 16 masts × 8 aspects.
- 16 Track Transmitters means it can *originate* speeds — useful for APB tumbledown.

### APB / tumbledown

- `Track Speed` value 1 is literally **`Restricting/Tumble Down`**, on both mast and conditional
  sides. The transport already exists and is cross-node.
- **Direction detection has a sanctioned primitive:** the `V1 AND Then V2 => true` temporal AND,
  applied to two adjacent blocks. Dick Bronson,
  [#7880](https://groups.io/g/layoutcommandcontrol/message/7880).
- **Direction must be latched, not computed.** Bronson,
  [#15225](https://groups.io/g/layoutcommandcontrol/message/15225): it is "an independent variable…
  not something that can be calculated from other variables". Every latch needs an explicit clearer
  or you get "fleeting".
- Direction belongs to a **section**, not a signal — eight signals share it.
- Reference on APB schemes: `lundsten.dk/us_signaling/abs_apb/`, via Donavan Pantke
  [#7876](https://groups.io/g/layoutcommandcontrol/message/7876) /
  [#7877](https://groups.io/g/layoutcommandcontrol/message/7877).
- **Not yet extracted:** the manual's own worked ABS *and APB* examples, Signal-LCC manual section 9,
  pages 35–47. `profile-extractions/signal-lcc/recipes.yaml` skipped them. **Highest-value gap.**

### Prototype signalling

- NORAC is **speed signalling**. A multi-head mast is one composite indication encoding *speed now* +
  *speed at the next signal*, not one head per route.
- NORAC 10th ed. rule text extracts fine; the **aspect drawings are images** (PDF pages 83, 85, 87,
  89, 91, 93, 95, 97, 99, 101). Rendered PNGs are in `temp/` (gitignored). A machine-readable aspect
  table was **never completed** — and is now judged *nice-to-have*, since `Rule Name` is only a label.

---

## Decisions made

1. **Facility = mast, not head.** Forced by composite indications and by one Link Address per mast.
2. **Three-part aspect model:** name (cosmetic) + up to 4 lit lamps + a speed that cascades.
3. **Signal type is a first-class, reusable, shareable object** — heads, lamps per head, and the
   indication table. Shipped catalog entries are ordinary signal types with no special status.
4. **Conditions are derived, not authored.** Each indication carries **Permits past** and **Expects
   ahead**; combined with the user's bindings this generates the condition table mechanically. An
   `Adjust…` per-indication escape hatch handles genuine exceptions. **This is the load-bearing
   claim of the whole proposal.**
5. **The signalling system (ABS / APB / Interlocking / CTC) is asked first**, and determines what the
   user is asked afterwards *and which hardware Bowties allocates*.
6. **ABS + interlocking run entirely on the Signal-LCC** (single node). **APB + CTC put logic on a
   Tower LCC+Q as generated STL.** Masts stay on the Signal-LCC either way.
7. **Nothing is excluded on grounds of complexity.** APB and CTC are *deferred*, not out of scope.
8. **Never show an event ID.**
9. **Bowties owns all 16 lamp drivers**, refuses double-claims, and restores ownership after tests.
10. **The guided flow mirrors the community bring-up order:** wiring → one lamp → one aspect → whole
    mast → logic → cascade.

## Corrections made mid-session (don't regress)

- `Linked to Previous` is about **aspects, not heads**.
- Green renders **top**, not bottom — 7 mast faces were wrong.
- Rule to Aspect is on the **Signal-LCC**, not the Tower LCC.
- The scaling argument was initially framed around the *Signal-LCC's* envelope; that wrongly conceded
  that the proposal is board-scoped. It is not.
- NORAC aspect data was initially treated as blocking; it is not, because `Rule Name` is only a label.

## Open questions

| # | Question | Blocks |
|---|---|---|
| 1 | Is derived-conditions (decision 4) believable for real signals? | Everything |
| 2 | Is **signal type** first-class, or just a saved template? | Glossary, file format |
| 3 | Keep the **indication** vs **aspect** distinction in the UI, given the node blurs it? | Glossary, UI |
| 4 | Does naming "next signal ahead" by hand scale to 200 signals, or is a track plan eventually required? | §7 shape |
| 5 | Do the 16 Tower LCC+Q `MultiLine` blocks concatenate into one program or are they 16 independent ones? | STL capacity |
| 6 | Is filtering the indication list by head complement helpful or patronising? | §3 |
| 7 | If a user hand-writes STL on their Tower LCC+Q, does Bowties generate, read, or leave it alone? | Later |
| 8 | Does the ABS catalog entry ship one head, or one head + marker? | Catalog data |

Glossary gap: none of **signal type**, **head**, **indication**, **aspect**, **section** or
**signalling system** exists in [product/glossary.md](../../../product/glossary.md).

## Mockup structure

`signaling-ux-mockups.html` — reviewer brief, then a **storyboard** (7 frames: add facility →
signalling system → signal type → wire lamps → what it protects → confirm derived behaviour → test →
done), then 11 detail sections:

1. Two-head interlocking mast · 2. Lamp driver assignment · 3. Signal type editor with filtered
indications · 4. **PRR position light built from scratch** (proves no rulebook lock-in) · 5. Capacity
envelope · 6. Bring-up sequence · 7. Cascade · 8. Three-aspect ABS (shown last, as the degenerate
case) · 9. **Beyond ABS — APB / CTC / interlocking** · 10. What this cannot do · 11. Vocabulary.

The reviewer challenge is deliberately falsifiable: *"Describe a signalling arrangement you actually
need — any system, any rulebook, any mast — that this model cannot describe. Not 'would be
laborious'; cannot describe at all."*

## Housekeeping

- **The NORAC rulebook PDF is in tracked [docs/ref/](../../../docs/ref/).** It is copyrighted; it
  should probably be gitignored if the repo is public.
- `temp/` (gitignored) holds `norac.txt`, rendered aspect-page PNGs, and `extract.mjs` / `render.mjs`
  scaffolding. `render.mjs` fails with `@napi-rs/canvas` (Path2D incompatibility); `canvas`
  (node-canvas) was installed but its build scripts were blocked by npm's allow-scripts policy.

## Next steps

1. Review the mockups with signalling and hardware experts; collect answers to the open questions.
2. Extract the Signal-LCC manual's ABS/APB worked examples (section 9, pp. 35–47) into
   `product/hardware/signal-lcc/`.
3. Settle vocabulary; add the missing glossary entries.
4. Write `signaling-ux-proposal.md`.
5. Revise [specs/020-abs-signaling/](../../020-abs-signaling/) to match — the existing
   `slices-lamp-selection.md` and `slices-rule-to-aspect.md` assume per-head facilities and a fixed
   3-aspect template, and will need re-cutting.
