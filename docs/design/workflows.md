# User Workflows

> **Status: Active aspirational design input.** These workflows are aspirational user journeys, not current step-by-step behavior. Current implemented workflows are documented in `product/user-stories/`. Use this file to guide UX and feature direction, not as a reference for how the app works today.

*These workflows represent aspirational user journeys that demonstrate the intended user experience. They guide feature development and UX design decisions.*

## Workflow 1: Understanding Existing Configuration

**Scenario:** User opens layout configured by someone else, wants to understand what's set up.

**Steps:**

1. Launch Bowties → Auto-scans network (1 second)
2. Default view: Event Bowties → Connected Only
3. See 3 bowties showing existing connections
4. Hover bowtie → Tooltip shows summary
5. Click bowtie → Jump to Configuration view, drilled to producer element
6. Miller Columns show: Node → Segment → Group → Element → Configuration
7. See full details: Event IDs, parameters, user names
8. Click "View Bowties" button → Back to Bowties view, highlights that element's connections

**Outcome:** User understands how the layout is wired logically, without needing to trace physical wiring or understand Event IDs.

---

## Workflow 2: Configuring Element Before Physical Wiring

**Scenario:** Planning to add button to Line 3, want to configure before wiring.

**Steps:**

1. Configuration View → Miller Columns
2. Column 1 (Nodes): Select "East Panel"
3. Column 2 (Segments): Auto-loads CDI (from cache or fetch), select "Port I/O"
4. Column 3 (Groups): Select "Line"
5. Column 4 (Elements): Select "Line 3" (shows ○ Unconfigured)
6. Column 5 (Configuration): 
   - Function type: Select "Input - Button (momentary)"
   - Active state: Select "Low"
   - Debounce: 50ms (default)
   - Event On: Shows empty field (Event ID comes from node)
   - Event Off: Shows empty field
   - User Name: Enter "Front Door Button"
   - Location: Select "East Hallway" tag
7. Click [Apply] → Writes configuration to node
8. Status changes to ✓ Configured
9. Later: Physically wire button to Line 3 terminal
10. Configuration View: Click [Test] next to element
11. Event Monitor opens, filtered to Line 3 events
12. Press button → See Event On/Off appear with Event IDs
13. Now ready to create event links

**Outcome:** User can plan and configure their layout logically before dealing with physical wiring, reducing errors.

---

## Workflow 3: Creating Event Link (Drag to Bowtie)

**Scenario:** Front Door Button configured, want to connect to Tower Red Light.

**Steps:**

1. Event Bowties View
2. Tower Controller already has configured outputs, producing half-bowties:
   - Existing: "→ Tower / Line 5 / Event 3 / Red Light" (unconnected consumer)
3. Expand "23 unconnected elements"
4. Find "East Panel / Line 3 / Event On / Front Door →"
5. Drag producer element
6. Drop on unconnected consumer bowtie (Event ID: 05.02.01.02.00.00.00.10)
7. System copies Event ID 05.02.01.02.00.00.00.10 to Line 3 Event On
8. Bowtie updates: Now shows both producer and consumer
9. Element moves from unconnected list to connected bowtie
10. Test: Press physical button → Event Monitor shows event → Red light turns on

**Outcome:** Creating an event link is a simple drag-and-drop operation. No need to manually copy Event IDs or understand the protocol.

---

## Workflow 4: Creating Event Link (Consumer with Multiple Slots)

**Scenario:** Want Tower Red Light to respond to both Front Door Button and Emergency Button.

**Steps:**

1. Front Door → Red Light already linked (from Workflow 3)
2. Emergency Button configured on Line 8, Event On has different Event ID
3. Event Bowties View: Find bowtie "Emergency Button →" (unconnected producer)
4. Expand unconnected consumers: "Tower Controller / Port I/O"
5. See "Tower / Line 5" with sub-items:
   ```
   └─ Tower / Line 5
      ├─ → Event 1 (empty)
      ├─ → Event 2 (empty)
      ├─ → Event 3 (configured: 05.02.01...10) ← Already Front Door
      ├─ → Event 4 (empty)
      ├─ → Event 5 (empty)
      └─ → Event 6 (empty)
   ```
