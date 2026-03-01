# Feature Specification: Editable Node Configuration with Save

**Feature Branch**: `007-edit-node-config`  
**Created**: 2026-02-28  
**Status**: Draft  
**Input**: User description: "Make the configuration editable and then have a save button that will save all changes. Add write support in the lcc-rs crate. Show clear UX indication of where there are unsaved changes until you click the Save button."

## Clarifications

### Session 2026-02-28

- Q: If a user has validly-edited fields and one field with invalid input, should all saving be blocked until the invalid field is fixed? → A: Yes — block all saves while any field has invalid input (user must fix or revert the invalid field first).
- Q: Should this feature handle Action buttons and Blob CDI field types? → A: No — exclude both. Render them as read-only/non-editable; defer to a future feature.
- Q: What should the UX look like during a save operation while multiple fields are being written sequentially? → A: Show a progress indicator (e.g., "Writing 3 of 7...") near the Save button, with fields transitioning to clean as each write completes.
- Q: Should this feature support writing to the ACDI User space (0xFB) in addition to the configuration space (0xFD)? → A: Yes — include ACDI User space. User name and description are editable alongside regular config fields.
- Q: Should the system prevent the user from navigating away from a field with invalid input, or allow free navigation? → A: Allow free navigation — the invalid field stays visually marked and blocks Save, but the user can freely move to other fields.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Edit and Save a Single Configuration Field (Priority: P1)

A user navigating the configuration tree of a connected LCC node sees a field (e.g., a string name or an integer setting) and wants to change its value. They click or focus the field, type a new value, see the field visually marked as "modified/unsaved," and then click a Save button to write the change to the node. After saving, the field's unsaved indicator clears.

**Why this priority**: This is the core value proposition — without the ability to edit and persist a single field, no other editing feature matters. It covers the full round-trip: edit → visual feedback → write → confirmation.

**Independent Test**: Can be fully tested by connecting to any node with a writable CDI string or integer field, changing the value, saving, and confirming the field reflects the written value. Delivers the fundamental ability to configure an LCC node.

**Acceptance Scenarios**:

1. **Given** a node's configuration is displayed with read values loaded, **When** the user modifies a string field's text, **Then** that field is visually marked as having unsaved changes (e.g., colored highlight or badge).
2. **Given** one or more fields are marked as unsaved, **When** the user clicks the Save button, **Then** all modified field values are written to the node and, upon successful write, the unsaved indicators clear.
3. **Given** a field has been modified, **When** the user changes it back to its original (last-read) value, **Then** the unsaved indicator for that field is removed automatically.
4. **Given** no fields have been modified, **When** the user views the Save button, **Then** the Save button is disabled or visually indicates there is nothing to save.

---

### User Story 2 - Edit Fields with Constrained Values (Priority: P1)

A user encounters an integer field that has a defined set of allowed values (map/enum entries from the CDI), such as a signal aspect selector or a turnout direction toggle. Instead of typing a raw number, they select from the defined options via a dropdown. The field shows the human-readable label and writes the corresponding numeric value.

**Why this priority**: Many LCC configuration fields use constrained value maps. Without dropdown/select support, users would need to know raw numeric codes, making the tool impractical for real-world use. This is equally critical as free-text editing.

**Independent Test**: Can be tested by finding any CDI field with `<map>` entries, verifying the dropdown renders with labels, selecting a different option, confirming the unsaved indicator appears, and saving.

**Acceptance Scenarios**:

1. **Given** an integer field has CDI map entries defined, **When** the field is rendered, **Then** it displays as a dropdown/select control showing the human-readable labels.
2. **Given** a dropdown field currently shows "Option A," **When** the user selects "Option B," **Then** the field is marked as unsaved and the pending value corresponds to the numeric value mapped to "Option B."
3. **Given** a dropdown field has been changed, **When** the user saves, **Then** the correct numeric value (not the label text) is written to the node's memory.

---

### User Story 3 - Edit Event ID Fields (Priority: P2)

A user needs to change an event ID on a node (e.g., reassigning which event a producer emits or a consumer listens for). They edit the event ID field, which accepts dotted-hex notation (e.g., `05.01.01.01.22.00.00.FF`). The field validates the format before allowing save.

