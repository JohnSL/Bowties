# Quickstart: Enhanced Node Discovery with SNIP Data

**Feature**: 001-node-snip-data  
**Target Users**: Developers implementing the feature, QA testing, and future contributors  
**Prerequisites**: Bowties application built and running, LCC network accessible

## Overview

This guide demonstrates how to use the Enhanced Node Discovery with SNIP Data feature to discover LCC nodes, retrieve their identification information, and understand their status.

## Table of Contents

1. [Basic Node Discovery](#basic-node-discovery)
2. [Viewing SNIP Data](#viewing-snip-data)
3. [Manual Refresh](#manual-refresh)
4. [Working with Node Status](#working-with-node-status)
5. [Automatic Discovery](#automatic-discovery)
6. [Troubleshooting](#troubleshooting)

---

## Basic Node Discovery

### Initial Discovery on Application Launch

When Bowties connects to an LCC network, it automatically discovers all nodes:

1. **Launch Bowties**
2. **Connect to LCC network** (via Settings or connection dialog)
   - Default: `localhost:12021` (JMRI or simulator)
   - Or specify IP:port of your LCC TCP hub
3. **Wait for discovery** (typically 250ms - 3 seconds)

**Expected Result**: Node list populates with discovered nodes showing:
- Temporary placeholder names (e.g., "Node 05.02.01.02.00.03")
- Gray "Unknown" status indicators
- "Loading..." indicators for SNIP data

### What Happens Behind the Scenes

```
Application Startup
       ↓
Connect to TCP LCC Hub (port 12021)
       ↓
Send global "Verify Node ID" message
       ↓
Collect "Verified Node ID" responses (250ms timeout)
       ↓
Queue SNIP requests (max 5 concurrent)
       ↓
Display nodes as SNIP data arrives
```

---

## Viewing SNIP Data

### Understanding Node Display Names

Nodes are displayed with friendly names based on SNIP data priority:

#### Priority 1: User-Assigned Name
If the node owner configured a custom name:
```
Display: "East Panel Controller"
Tooltip: "RR-CirKits Tower-LCC v2.3.1 | Node ID: 05.02.01.02.00.03"
```

#### Priority 2: Manufacturer + Model
If no user name, but manufacturer data available:
```
Display: "RR-CirKits Tower-LCC"
Tooltip: "Software v2.3.1 | Node ID: 05.02.01.02.00.03 | Alias: 3AE"
```

#### Priority 3: Node ID Only
For nodes without SNIP support:
```
Display: "Node 05.02.01.02.00.03"
Tooltip: "Alias: 3AE | SNIP not supported"
```

### Viewing Full SNIP Details

**To see complete SNIP information**:
1. Click on a node in the list
2. View the detail panel (or right-click → "Properties")

**Detail View Shows**:
- **Manufacturer**: e.g., "RR-CirKits"
- **Model**: e.g., "Tower-LCC"
- **Hardware Version**: e.g., "1.0"
- **Software Version**: e.g., "2.3.1"
- **User Name**: e.g., "East Panel Controller"
- **User Description**: e.g., "Controls east hallway section"
- **Node ID**: 05.02.01.02.00.03
- **Alias**: 3AE (hex)
- **Last Verified**: "2 minutes ago"

---

## Manual Refresh

### When to Refresh

Refresh the node list when:
- Nodes physically added/removed from the network
- Node power cycled or restarted
- Checking if an unresponsive node is back online
- Verifying recent configuration changes

### How to Refresh

**Method 1: Refresh Button**
1. Click **"Refresh"** or **"Rescan Network"** button in the toolbar
2. Wait for completion (typically 3-5 seconds for 20 nodes)
3. Status indicators and SNIP data update

**Method 2: Keyboard Shortcut**
- Press **F5** or **Ctrl+R** (Windows/Linux) / **Cmd+R** (macOS)

**What Happens During Refresh**:
1. Existing node list grayed out with "Refreshing..." overlay
2. Global Verify Node ID sent
3. All responding nodes collected
4. SNIP data re-queried (5 concurrent max)
5. Status indicators updated
6. Node list refreshed with new data

### Expected Timing

| Network Size | Expected Refresh Time | Notes |
|--------------|----------------------|-------|
| 1-5 nodes | <2 seconds | Very fast |
| 6-20 nodes | 3-5 seconds | Target performance |
| 21-50 nodes | 5-10 seconds | May vary based on network |

---

## Working with Node Status

### Understanding Status Indicators

Each node shows a colored status indicator:

| Indicator | Meaning | When Shown |
|-----------|---------|------------|
| 🟢 Green dot | **Connected** | Node responded to recent verification |
| 🔴 Red dot | **Not Responding** | Node failed to respond within timeout |
| ⚫ Gray dot | **Unknown** | Status not yet verified |
| ⏳ Pulsing gray | **Verifying** | Verification in progress |

### Verification Timestamps

Below each node: **"Last verified: X ago"**

Examples:
- "Last verified: Just now" (< 1 minute)
- "Last verified: 5 min ago"
- "Last verified: 2 hr ago"
- "Last verified: Never" (not yet verified)

### SNIP Status Badges

Additional badges show SNIP data status:

| Badge | Meaning |
|-------|---------|
| ✓ **Complete** | All SNIP fields retrieved successfully |
| ⚠️ **Partial** | Some SNIP data received, but incomplete |
| ⏱️ **Timeout** | SNIP request timed out (5 seconds) |
| ⊘ **Not Supported** | Node doesn't implement SNIP protocol |

### Verifying a Specific Node

**To check if a specific node is online**:
1. Right-click the node → "Verify Status"
2. Or click the node and press **"Verify"** button
3. Watch for status indicator to update (2 second timeout)

**Successful verification**:
```
Status changes: Unknown → Verifying → Connected (green)
Last verified: "Just now"
Response time displayed: "Response: 45ms"
```

**Failed verification**:
```
Status changes: Unknown → Verifying → Not Responding (red)
Last verified: "Just now"
Warning: "Node did not respond to verification"
```

---

## Automatic Discovery

### Background Listening

Bowties continuously listens for new nodes joining the network in the background.

**When a new node joins**:
1. Node broadcasts "Verified Node ID" message (part of LCC initialization)
2. Bowties detects the broadcast
3. Node appears in list within 10 seconds
4. SNIP data automatically queried
5. User sees notification: "New node discovered: [name]"

### Example Scenario

```
1. Application running with 3 nodes visible
2. User powers on new LCC board
3. New board completes initialization (~5 seconds)
4. Bowties detects broadcast
5. New node appears in list: "Node 05.02.01.02.00.05"
6. SNIP query sent automatically
7. Display updates to "RR-CirKits Tower-LCC" (few seconds later)
```

### Disabling Auto-Discovery

If network has frequent resets or temporary nodes:
1. Go to **Settings** → **Discovery**
2. Uncheck **"Auto-discover new nodes"**
3. Use manual **Refresh** to update node list

---

## Troubleshooting

### Problem: No Nodes Discovered

**Symptoms**: Node list remains empty after connection

**Causes & Solutions**:

1. **Not connected to LCC network**
   - Check connection status indicator (top-right)
   - Verify TCP hub IP:port is correct
   - Test with: `telnet <ip> <port>` to verify hub reachable

2. **No nodes on network**
   - If using simulator, ensure nodes are started
   - If real hardware, verify physical connections and power

3. **Firewall blocking connection**
   - Check firewall allows outbound connections on port 12021
   - Try connecting to `localhost:12021` if running JMRI locally

### Problem: SNIP Data Shows "Timeout"

**Symptoms**: Node appears in list but SNIP status is "Timeout"

**Causes & Solutions**:

1. **Node is slow or busy**
   - Wait a few seconds for SNIP indicator to update
   - Try manual refresh
   - Some nodes take 3-5 seconds to respond

2. **Node doesn't support SNIP**
   - Older LCC devices may not implement SNIP
   - Check node documentation
   - Node still usable, just shows Node ID instead of friendly name

3. **Network congestion**
   - Reduce number of simultaneous devices
   - Wait for current operations to complete
   - Refresh one node at a time (right-click → Verify)

### Problem: Node Shows "Partial" Data

**Symptoms**: Some SNIP fields empty, status shows "Partial"

**Causes & Solutions**:

1. **Incomplete SNIP implementation**
   - Some nodes only provide manufacturer/model, not user fields
   - This is normal for certain devices
   - **Action**: Use what's available; no fix needed

2. **Network interruption during retrieval**
   - **Action**: Click "Retry" or manually refresh node
   - If problem persists, check network reliability

### Problem: Duplicate Node Names

**Symptoms**: Multiple nodes show same display name

**Expected Behavior**: Bowties automatically disambiguates:
```
East Panel (05.02.01...)
East Panel (05.02.02...)
East Panel (05.02.03...)
```

**If disambiguation not working**:
1. Check application version (bug in older versions)
2. Manually assign unique names on each device
3. Report bug with node details

### Problem: Node Status Stuck on "Verifying"

**Symptoms**: Gray pulsing indicator doesn't resolve

**Solutions**:
1. Wait up to 5 seconds (SNIP timeout)
2. If still stuck after 10 seconds, refresh the node list
3. If problem persists:
   - Disconnect and reconnect to LCC network
   - Check application logs for errors
   - Report bug with console output

### Problem: New Node Not Auto-Detected

**Symptoms**: Manually added node to network but not appearing automatically

**Solutions**:

1. **Wait up to 10 seconds** - Detection has built-in delay
2. **Check node initialization**:
   - Ensure new node completed startup sequence
   - Some nodes take 10-15 seconds to initialize
3. **Manual refresh**: Click "Refresh" button to force discovery
4. **Verify auto-discovery enabled**: Check Settings → Discovery
5. **Check node broadcasts**: Node must send Verified Node ID on startup

---

## Developer Testing Checklist

When testing this feature implementation:

### Discovery Tests
- [ ] Connect to network with 0 nodes → empty list
- [ ] Connect to network with 1 node → node appears
- [ ] Connect to network with 20 nodes → all appear within 5 seconds
- [ ] Discovery completes within timeout (250ms for verification)

### SNIP Tests
- [ ] Node with complete SNIP → all 6 fields populated
- [ ] Node with user name → display shows user name
- [ ] Node without user name → display shows manufacturer + model
- [ ] Node without SNIP → shows Node ID, "SNIP not supported"
- [ ] Slow node (simulated delay) → shows "Loading..." then data
- [ ] Timeout node (no response) → shows "Timeout" status after 5s

### Status Tests
- [ ] Verify node status → green indicator + timestamp
- [ ] Disconnect node → status changes to "Not Responding"
- [ ] Reconnect node → auto-detected and status updates
- [ ] Last verified timestamp updates correctly

### UI/UX Tests
- [ ] Duplicate names disambiguated with Node ID suffix
- [ ] Tooltip shows complete node information
- [ ] Status indicators have correct colors
- [ ] Refresh button works and completes in <5 seconds
- [ ] Node detail panel shows all SNIP fields

### Edge Case Tests
- [ ] Malformed SNIP data (invalid UTF-8) → sanitized display
- [ ] Very long user name/description → truncated with ellipsis
- [ ] Network with 50 nodes → no UI lag
- [ ] Network reset during discovery → handled gracefully

---

## Next Steps

- **For Developers**: See `tasks.md` (generated by `/speckit.tasks`) for implementation breakdown
- **For Testing**: Run through all scenarios in user stories (see `spec.md`)
- **For Integration**: Use Tauri commands defined in `contracts/tauri-commands.json`

## Related Documentation

- **Feature Specification**: `spec.md` - User scenarios and requirements
- **Data Model**: `data-model.md` - Entity structures and validation
- **Research**: `research.md` - Protocol details and design decisions
- **API Contracts**: `contracts/tauri-commands.json` - Tauri command signatures
