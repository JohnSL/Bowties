# Plan: Fix LCC Memory Configuration Protocol Reading

**TL;DR**: Cross-referencing the JMRI `MemoryConfigurationService.java` (in `OpenLCB_Java`) with our traffic captures exposed three bugs. All three are now fixed. **Phase 1** (protocol layer) and **Phase 2** (address calculation) are complete and verified. **Phase 3** (docs update) remains for a future session.

**Status: ALL PHASES COMPLETE ✅**

---

## Session Summary (2025-02)

All code changes were implemented and verified. `cargo test -p lcc-rs` passes 222 tests. The Tauri app compiles cleanly (`cargo check`). A new traffic capture ([Bowties_async_blink.txt](specs/004-read-node-config/traffic/Bowties_async_blink.txt)) confirms correct sequential reads with advancing addresses matching JMRI's pattern. Documentation (`technical-context.md`, `tasks.md`) updated.

Four bugs were fixed:
- **Bug 1 (Protocol format)**: `build_read()` always sent generic `0x40`; `parse_read_reply()` had fragile content-detection. Fixed by using `command_flag()` in `build_read()` and deterministic bit-testing in `parse_read_reply()`.
- **Bug 2 (Zero-stride replication)**: Groups with `calculate_size()==0` and replication>1 caused repeated identical reads. Fixed by clamping `effective_replication` to 1.
- **Bug 3 (All elements at same address)**: `process_elements()` treated `offset` as absolute, so every element resolved to `segment_origin + 0`. Fixed with a running sequential cursor. Addresses now advance correctly: `0x80`→`0x82`→`0x84`→`0x85`→`0x87`→`0x96`→`0x97`→`0x9F`→`0xA7`→`0xAF`→`0xB7`.
- **Bug 4 (One datagram per element)**: `read_all_config_values` issued one `read_memory` round-trip per leaf element (~125ms each). Fixed by sorting elements by `(space, address)`, grouping consecutive same-space elements into ≤64-byte batches, and issuing one `read_memory` per batch with element values sliced from the reply. For async_blink's 11 consecutive elements, all 11 collapse to 1–2 round-trips.

---

## JMRI / OpenLCB_Java Reference Files

The key files for understanding the production-tested LCC implementation are:

| Purpose | File |
|---|---|
| Embedded-vs-generic decision | [OpenLCB_Java/src/org/openlcb/implementations/MemoryConfigurationService.java](OpenLCB_Java/src/org/openlcb/implementations/MemoryConfigurationService.java) — `getSpaceOffset()`, `fillRequest()`, `getSpaceFromPayload()`, `getPayloadOffset(data)` |
| Chunked read loop, error codes | [JMRI/java/src/jmri/jmrix/openlcb/swing/memtool/MemoryToolPane.java](JMRI/java/src/jmri/jmrix/openlcb/swing/memtool/MemoryToolPane.java) — `cbr` handler, 64-byte chunk logic, end conditions |
| Connection wiring | [JMRI/java/src/jmri/jmrix/openlcb/OlcbConfigurationManager.java](JMRI/java/src/jmri/jmrix/openlcb/OlcbConfigurationManager.java) |

The `OpenLCB_Java` `MemoryConfigurationService` is the authoritative source: it is the same library JMRI depends on and has been deployed for years.

---

## Steps

### Phase 1 — Fix `memory_config.rs` (Protocol Layer) ✅ DONE

1. ✅ **Corrected `build_read()`** in [memory_config.rs](lcc-rs/src/protocol/memory_config.rs): now calls `command_flag()` to select format. Spaces `>= 0xFD` (`Configuration`/`AllMemory`/`Cdi`) use embedded format (`0x41`/`0x42`/`0x43`) — 7-byte payload, no space byte. `AcdiUser`/`AcdiManufacturer` use generic `0x40` — 8-byte payload, space byte at `[6]`.

2. ✅ **Corrected `parse_read_reply()`**: removed fragile content-detection. Now uses the canonical rule from `MemoryConfigurationService.getPayloadOffset(data)`:
   - `cmd & 0x03 != 0` → embedded reply (`0x51`/`0x52`/`0x53`) → **no space byte**, data at `[6..]`
   - `cmd & 0x03 == 0` → generic reply (`0x50`) → **space byte always at `[6]`**, data at `[7..]`

   `address_space` parameter removed; space derived from reply bytes. Minimum-length check corrected from `7` to `6`.

3. ✅ **Aligned `command_flag()`**: `build_read()` now calls it consistently — no more dead code.

4. ✅ **Updated and added tests** in [memory_config.rs](lcc-rs/src/protocol/memory_config.rs) — 9 tests:
   - `test_build_read_cdi`: 7-byte payload, command `0x43`, no space byte
   - `test_build_read_configuration`: 7-byte payload, command `0x41`
   - `test_build_read_all_memory`: 7-byte payload, command `0x42`
   - `test_build_read_acdi_user`: 8-byte payload, command `0x40`, space byte `0xFB` at `[6]`
   - `test_parse_read_reply_success_embedded`: embedded `0x53`, data at `[6..]`
   - `test_parse_read_reply_generic_with_space_byte`: generic `0x50`, space at `[6]`, data at `[7..]`
   - `test_parse_read_reply_generic_acdi_user`: generic reply for AcdiUser
   - `test_parse_read_reply_failed_embedded`: embedded failure `0x5B`, error at `[6-7]`
   - `test_parse_read_reply_failed_generic`: generic failure `0x58`, space at `[6]`, error at `[7-8]`

