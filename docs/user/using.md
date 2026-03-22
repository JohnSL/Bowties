# Using Bowties

This guide walks you through connecting Bowties to your LCC layout, exploring your network, and viewing and editing node configuration.

## Connecting to your layout

### Via a TCP hub (JMRI or standalone bridge)

1. Launch Bowties.
2. Click **Add connection** in the top bar.
3. Select **TCP**.
4. Enter the host and port — for JMRI use `localhost:12021`.
5. Click **Connect**.

The status indicator in the connection bar turns green when the link is established.

### Via a USB-to-CAN adapter (GridConnect serial)

> Supported adapters: SPROG CANISB, SPROG USB-LCC, RR-Cirkits Buffer LCC, CAN2USBINO

1. Plug in your adapter; let Windows/Linux install the USB serial driver.
2. Click **Add connection**.
3. Select **GridConnect (USB/Serial)**.
4. Choose the correct COM port (Windows) or `/dev/ttyUSB*` device (Linux) from the dropdown.
5. Click **Connect**.

### Via a USB-to-CAN adapter (SLCAN)

> Supported adapters: Canable, Lawicel CANUSB, other `slcand`-compatible adapters

Same steps as GridConnect serial, but choose **SLCAN (USB/Serial)** in step 3.

---

## Discovering nodes

After connecting, click **Discover Nodes** in the toolbar. Bowties sends a broadcast and collects responses from every node on the network.

The **Node List** appears showing:

| Column | Description |
|--------|-------------|
| Name | User-assigned name (if set) or node ID |
| Manufacturer | From the node's SNIP data |
| Model | Hardware model string |
| Version | Software version |
| Status | Online / Offline |

Discovery takes about one second on a typical layout. You can run it again at any time to refresh the list.

---

## Viewing node configuration

1. Click any node row in the Node List to select it.
2. The **Configuration View** opens. The left sidebar lists the node's CDI segments. Click a segment to select it.
3. The main area shows the segment's groups as cards. Each card displays the fields and sub-groups for that configuration group.
4. Field values are read from the node and shown in-place inside each card.

> **Note:** The first time you open a node, Bowties fetches and caches its CDI from the hardware. Subsequent opens load from the local cache and are instant.

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

- Each **bowtie** shape represents a matched producer ↔ consumer pair.
- **Half-bowties** represent events that have a producer but no consumer yet, or vice versa.
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

---

## Traffic monitor

Click **Monitor** in the toolbar to open the real-time traffic monitor. All LCC frames received on the connection are displayed with timestamps, MTI labels, and decoded payloads. This is useful for verifying that a node is responding and for watching event traffic when you press buttons or trigger sensors.

---

## Saving your work

Configuration is written directly to node hardware each time you click Apply — there is no separate save step. The CDI cache is stored on disk automatically and persists between sessions.

Connection settings (host, port, adapter) are saved automatically and restored the next time you launch Bowties.

---

## Troubleshooting

**No nodes appear after discovery**
- Check that the connection status is green.
- Ensure your LCC network has power and that at least one node is online.
- Try running **Discover Nodes** again — some nodes respond slowly on first boot.

**A USB adapter is not listed**
- Check Device Manager (Windows) or `dmesg` (Linux) to confirm the adapter is recognized as a serial port.
- Try unplugging and re-plugging the adapter, then reopen the connection dialog.

**Configuration changes are not accepted**
- Some nodes require a reboot before new configuration takes effect. Check the node's manual.
- Ensure you are connected when clicking Apply — a dropped connection will prevent writes.