**Why this priority**: Event IDs are central to LCC node configuration. However, event IDs are less commonly edited manually (often taught via physical button interactions), making this P2 rather than P1.

**Independent Test**: Can be tested by editing an event ID field to a valid new value, saving, and verifying the 8-byte value was written correctly. Also test invalid input is rejected.

**Acceptance Scenarios**:

1. **Given** an event ID field is displayed, **When** the user modifies the dotted-hex text, **Then** the field validates that the input is exactly 8 bytes in dotted-hex format.
2. **Given** an event ID field has invalid input (wrong number of bytes, non-hex characters), **When** the user attempts to save, **Then** the field is visually marked as invalid and the save does not proceed for that field.
3. **Given** a valid event ID has been entered, **When** the user saves, **Then** the 8-byte binary value is written to the correct memory address on the node.

---

### User Story 4 - Global Unsaved Changes Awareness (Priority: P2)

A user has modified several fields across different groups within a segment. They can see at a glance which fields have pending changes — through consistent visual highlighting on each modified field. A summary or count near the Save button tells them how many changes are pending. In the sidebar, the node entry and each segment entry that contains unsaved changes are also visually marked (e.g., a dot, badge, or highlight), so the user always knows which nodes and which segments have pending edits — even before selecting them. If they try to navigate away (selecting a different node or segment), they are warned about unsaved changes.

**Why this priority**: Without global change awareness, users may lose edits or forget what they changed. Sidebar-level indicators are especially important when multiple nodes are listed — the user needs to know which nodes still need attention without clicking into each one. This is essential for a trustworthy editing experience but not as fundamental as the write mechanics themselves.

**Independent Test**: Can be tested by modifying 3+ fields across different groups, verifying the field-level indicators, the Save-area count, the sidebar node indicator, and the sidebar segment indicator all appear. Then navigate to a different segment and confirm the warning dialog appears.

**Acceptance Scenarios**:

1. **Given** multiple fields have been modified, **When** the user views the Save area, **Then** a count or summary of pending changes is visible (e.g., "3 unsaved changes").
2. **Given** one or more fields in a node have unsaved changes, **When** the user views the sidebar node list, **Then** that node's entry displays a visual indicator (e.g., badge or dot) showing it has unsaved changes.
3. **Given** one or more fields within a specific segment have unsaved changes, **When** the user views the sidebar segment list under that node, **Then** that segment's entry displays a visual indicator showing it has unsaved changes.
4. **Given** unsaved changes exist for the current node, **When** the user attempts to select a different node or segment, **Then** a confirmation dialog warns about unsaved changes and offers to save, discard, or cancel navigation.
5. **Given** all changes have been saved or discarded, **When** the user views the sidebar, **Then** no node or segment entries show unsaved-change indicators.
6. **Given** all changes have been saved or discarded, **When** the user navigates to a different node, **Then** no warning is shown.

---

### User Story 5 - Handle Write Failures Gracefully (Priority: P2)

A user clicks Save, but one or more writes fail (e.g., node goes offline, timeout, or the node rejects the datagram). The user sees which specific fields failed to save, with an error indication distinct from the "unsaved" indicator. They can retry saving just the failed fields.

**Why this priority**: Write failures are inevitable in real-world LCC networks (nodes power off, CAN bus errors, etc.). Without clear error handling, users cannot trust that their saves succeeded.

**Independent Test**: Can be tested by disconnecting the node mid-save or simulating a datagram rejection, then verifying the error indicators appear on the correct fields.

**Acceptance Scenarios**:

1. **Given** a save is in progress, **When** a write to a specific field fails after retries, **Then** that field is marked with an error indicator distinct from "unsaved."
2. **Given** some fields saved successfully and others failed, **When** the save operation completes, **Then** only the failed fields retain their error/unsaved state; successful fields return to clean state.
3. **Given** fields are in error state, **When** the user clicks Save again, **Then** only the failed/unsaved fields are re-attempted (already-saved fields are not re-written).

---

### User Story 6 - Discard Changes (Priority: P3)

A user has made several edits but decides they want to revert to the values currently on the node. They click a Discard button that reverts all modified fields back to their last-read values.

**Why this priority**: A safety mechanism to undo unintended edits. Lower priority because users can also simply not click Save, but an explicit discard action improves confidence.

