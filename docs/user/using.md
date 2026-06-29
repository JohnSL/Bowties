# Using Bowties

This guide walks you through opening a layout, connecting Bowties to your LCC bus, exploring your network, viewing and editing node configuration, and working offline.

## Opening or creating a layout

When Bowties starts, it opens to the layout picker. Choose one of these options before doing anything else:

- Open a known layout from the list.
- Click **New Layout** to create a new layout folder.
- Click **Browse...** to open an existing layout folder from disk.

Once a layout is open, Bowties restores the saved node snapshots, pending offline changes, and any layout-specific connections.

> **Tip:** A layout is a self-contained folder. You can share it with others, back it up, or move it to another computer by copying the entire folder and its contents.

## Connecting to your layout

Bowties keeps a list of named connections in the **Connect to LCC Network** card for the currently open layout. This card is opened from the connection status button in the top-right corner. When you have not connected yet, that button shows **Offline**.

Click the top-right **Offline** (or connection status) button to open the connection card. Each saved entry has a **Connect** button, plus an edit (🖊) and remove (×) action. The **`+`** button in the card header opens the Add connection dialog.

The status indicator in the connection bar turns green once a connection is established.

### Adding a connection

1. Open or create a layout.
2. Click the top-right **Offline** (or connection status) button to open the **Connect to LCC Network** card.
3. Click **`+`** in the card.
4. Enter a **Name** for the connection (for example *Layout hub* or *Workbench SPROG*).
5. Pick your hardware from the **Device** dropdown. Bowties auto-fills baud rate and flow control for known devices and shows only the fields that device needs.
6. Fill in the device-specific fields (see table below).
7. Click **Add**.

![Add connection dialog with the RR-CirKits LCC Buffer-USB device selected](../images/add-connection-dialog.png)

### Device options

| Device | Use for | Fields you enter |
|--------|---------|------------------|
| Network hub (TCP) | JMRI, WifiTrax, or a standalone TCP/IP bridge | Host + Port (JMRI uses `localhost:12021`) |
| RR-CirKits LCC Buffer-USB | RR-CirKits LCC Buffer-USB, also the LCC to Loconet Bridge | COM port |
| SPROG USB-LCC | SPROG DCC Ltd USB-LCC CAN adapter | COM port |
| SPROG PI-LCC | SPROG DCC Ltd Raspberry Pi LCC hat | COM port |
| Canable / Lawicell CANUSB | SLCAN-compatible USB-CAN adapter | COM port |
| Other GridConnect adapter | CAN2USBINO, MERG CAN-RS, or any other GridConnect device | COM port + Baud rate + Flow control |
| Other SLCAN adapter | Any `slcand`-compatible adapter not listed above | COM port + Baud rate + Flow control |

For serial devices, choose the correct COM port (Windows), `/dev/cu.*` device (macOS), or `/dev/tty*` device (Linux) from the dropdown. If you plugged the adapter in after opening the dialog, click the **⟳** button next to the COM port list to rescan.

### Connecting, editing, and removing

- Click **Connect** next to a saved entry to bring the link up.
- Click 🖊 to reopen the dialog pre-filled with that entry's settings.
- Click × and then **Delete** to remove a saved connection.

Saved connections persist with the layout.

---

## Discovering nodes

When you connect to the bus, Bowties discovers nodes automatically. The **Node List** populates with all nodes on your layout, showing manufacturer, model, and online status. Discovery is usually very fast.

## Reading node configuration

Before you can view configuration or the Bowties event map, click **Read Node Configuration** in the toolbar. Progress bars show how each node is coming along — on a large layout this can take a while.

![Node list after discovery, showing the Read Node Configuration button](../images/nodes-list.png)

Once complete, you can click into any node to view or edit its configuration, or switch to the Bowties view.

> **Note:** Configuration is cached after the first read, so subsequent launches are much faster.

## Making and saving changes

After reading node configuration, **Save** and **Discard** buttons appear in the toolbar.

**While connected to the bus:**
- Edit configuration fields in the cards and click **Apply** to write directly to the node hardware.
- When all pending edits are applied, click **Save** in the toolbar to persist node snapshots and bowtie metadata to the layout.

**While disconnected (offline mode):**
- Edit configuration fields and click **Apply**. These edits are staged locally as pending changes.
- Click **Save** to write your pending changes to the layout.
- Click **Discard** to abandon all pending edits and restore the last saved layout state.

Your layout always reflects the state you last saved, so you can safely close and reopen Bowties without losing work.

---

## Viewing node configuration

1. Click any node row in the Node List to select it.
2. The **Configuration View** opens. The left sidebar lists the node's CDI segments. Click a segment to select it.
3. The main area shows the segment's groups as cards. Each card displays the fields and sub-groups for that configuration group.
4. Field values are read from the node and shown in-place inside each card.

![Configuration view showing discovered nodes and a node being edited](../images/config-view-edit.png)

### Connector daughterboards on supported carrier boards

Some RR-CirKits carrier boards include one or more connector slots for optional daughterboards.

1. Open the supported node's configuration view.
2. Choose the installed daughterboard for each connector slot from the selector above the affected configuration.
3. Bowties immediately narrows the visible sections and valid options for the governed lines.
4. If a new daughterboard makes a current value invalid, Bowties stages the compatible repair before you apply changes.
5. Save the layout or project to keep those connector selections with that node instance.

Nodes without connector-slot metadata keep the normal pre-feature configuration workflow.

