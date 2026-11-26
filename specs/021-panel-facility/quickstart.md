# Quickstart: Panel Facility acceptance

You need:

- a layout with detected blocks, at least three of them in one Automatic Block Signals facility;
- a Signal LCC with free Direct Lamp Control rows and LEDs on #3, #6, #9, #12 and #14;
- a way to change block occupancy, such as detector inputs or the detector board's test buttons.

## 1. Build and wire (US1)

1. On the Railroad tab, choose Add facility, then Panel, and name it "Eagle Creek fascia". The editor opens on Blocks with an empty list.
2. Add all blocks from the signals facility. They appear in travel order. Add one more block by searching for it, then move it up.
3. Go to Next, then Wiring. Pick the first block, choose the Signal LCC, and click #9. Wire the second block to #3 and the third to #14. Leave the fourth unwired.
4. Click #14 again on the third block. It goes back to needing a lamp. Click #14 again to wire it.
5. Select the fourth block and click #9. The pin moves to the fourth block, a message says the first block now needs a lamp, and the first block's status changes to match. Rewire the first block to #9 and leave the fourth unwired.
6. Press Done, then Save while connected. Change each block's occupancy. Each LED follows its own block.
7. Set the second block's "Lit when" to Clear, then press Done and Save. That LED is lit when its block is clear.

## 2. Railroad card (US2)

There is one card. It is Incomplete, with "1 needs a lamp", and lists the entries in panel order with their pins. Lamp states change as occupancy changes. A lamp shows unknown until its event is seen.

## 3. Edit and remove (US3)

1. Edit the panel. Move the third block from #14 to #12, remove the second block, then Done and Save. Rows #14 and #3 are reset, #12 follows the block, and the signals facility still works.
2. Remove the panel. The confirmation names the pins on each board. Confirm and Save. Only those rows are reset, and the blocks remain.

## 4. Migration (US4)

Open a layout saved by the previous Bowties release that has Block Indicators on two boards, one with no lamp and one with no block. Check that:

- you see a notice of the form "N Block Indicators became M panels";
- there are two board panels and one "Indicators without a lamp" panel;
- the lamp names match the old facility names;
- the layout is marked changed;
- there are no pending board changes;
- the lamps still follow occupancy;
- Add facility doesn't offer Block Indicator.