**Independent Test**: Can be tested by editing several fields, clicking Discard, and confirming all fields revert to original values with unsaved indicators cleared.

**Acceptance Scenarios**:

1. **Given** one or more fields have unsaved changes, **When** the user clicks Discard, **Then** a confirmation prompt appears asking to confirm reverting all changes.
2. **Given** the user confirms discard, **When** the discard executes, **Then** all fields revert to the last-read values and all unsaved/error indicators clear.
3. **Given** no fields have unsaved changes, **When** the user views the Discard button, **Then** the Discard button is disabled.

---

### Edge Cases

- What happens when a field's CDI-defined size constraint is exceeded (e.g., a string longer than the max size)? The field must enforce the max length and prevent over-length input.
- What happens when a node disconnects while the user has unsaved edits? Unsaved edits should be preserved in the UI so the user can retry when the node reconnects.
- What happens when the user edits a field and another process (e.g., physical button teach) changes the same value on the node? The local edit should take precedence until explicitly saved or discarded; a future enhancement could detect and flag conflicts.
- What happens when writing to a field that spans more than 64 bytes (e.g., long strings)? The write must be chunked into sequential 64-byte datagram payloads, each acknowledged before sending the next.
- What happens when the CDI defines a field with min/max constraints and the user enters an out-of-range value? The field must validate against constraints and show an invalid state, blocking save for that field.
- What happens when the user edits a float field and enters non-numeric text? Input validation must reject non-numeric values and show an invalid indicator.

## Requirements *(mandatory)*

### Functional Requirements

#### Editing

- **FR-001**: System MUST render string-type CDI fields as editable text inputs, allowing the user to type new values.
- **FR-002**: System MUST render integer-type CDI fields with map entries as dropdown/select controls, displaying human-readable labels.
- **FR-003**: System MUST render integer-type CDI fields without map entries as editable numeric inputs.
- **FR-004**: System MUST render event ID fields as editable text inputs accepting dotted-hex notation (8 bytes, e.g., `05.01.01.01.22.00.00.FF`).
- **FR-005**: System MUST render float-type CDI fields as editable numeric inputs.
- **FR-006**: System MUST enforce CDI-defined constraints during editing: max string length, integer min/max bounds.
- **FR-007**: System MUST NOT allow editing of fields on nodes that are offline/disconnected (inputs should be disabled).
- **FR-007a**: System MUST render Action and Blob CDI field types as read-only/non-editable. These types are out of scope for this feature.

#### Dirty/Change Tracking

- **FR-008**: System MUST track per-field dirty state by comparing the current input value to the last-read value from the node.
- **FR-009**: System MUST visually distinguish four field states: **clean** (matches node value), **unsaved** (modified but not yet written), **error** (write failed), and **invalid** (fails validation — e.g., out-of-range number, malformed event ID). The user MUST be able to freely navigate away from an invalid field; focus is not trapped. The invalid indicator persists until the field is corrected or reverted.
- **FR-011**: System MUST auto-clear the unsaved indicator when the user changes a field back to its original value.
- **FR-012**: System MUST display a count or summary of pending unsaved changes near the Save button.
- **FR-012a**: System MUST display a visual indicator (e.g., badge, dot, or highlight) on each node entry in the sidebar that has one or more unsaved field changes.
- **FR-012b**: System MUST display a visual indicator on each segment entry in the sidebar that contains one or more unsaved field changes within that segment.
- **FR-012c**: Sidebar unsaved-change indicators MUST update in real time as fields are edited, saved, or reverted.

#### Saving