---

## Editing configuration

1. Navigate to the field you want to change in the Configuration View.
2. Edit the value inside the card:
   - **Drop-down** fields: choose from the list of options.
   - **Text / number** fields: click and type the new value.
   - **Event ID** fields: enter a 64-bit event identifier in `XX.XX.XX.XX.XX.XX.XX.XX` format.
3. Click **Apply** to write the new value to the node.

The field indicator changes to ✓ when the write is confirmed by the node.

---

## Bowties view (event relationship map)

The **Bowties View** shows a visual map of event producer/consumer relationships across your entire layout.

![Bowties view showing two buttons wired to a turnout's direction and indicator LED](../images/bowties-view.png)

- Each **bowtie** shape represents an event shared between one or more producers and one or more consumers.
- **Half-bowties** represent events that have producers but no consumers yet, or vice versa.
- **Hovering** a bowtie shows a summary tooltip (node name, segment, element).
- **Clicking** a bowtie jumps to that element in the Configuration View.

Use the filter bar at the top to show:

| Filter | Description |
|--------|-------------|
| Connected Only | Bowties with both a producer and consumer |
| Unconnected | Half-bowties only |
| All | Everything |

### Creating a new connection

To link a producer to a consumer:

1. Click **+ New Connection** in the Bowties view toolbar.
2. In the **New Connection** dialog, use the **Producer** panel (left) to pick the element that sends the event.
3. Use the **Consumer** panel (right) to pick the element that should respond to it.
4. Optionally enter a name for the connection.
5. Click **Create Connection**. Bowties resolves the event ID (preferring an already-configured side) and writes it to the other side.

You can also start a connection from the Configuration View: click **→ New Connection** next to any event ID field in a card. The dialog opens with that element pre-filled on the appropriate side.

![New Connection dialog with a planning bowtie being created](../images/planning-bowtie.png)

---

## Facilities (preview)

The **Railroad** tab includes a **Facilities** section for grouping the channels that make up a higher-level layout feature — for example, a *Block Indicator* that ties a block-occupancy input to an indicator-lamp output.

> **Preview feature.** Facilities are an early, evolving area of Bowties. The shape of facilities — what templates exist, how their slots are filled, and what they do once complete — is expected to change in upcoming releases. Feedback on this area is especially welcome.

### What you can do today

- **Add a facility** from the Railroad tab. Pick the **Block Indicator** template and give the facility a name (e.g., *Block 5*). The facility appears with status **Incomplete** and one empty slot per role declared by the template.
- **Rename** a facility at any time.
- **Delete** a facility you no longer want.

Facilities and their names are saved with the layout. Closing and reopening Bowties restores them exactly as you left them.

### What is not yet wired

Empty slots are placeholders only in this release. The workflows for binding a channel to a slot, creating a new channel from a slot, and watching a Block Indicator follow a real block on the bus are still under construction and will land in subsequent releases.

---

## Reconnecting with pending offline changes

If you saved a layout with pending offline changes and later reconnect with that layout, Bowties opens the **Sync Offline Changes** flow so you can review and apply them safely.

---

## Syncing offline changes back to the bus

When a saved layout with pending offline changes is open and you reconnect to the bus, Bowties compares the planned values in the layout with the current live values.

- **Conflicts** require you to choose whether to apply the offline value or skip it.
- **Clean changes** are ready to apply and are selected by default.
- **Already applied** changes are cleared automatically and reported as a count.
- **Missing nodes** stay pending in the layout until those nodes are present again.

If Bowties is not confident the connected bus matches the saved layout, it asks whether this is the **Target layout bus** or a **Bench / other bus** before showing apply choices.

---

## Advanced Configuration

Bowties reads an optional `tuning.toml` file from the application data directory at startup. This file lets you adjust datagram timing parameters without rebuilding the application — useful when troubleshooting CAN gateway connectivity issues.

**File location:**

| Platform | Path |
|----------|------|
| Windows  | `%APPDATA%\com.lcc.bowties\tuning.toml` |
| macOS    | `~/Library/Application Support/com.lcc.bowties/tuning.toml` |
| Linux    | `~/.config/com.lcc.bowties/tuning.toml` |

**Example `tuning.toml`:**

```toml
# Delay (ms) after acknowledging a reply datagram before sending the next
# request. Increase if reads through a CAN gateway time out intermittently.
post_ack_delay_ms = 10

# Per-attempt timeout (ms) waiting for a memory-config read reply.
read_timeout_ms = 3000

# Maximum retries when a node rejects with the resend-OK flag.
max_datagram_retries = 3
```

All fields are optional — omitted fields use the built-in defaults shown above. Changes take effect the next time you launch the application.

---

## Troubleshooting

**No nodes appear after connecting**
- Check that the connection status is green (top-right button shows a colored status, not "Offline").
- Ensure your LCC network has power and that at least one node is online.
- If you still see no nodes after a few seconds, try disconnecting and reconnecting — some nodes respond slowly on first boot.

**A USB adapter is not listed**
- Check Device Manager (Windows), System Information → USB (macOS), or `dmesg` (Linux) to confirm the adapter is recognized as a serial port.
- Click the **⟳** button next to the COM port dropdown to rescan after plugging the adapter in.

**Configuration changes are not accepted**
- Some nodes require a reboot before new configuration takes effect. Check the node's manual.
- Ensure you are connected when clicking Apply — a dropped connection will prevent writes.