6. Drag "→ Event 4 (empty)"
7. Drop on "Emergency Button →" bowtie
8. System copies Emergency Button's Event ID to Line 5 Event 4
9. Now two separate bowties both target same consumer:
   - "Front Door → Tower Light (Event 3)"
   - "Emergency Button → Tower Light (Event 4)"
10. Either button now activates the same physical light

**Outcome:** Multiple producers can trigger the same consumer using different event slots. Visual interface makes this relationship clear.

---

## Workflow 5: Configuring Logic Element (Timer)

**Scenario:** Want light to turn off automatically 5 minutes after button press.

**Steps:**

1. Configuration View → Select "Tower Controller"
2. Segment: Select "Logic" or "Conditionals"
3. Group: Select "Timers"
4. Element: Select "Timer 1" (○ Unconfigured)
5. Configuration:
   - Function: "Delay Off Timer"
   - Delay: 300 seconds
   - Trigger Event: (empty, will link in Bowties view)
   - Output Event: (empty, will link in Bowties view)
   - User Name: "Light Auto-off Timer"
6. Click [Apply]
7. Switch to Event Bowties View
8. Find unconnected elements:
   - Producer: "Timer 1 / Output Event →"
   - Consumer: "→ Timer 1 / Trigger Event"
9. Drag "→ Timer 1 / Trigger" onto "Front Door / Event On" bowtie
   - Now button press triggers timer
10. Create new connection:
    - Drag "→ Tower / Line 5 / Event Off" 
    - Drop on "Timer 1 / Output →" bowtie
    - Now timer output turns off light
11. Result: Button press turns on light immediately + starts timer → 5 min later, timer turns off light

**Outcome:** Logic elements (timers, conditionals) are configured the same way as physical I/O. Event linking workflow remains consistent.

---

## Workflow 6: Diagnosing Issue with Event Monitor

**Scenario:** Button press doesn't turn on light, need to troubleshoot.

**Steps:**

1. Switch to Event Monitor View
2. Press physical button
3. Monitor shows:
   ```
   🟢 Event: 05.02.01.02.00.00.00.15
      Possible producers (1):
      • East Panel / Line 3 / Event On / Front Door
      Consumed by (0)  ← Problem!
   ```
4. No consumers listening → Configuration issue
5. Click "East Panel / Line 3" link → Jump to Configuration
6. See Event On = 05.02.01.02.00.00.00.15
7. Switch to Event Bowties, find "Tower / Line 5 / Event 3"
8. Check configuration → Event 3 = 05.02.01.02.00.00.00.10 (different!)
9. Fix: Use copy/paste or drag-drop to synchronize Event IDs
10. Test again: Monitor now shows consumer responding

**Outcome:** Event Monitor provides real-time visibility into protocol traffic, making it easy to diagnose misconfigurations. Links to configuration make fixing issues quick.

---

## Common Patterns

### Testing Physical Connections

1. Configure element in Configuration view
2. Open Event Monitor (filtered to that element)
3. Activate physical device (press button, trigger sensor)
4. Verify event appears in monitor with correct Event ID
5. Proceed to create event links

### Copying Event IDs

**Method 1: Copy/Paste in Configuration View**
1. Navigate to source element → Click [Copy] next to Event ID field
2. Navigate to target element → Click [Paste] next to Event ID field
3. Click [Apply] to save

**Method 2: Drag-to-Bowtie**
1. Find elements in Event Bowties View (unconnected list)
2. Drag onto existing bowtie
3. System automatically copies Event ID

### Understanding Existing Layout

1. Start with Event Bowties view
2. See all connected relationships at a glance
3. Click specific bowtie → Jump to detailed configuration
4. Use Event Monitor to observe live behavior
5. Validate understanding by testing physical actions

---

*See [docs/design/vision.md](vision.md) for overall product vision and feature descriptions.*