- **FR-013**: System MUST provide a Save button that writes all unsaved field values to the node.
- **FR-013a**: While a save operation is in progress, the system MUST display a progress indicator near the Save button showing the current write count out of total (e.g., "Writing 3 of 7..."). Each field MUST transition to clean state as its individual write completes successfully.
- **FR-014**: System MUST disable the Save button when there are no unsaved changes or when any field has invalid input. All saves are blocked until every invalid field is either fixed or reverted to its original value.
- **FR-015**: System MUST write each field individually to the node's memory at the correct address and space, using the Memory Configuration Protocol write datagram. Supported writable spaces include the configuration space (`0xFD`) and the ACDI User space (`0xFB`).
- **FR-016**: System MUST serialize values correctly per type: integers as big-endian bytes of the CDI-defined size, strings as UTF-8 with null terminator (minimal length: string bytes + 1 null byte, NOT padded to full CDI-defined size, per OpenLCB_Java reference), event IDs as 8 raw bytes, floats as IEEE 754 big-endian (4-byte single or 8-byte double, matching CDI-defined size).
- **FR-017**: System MUST chunk writes larger than 64 bytes into sequential ≤64-byte datagrams, waiting for acknowledgment of each chunk before sending the next.
- **FR-018**: System MUST handle write acknowledgment (Datagram Received OK) and clear the field's unsaved state upon successful write.
- **FR-019**: System MUST retry failed writes up to 3 times with a timeout of 3 seconds per attempt.
- **FR-020**: System MUST mark fields with an error state if all write retries are exhausted without acknowledgment.
- **FR-021**: System MUST update its local value cache with the written value upon successful write, so subsequent reads reflect the new value without re-reading from the node.
- **FR-022**: System MUST send an "Update Complete" indicator (command `0xA8`) to the node after all writes in a save operation complete successfully, signaling the node to apply/persist changes.

#### Discarding

- **FR-023**: System MUST provide a Discard button that reverts all unsaved fields to their last-read values.
- **FR-024**: System MUST prompt for confirmation before discarding when unsaved changes exist.
- **FR-025**: System MUST disable the Discard button when there are no unsaved changes.

#### Navigation Guards

- **FR-026**: System MUST warn the user when navigating away from a node or segment that has unsaved changes, offering options to save, discard, or cancel.

#### Write Protocol Support

- **FR-027**: The protocol library MUST support constructing Memory Configuration Protocol write datagrams with the correct command bytes (`0x00`–`0x03`, mirroring read command bytes `0x40`–`0x43` by address space encoding), address encoding, space encoding, and data payload.
- **FR-028**: The protocol library MUST support the write acknowledgment flow: write datagram → Datagram Received OK (no reply datagram expected unless `FLAG_REPLY_PENDING` is set).
- **FR-029**: The protocol library MUST support constructing and sending the "Update Complete" command (`0x20, 0xA8`) after a batch of writes.

### Key Entities

- **PendingEdit**: Represents a field that has been modified but not yet saved. Attributes: field path, original value (last-read), current edited value, field type, memory address, memory space, data size, validation state (valid/invalid), write state (unsaved/writing/error/clean).
- **WriteResult**: Outcome of a write operation for a single field. Attributes: field path, success/failure status, error details (if failed), retry count.
- **ConfigValue**: An already-existing entity representing a typed configuration value (int, string, eventId, float). Extended to support serialization for write operations.

## Assumptions

- The application is already connected to the LCC network and has read configuration values for the target node before editing begins (feature 004-read-node-config is a prerequisite).
- The CDI XML has been parsed and the memory address of each field is known.
- The Memory Configuration Protocol write command format follows the OpenLCB standard: datagram type `0x20`, subcommand `0x00` for write, with address and space encoding identical to the read command but with data payload appended.
- Writes to individual fields happen sequentially (not in parallel) to avoid overwhelming the target node's datagram processing.
- The target node will process writes to the configuration address space (`0xFD`) and the ACDI User space (`0xFB`) and respond with Datagram Received OK.
- No write-then-verify (read-back) is required; successful datagram acknowledgment is sufficient confirmation, consistent with the reference OpenLCB_Java implementation.
- The "Update Complete" command (`0xA8`) is sent once after all fields in a save batch are written, not after each individual field write.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Users can edit and save a configuration value on a connected LCC node in under 30 seconds (from clicking a field to seeing the save confirmation).
- **SC-002**: 100% of CDI field types (string, integer, integer-with-map, event ID, float) are rendered as appropriate editable controls.
- **SC-003**: Users can visually identify all unsaved fields at a glance — every modified field has a distinct visual indicator.
- **SC-004**: The system correctly handles write failures by showing per-field error states, with no silent data loss (user always knows if a write did not succeed).
- **SC-005**: Users are prevented from accidentally losing edits — navigation away from unsaved changes always triggers a warning.
- **SC-006**: Write operations complete within 5 seconds per field under normal network conditions (including retry overhead).
- **SC-007**: Input validation catches 100% of invalid values (out-of-range integers, malformed event IDs, over-length strings) before attempting a write.
