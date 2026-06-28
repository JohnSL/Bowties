# IPC Command Contracts — Block Indicator Facility (Spec 018)

All commands are Tauri IPC commands exposed by `app/src-tauri/src/lib.rs`. Each command has a Rust handler in the noted command module, returns `Result<T, AppError>` (Bowties' canonical error envelope), and a typed TS wrapper in `app/src/lib/api/`.

**Convention**: command names use `snake_case` in Rust and become `camelCase` on the TS side (per existing Bowties IPC convention).

---

## New commands

### `list_facilities`

**Module**: `commands/facilities.rs`
**Purpose**: Load every persisted facility from `facilities.yaml` for the currently-open layout.

```ts
// TS wrapper signature
function listFacilities(): Promise<Facility[]>;
```

```rust
#[tauri::command]
async fn list_facilities(state: State<AppState>) -> Result<Vec<FacilityDto>, AppError>;
```

**Behaviour**:
- Returns the persistent baseline (post-save) facility set, not the draft layer. Frontend layers drafts on top.
- Returns `[]` if `facilities.yaml` is absent (a fresh or pre-018 layout) — no error.
- Errors only on YAML parse failure or schema-version mismatch from a future release.

---

### `create_facility`

**Module**: `commands/facilities.rs`
**Purpose**: Stage a new facility for the current layout (write-on-save per ADR-0012).

```ts
function createFacility(args: {
  templateId: string;
  name: string;
}): Promise<Facility>;            // returns the newly created facility with assigned facilityId
```

```rust
#[tauri::command]
async fn create_facility(
  template_id: String,
  name: String,
  state: State<AppState>,
) -> Result<FacilityDto, AppError>;
```

**Behaviour**:
- Backend mints a UUID v4 `facilityId`, materialises slot labels with `null` bindings from the named template, and returns the new facility object.
- **Does not** write to `facilities.yaml`; the change is held in the in-memory draft layer (mirrored by the `facilities.svelte.ts` store) until `save_layout` flushes deltas.
- Errors if `template_id` is not a registered template.

---

### `rename_facility`

**Module**: `commands/facilities.rs`

```ts
function renameFacility(args: { facilityId: string; newName: string }): Promise<void>;
```

```rust
#[tauri::command]
async fn rename_facility(facility_id: String, new_name: String, state: State<AppState>)
  -> Result<(), AppError>;
```

**Behaviour**:
- Staged write through the draft layer.
- Errors if `facilityId` is unknown.
- No uniqueness constraint on names (per spec clarifications; reuses spec 015 rename semantics).

---

### `delete_facility`

**Module**: `commands/facilities.rs`

```ts
function deleteFacility(args: { facilityId: string }): Promise<void>;
```

```rust
#[tauri::command]
async fn delete_facility(facility_id: String, state: State<AppState>) -> Result<(), AppError>;
```

**Behaviour**:
- Stages deletion of the facility. User-owned channels bound to its slots are also staged for deletion. Hardware-owned channels bound to its slots are staged for unbinding only.
- Bowties whose `createdByFacility == facility_id` are staged for deletion via the existing bowtie-delete path.
- Errors if `facilityId` is unknown.

---

### `bind_slot`

**Module**: `commands/facilities.rs`

```ts
function bindSlot(args: {
  facilityId: string;
  slotLabel: string;
  channelId: string;
}): Promise<void>;
```

```rust
#[tauri::command]
async fn bind_slot(
  facility_id: String,
  slot_label: String,
  channel_id: String,
  state: State<AppState>,
) -> Result<(), AppError>;
```

**Behaviour**:
- Stages a `slotBindings[slot_label] = channel_id` change.
- Validates: facility exists; slot label belongs to the facility's template; channel exists; channel.role == template.slot.requiredRole; channel is not already bound to another slot in any facility.
- The Incomplete↔Wired transition is computed at the orchestrator/store layer; if the bind moves the facility to Wired, the frontend `facilityOrchestrator` composes existing bowtie-creation IPC calls to lay down the underlying bowtie(s) with `createdByFacility` set.

---

### `unbind_slot`

**Module**: `commands/facilities.rs`

```ts
function unbindSlot(args: {
  facilityId: string;
  slotLabel: string;
}): Promise<void>;
```

```rust
#[tauri::command]
async fn unbind_slot(
  facility_id: String,
  slot_label: String,
  state: State<AppState>,
) -> Result<(), AppError>;
```

**Behaviour**:
- Stages `slotBindings[slot_label] = null`.
- If the channel that was bound is user-owned and no longer in any slot afterward, it is also staged for deletion (and its lamp row's style-locked constraints lift).
- If the facility was Wired, the orchestrator initiates the existing slot-detach pipeline for the facility's bowtie(s).

---

### `create_user_owned_channel`

**Module**: `commands/channels.rs` (extends the existing channels command module)

```ts
function createUserOwnedChannel(args: {
  role: ChannelRole;
  style: ChannelStyle;
  binding: ChannelBinding;
  defaultName?: string;       // if omitted, backend generates per spec 015 default-name rules
}): Promise<Channel>;
```

```rust
#[tauri::command]
async fn create_user_owned_channel(
  role: String,
  style: String,
  binding: ChannelBindingDto,
  default_name: Option<String>,
  state: State<AppState>,
) -> Result<ChannelDto, AppError>;
```

**Behaviour**:
- Mints UUID v4 `channelId`; sets `ownership = 'user-owned'`.
- Validates: style is registered and user-creatable; style's role equals the requested role; binding.kind matches the style's claim shape; binding target is unclaimed and constraint-compatible.
- Staged write through the channel store's draft layer. Frontend's `facilityOrchestrator` calls this and `bind_slot` atomically (failure of either rolls back the staged channel via the draft layer's discard path).
- The Add-channel sub-picker enumerates eligible targets *client-side* from the cached CDI tree (R9); this command is the persistence half of that flow.

---

### `list_behavior_templates`

**Module**: `commands/behavior_templates.rs`

```ts
function listBehaviorTemplates(): Promise<BehaviorTemplate[]>;
```

```rust
#[tauri::command]
fn list_behavior_templates() -> Vec<BehaviorTemplateDto>;
```

**Behaviour**:
- Returns the contents of `bowties_core::behavior_templates::registered_templates()` serialised to JSON.
- Stateless; no `Result` wrapper needed (cannot fail).
- Called once on app start by the frontend `behaviorTemplates.svelte.ts` store.

---

## Extended commands (already exist; new behaviour or new fields)

### `list_channels` (extended)

**Module**: `commands/channels.rs`
**Existing signature** remains. **What changes**: the returned `ChannelDto` now includes `role`, `style`, `ownership`, and a discriminated `binding` field (per data-model.md). The legacy `channelType` + `hardwareRef` fields are kept in the Slice 2 transitional window and removed in Slice 6.

### `rename_channel` (unchanged signature)

**Module**: `commands/channels.rs`
**What changes**: the implementation now applies the rename to either hardware-owned or user-owned channels indifferently. Rename does not change `ownership`.

### `delete_channels` (extended)

**Module**: `commands/channels.rs`
**What changes**: deletion of a hardware-owned channel is rejected (it must come from the cascade on hardware-config clear, not from a direct delete). Deletion of a user-owned channel is rejected if it is currently bound to a slot (callers must `unbind_slot` first; `delete_facility` orchestrates both atomically).

### Save flow — `saveLayoutOrchestrator` delta collection

The save flow (no IPC change) now collects facility deltas alongside channel and connector-selection deltas. Backend `save_layout` writes `facilities.yaml` through the existing journal (ADR-0006). All four files (`bowties.yaml`, `channels.yaml`, `facilities.yaml`, `manifest.yaml` if metadata changed) commit atomically.

---

## Cross-command invariants and ordering

1. **Atomic Add-channel flow** (FR-018): the orchestrator must call `create_user_owned_channel` then `bind_slot` in immediate succession with a rollback-on-failure contract. The backend exposes both as staged operations (the draft layer makes rollback cheap — a discard of either staged change is O(1)).

2. **Atomic facility deletion** (FR-017): `delete_facility` is one command; the backend stages all dependent unbinds, user-owned-channel deletions, and bowtie deletions in one transactional change set.

3. **Atomic Wired transition** (FR-021): not a single command. The orchestrator (`facilityOrchestrator`) observes the slot-fill that completes a facility and then composes calls to the existing bowtie-creation IPC chain. The `createdByFacility` field on the new bowtie(s) is the durable link.

4. **Atomic Incomplete transition** (FR-022): similarly, the orchestrator observes the slot-empty (or cascade from hardware clear) and composes calls to the existing bowtie-delete path, scoped to bowties with the matching `createdByFacility`.

5. **Hardware-clear cascade**: `connectorSelectionOrchestrator` step 4's deletion of hardware-owned channels publishes a notification; `facilityOrchestrator` listens, identifies any Wired facility whose slot was bound to one of those channels, and triggers the Incomplete transition. The end-user observation is one atomic step (no half-Wired state visible).

---

## Error model

All `Result<T, AppError>` returns carry one of:

- `AppError::NotFound { kind, id }` — facility / channel / template / hardware target absent
- `AppError::InvalidArgument { reason }` — schema/contract violation (e.g., role mismatch, binding-kind mismatch)
- `AppError::StyleConstraintViolation { target, reason }` — constraint contract rejects the binding
- `AppError::AlreadyBound { channelId, currentSlot }` — channel already bound elsewhere
- `AppError::PersistenceError { path, source }` — disk / YAML failure

Errors are translated to `Error` objects on the TS side with the same shape (existing convention) and surfaced as toast / inline messages by the calling component.
