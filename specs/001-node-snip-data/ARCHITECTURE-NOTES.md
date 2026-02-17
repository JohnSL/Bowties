# Architecture Notes: User Story 3 Automatic Discovery

**Date**: February 16, 2026  
**Status**: Deferred - Architecture refactoring required  
**Context**: User Stories 1 & 2 are complete and working. User Story 3 attempted but reverted.

## Problem Encountered

When implementing User Story 3 (Automatic Discovery of New Nodes), we discovered a fundamental architectural limitation with the current LCC transport design:

### The Issue

- **Single TCP connection** holds exclusive ownership of the transport layer
- **Background listener** attempted to create a **second TCP connection** to monitor network traffic
- **Result**: The second connection consumed frames meant for the main connection
- **Symptom**: Discovery and SNIP queries hung indefinitely, waiting for responses that were consumed by the background listener

### Why It Happened

```rust
// Current architecture (PROBLEMATIC)
pub struct LccConnection {
    transport: Box<dyn LccTransport>,  // Exclusive ownership
    our_alias: NodeAlias,
}

// When we tried to add background listener:
// 1. Main connection has transport #1
// 2. Background listener creates transport #2 (separate TCP connection)
// 3. TCP hub sends responses to BOTH connections
// 4. Background listener consumes frames → main connection never sees them
```

## Required Architecture Refactoring

To properly support User Story 3 (automatic discovery), we need to refactor the transport layer to support **shared frame access** with multiple consumers.

### Option 1: Shared Transport with Arc<Mutex<>>

```rust
pub struct LccConnection {
    transport: Arc<Mutex<Box<dyn LccTransport>>>,
    our_alias: NodeAlias,
}

// Benefits:
// - Single TCP connection
// - Multiple consumers can lock and read frames
// - No duplicate connections

// Drawbacks:
// - Mutex contention if many consumers
// - Need careful lock management to avoid deadlocks
```

### Option 2: Frame Broadcasting Channel

```rust
pub struct LccConnection {
    transport: Box<dyn LccTransport>,
    frame_broadcaster: FrameBroadcaster,  // mpsc::channel based
}

// FrameBroadcaster receives all frames and duplicates them
// to multiple subscribers (main connection, background listener, etc.)

// Benefits:
// - Decouples consumers
// - Each consumer gets all frames
// - No contention

// Drawbacks:
// - More complex implementation
// - Memory overhead for frame duplication
// - Need to handle backpressure
```

### Option 3: Single Connection with Request/Response Tracking

```rust
// Track pending requests and route responses
pub struct LccConnection {
    transport: Box<dyn LccTransport>,
    request_tracker: RequestTracker,  // Maps request_id → Response channel
}

// Benefits:
// - Clean separation of request/response
// - Multiple concurrent operations
// - Single TCP connection

// Drawbacks:
// - Need to handle unsolicited frames (like Verified Node broadcasts)
// - More complex state management
```

## Recommended Approach

**Hybrid: Option 2 (Frame Broadcasting) + Option 3 (Request Tracking)**

1. **Single TCP connection** reads all frames from network
2. **Frame Broadcaster** duplicates each frame to:
   - Request/Response tracker (for query_snip, verify_node, etc.)
   - Background listener (for Verified Node broadcasts)
   - Any other future consumers
3. **Request Tracker** matches responses to pending requests
4. **Background Listener** filters for Verified Node MTI and emits discovery events

### Implementation Sketch

```rust
pub struct FrameBroadcaster {
    subscribers: Vec<mpsc::Sender<GridConnectFrame>>,
}

impl FrameBroadcaster {
    async fn broadcast(&self, frame: GridConnectFrame) {
        for subscriber in &self.subscribers {
            let _ = subscriber.send(frame.clone()).await;
        }
    }
}

pub struct LccConnection {
    transport: Box<dyn LccTransport>,
    broadcaster: Arc<FrameBroadcaster>,
    request_tracker: RequestTracker,
    background_listener_rx: mpsc::Receiver<GridConnectFrame>,
}
```

## Migration Path

### Phase 1: Refactor Transport Layer (Breaking Changes)
- Implement FrameBroadcaster
- Add request/response tracking
- Update all existing commands to use new pattern
- **Impact**: All Tauri commands need updates

### Phase 2: Implement Background Listener (New Feature)
- Add background listener using broadcaster
- Filter for Verified Node MTI (0x19170)
- Extract node info and emit Tauri events
- Auto-trigger SNIP queries for new nodes

### Phase 3: Testing & Validation
- Test with multiple nodes joining/leaving
- Verify no frame loss or duplication
- Performance testing with 50+ nodes
- Integration tests with Python POC as reference

## Lessons Learned

1. ✅ **Multiple TCP connections don't work** - LCC hub sends frames to all connections
2. ✅ **Shared state requires careful design** - Rust ownership rules highlighted the issue early
3. ✅ **Incremental implementation was wise** - User Stories 1 & 2 work independently
4. ✅ **Architecture matters** - Trying to bolt on features exposed design limitations

## Current Working State

**User Story 1**: ✅ View Discovered Nodes with Friendly Names  
**User Story 2**: ✅ On-Demand Node Status Verification  
**User Story 3**: ⚠️ **Deferred** - Requires architecture refactoring (documented above)

The application is **fully functional** for manual discovery and refresh workflows. Automatic discovery can be added in a future iteration after the transport layer refactoring.

## Next Steps

When ready to implement User Story 3:

1. Read this document and review the recommended approach
2. Create a new feature branch: `002-transport-refactor` or similar
3. Implement FrameBroadcaster and request tracking
4. Update all existing Tauri commands to use new architecture
5. Add comprehensive tests for frame distribution
6. Then implement User Story 3 (automatic discovery)
7. Validate with end-to-end testing

---

**Note**: This is **not a failure** - it's **good software engineering**. We identified an architectural limitation early, documented it thoroughly, and avoided shipping broken code. The working features (US1 & US2) remain stable and valuable.