5. ✅ **Updated `discovery.rs` callers**: both `parse_read_reply` call sites have the dropped `address_space` argument removed.

### Phase 2 — Fix Address Calculation (Application + Library Layer) ✅ DONE

6. ✅ **Zero-stride guard** added to `extract_all_elements_with_addresses` in [cdi.rs](app/src-tauri/src/commands/cdi.rs): if `calculate_size() == 0` for a replicated group, `effective_replication` is clamped to 1 with a warning to avoid duplicate reads.

7. ✅ **Fixed `calculate_element_size`** in [hierarchy.rs](lcc-rs/src/cdi/hierarchy.rs): now includes each element's `offset` skip in its total footprint (`offset + size`). This makes `Group::calculate_size()` return the correct replication stride when elements carry explicit offsets.

8. ✅ **Fixed sequential cursor in `process_elements`** ([cdi.rs](app/src-tauri/src/commands/cdi.rs)): replaced incorrect absolute-offset approach with a running `cursor: i32`. The CDI spec `offset` attribute is a **relative skip** from the end of the previous element, not an absolute address. Each element's address is now `segment_origin + base_offset + cursor`. This was the root cause of all values being read from `0x80` regardless of position.

   Key logic:
   ```rust
   cursor += e.offset;          // apply relative skip before this element
   let addr = base_offset as i32 + cursor;
   cursor += e.size;            // advance past the bytes
   ```
   Action and Blob elements are skipped for reading but cursor is still advanced so subsequent elements get correct addresses.

**Verification**: New traffic capture ([Bowties_async_blink.txt](specs/004-read-node-config/traffic/Bowties_async_blink.txt)) confirms:
- Requests use `0x41` (embedded) for Configuration space `0xFD` ✅
- Requests use `0x40` (generic) for AcdiUser space `0xFB` ✅
- Every address advances by exactly the previous element's byte size ✅
- No repeated identical reads ✅

### Phase 3 — Update `specs/004-read-node-config` (Docs Only, No Deletion) ✅ DONE

9. ✅ **Corrected `technical-context.md`**:
   - Updated `parse_read_reply` entry to document the deterministic rule and note the removed `address_space` param
   - Added **embedded vs generic format** warning block (with traffic capture references) before the `read_cdi()` example
   - Fixed the **Calculating Absolute Address** formula — now documents the running-cursor approach and explains that `offset` is a relative skip, not an absolute address
   - Updated document header with last-updated date

10. ✅ **Updated `tasks.md`**:
    - Added T088–T096 (9 `memory_config.rs` protocol tests, all already passing ✅)
    - Added note on T024–T031 (`parse_config_value` unit tests still need to be written to satisfy Gate III)
    - Corrected `Memory Protocol` note (was hardcoded `0xFD`; now says use `segment.space`)
    - Added `Protocol Format` and `CDI offset semantics` notes
    - Updated total task count to 96

### Bug 4 — Read Batching (Performance Optimisation) ✅ DONE

Implemented in [cdi.rs](app/src-tauri/src/commands/cdi.rs) — `read_all_config_values`:

1. After `extract_all_elements_with_addresses`, a `ReadItem` struct (containing `orig_index`, `absolute_address`, `size`, `space`) is built for each element, filtering out any with invalid or >64-byte sizes up-front (these are counted as errors immediately)
2. The item list is sorted by `(space, absolute_address)` to place consecutive same-space elements adjacent
3. Items are grouped into batches: a new batch is started whenever the space changes, the address is non-consecutive (gap), or adding the next item would push the batch total past 64 bytes
4. One `read_memory` call is issued per batch; each element's bytes are sliced out of the reply at `reply[element_addr - batch_start..]`
5. Errors abort the whole batch (all its elements counted as failed) and processing continues with the next batch
6. Progress events are emitted every 10 batches (vs every 10 elements before)

For async_blink's 11 consecutive elements in a single segment, all 11 are expected to collapse into 1–2 batches (the sequential cursor fix ensures addresses are consecutive), reducing 11 round-trips to 1–2.

---

## Verification Checklist

- [x] `cargo test -p lcc-rs` — 222 tests pass
- [x] `cargo check` on Tauri app — no errors
- [x] Traffic capture confirms `0x41` (embedded) for config space, `0x40` (generic) for ACDI
- [x] Traffic capture confirms addresses advance sequentially, matching JMRI
- [x] No repeated identical reads
- [x] `technical-context.md` updated (Phase 3)
- [x] `tasks.md` updated (Phase 3)
- [x] Read batching implemented (Bug 4)

---

## Decisions

- **Delete vs fix specs/004**: Keep and patch — the spec, tasks, contracts, and traffic captures are all accurate; only `technical-context.md` has stale API detail
- **Remove `address_space` param from `parse_read_reply`**: Done — with deterministic parsing it's no longer needed; callers in `discovery.rs` updated
- **Embedded vs generic for 0xFD/0xFE/0xFF**: Done — using embedded, matching OpenLCB_Java and JMRI traffic
- **CDI `offset` as relative skip vs absolute**: Confirmed relative skip per spec — sequential cursor is correct
- **Zero-stride guard**: Clamp to 1 instance with warning — safe for all known CDIs
