//! Alias allocation protocol for LCC/OpenLCB
//!
//! This module implements the CAN alias allocation sequence per OpenLCB S-9.7.2.1:
//! 1. Send 4 CID frames (CID7→CID4): same alias, NodeID segments in header bits 23:12
//! 2. Listen 400 ms for conflict responses (RID from another node using our alias)
//! 3. Send RID frame to reserve the alias
//! 4. Send InitializationComplete frame carrying the 6-byte NodeID
//!
//! CID frame header layout (29-bit CAN extended ID):
//!   bits 27:24 = CID type (7, 6, 5, 4 for CID7–CID4)  bit 28 always 1
//!   bits 23:12 = 12-bit segment of the NodeID
//!   bits 11:0  = 12-bit alias candidate

use crate::{
    Result, Error,
    types::{NodeID, NodeAlias},
    protocol::GridConnectFrame,
    transport::LccTransport,
};
use crate::protocol::MTI;
use crate::constants::{ALIAS_CONFLICT_LISTEN_MS, MAX_ALIAS_ATTEMPTS};
use std::time::Duration;
use tokio::time::{timeout, Instant};

/// Alias allocator for negotiating a unique alias for this node
pub struct AliasAllocator;

impl AliasAllocator {
    /// Allocate a unique alias for the given Node ID.
    ///
    /// Per OpenLCB S-9.7.2.1:
    /// - Generates candidate aliases using the standard LFSR algorithm (NIDa)
    /// - Sends CID7, CID6, CID5, CID4 frames (same alias; NodeID segments in header)
    /// - Waits 400 ms for any RID conflict frame carrying our alias
    /// - On no conflict: sends RID then InitializationComplete (with 6-byte NodeID)
    /// - On conflict: advances the LFSR to a pseudo-random next candidate and retries
    pub async fn allocate(
        node_id: &NodeID,
        transport: &mut Box<dyn LccTransport>,
    ) -> Result<NodeAlias> {
        // Split the 48-bit NodeID into four 12-bit segments.
        // CID7 carries the most-significant 12 bits, CID4 carries the least-significant.
        let id = node_id.as_bytes();
        let node_val: u64 = (id[0] as u64) << 40
            | (id[1] as u64) << 32
            | (id[2] as u64) << 24
            | (id[3] as u64) << 16
            | (id[4] as u64) << 8
            | (id[5] as u64);
        let segments: [u32; 4] = [
            ((node_val >> 36) & 0xFFF) as u32, // CID7 — bits 47:36
            ((node_val >> 24) & 0xFFF) as u32, // CID6 — bits 35:24
            ((node_val >> 12) & 0xFFF) as u32, // CID5 — bits 23:12
            (node_val & 0xFFF) as u32,          // CID4 — bits 11:0
        ];

        let mut gen = AliasGenerator::new(node_id);

        for attempt in 0u32..MAX_ALIAS_ATTEMPTS {
            let alias_val = if attempt == 0 {
                gen.current_alias()
            } else {
                gen.next_alias()
            };
            let alias = NodeAlias::new(alias_val)
                .map_err(|e| Error::AliasAllocation(e))?;

            // Send CID7→CID4: header = (0x17..0x14 << 24) | (segment << 12) | alias
            for (cid_type, &seg) in [0x17u32, 0x16, 0x15, 0x14].iter().zip(segments.iter()) {
                let header = (*cid_type << 24) | (seg << 12) | (alias_val as u32);
                let frame = GridConnectFrame::new(header, vec![])?;
                transport.send(&frame).await?;
            }

            // Wait for conflict per OpenLCB S-9.7.2.1 (400 ms gives slower
            // bridges and gateways time to relay RID frames)
            let conflict = Self::listen_for_conflict(
                transport,
                alias_val,
                Duration::from_millis(ALIAS_CONFLICT_LISTEN_MS),
            )
            .await?;

            if !conflict {
                // Reserve the alias
                let rid = GridConnectFrame::from_mti(MTI::ReserveID, alias_val, vec![])?;
                transport.send(&rid).await?;

                // Announce: InitializationComplete carrying our 6-byte NodeID
                let init = GridConnectFrame::from_mti(
                    MTI::InitializationComplete,
                    alias_val,
                    id.to_vec(),
                )?;
                transport.send(&init).await?;

                return Ok(alias);
            }
        }

        Err(Error::AliasAllocation(
            format!("Failed to allocate alias after {} attempts", MAX_ALIAS_ATTEMPTS),
        ))
    }

    /// Listen on the transport for up to `wait` ms.
    ///
    /// Returns `true` if any incoming frame has `alias_val` as its source alias
    /// (indicating that alias is already in use by another node).
    async fn listen_for_conflict(
        transport: &mut Box<dyn LccTransport>,
        alias_val: u16,
        wait: Duration,
    ) -> Result<bool> {
        let deadline = Instant::now() + wait;

        loop {
            let remaining = match deadline.checked_duration_since(Instant::now()) {
                Some(d) => d,
                None => break,
            };
            let poll_ms = remaining.as_millis().min(50) as u64;

            match timeout(
                Duration::from_millis(poll_ms + 1),
                transport.receive(poll_ms),
            )
            .await
            {
                Ok(Ok(Some(frame))) => {
                    // Any frame whose source alias equals ours is a conflict
                    if frame.source_alias() == alias_val {
                        return Ok(true);
                    }
                }
                Ok(Ok(None)) | Ok(Err(_)) | Err(_) => {
                    // No frame, transport error, or poll timeout — keep waiting
                }
            }
        }

        Ok(false)
    }
}

/// LFSR-based alias generator per OpenLCB S-9.7.2.1.
///
/// Ports the NIDa algorithm from OpenLCB_Java: a 48-bit state (two 24-bit
/// registers) seeded from the NodeID, stepped with a nonlinear feedback
/// function, and reduced to a 12-bit alias.
struct AliasGenerator {
    lfsr1: u32,
    lfsr2: u32,
    current_alias: u16,
}

impl AliasGenerator {
    /// Create a new generator seeded from the given NodeID.
    /// The first alias is immediately available via `current_alias()`.
    fn new(node_id: &NodeID) -> Self {
        let id = node_id.as_bytes();
        let lfsr1 = ((id[0] as u32) << 16) | ((id[1] as u32) << 8) | (id[2] as u32);
        let lfsr2 = ((id[3] as u32) << 16) | ((id[4] as u32) << 8) | (id[5] as u32);
        let mut gen = Self { lfsr1, lfsr2, current_alias: 0 };
        gen.next_alias(); // Step once to produce first alias (matches NIDa constructor)
        gen
    }

    /// Advance the LFSR and compute the next alias.
    /// Skips alias 0 (invalid per OpenLCB S-9.7.2.1 §6.3).
    fn next_alias(&mut self) -> u16 {
        loop {
            self.step();
            let alias = self.compute_alias();
            if alias != 0 {
                self.current_alias = alias;
                return alias;
            }
        }
    }

    /// Get the current alias without advancing the generator.
    fn current_alias(&self) -> u16 {
        self.current_alias
    }

    /// Step the LFSR (matches OpenLCB_Java NIDa.stepGenerator)
    fn step(&mut self) {
        let temp1 = ((self.lfsr1 << 9) | ((self.lfsr2 >> 15) & 0x1FF)) & 0xFF_FFFF;
        let temp2 = (self.lfsr2 << 9) & 0xFF_FFFF;

        self.lfsr2 = self.lfsr2.wrapping_add(temp2).wrapping_add(0x7A_4BA9);
        self.lfsr1 = self.lfsr1.wrapping_add(temp1).wrapping_add(0x1B_0CA3);

        // Carry from lfsr2 overflow into lfsr1
        self.lfsr1 = (self.lfsr1 & 0xFF_FFFF).wrapping_add((self.lfsr2 & 0xFF00_0000) >> 24);
        self.lfsr2 &= 0xFF_FFFF;
    }

    /// Compute 12-bit alias from current LFSR state
    fn compute_alias(&self) -> u16 {
        ((self.lfsr1 ^ self.lfsr2 ^ (self.lfsr1 >> 12) ^ (self.lfsr2 >> 12)) & 0xFFF) as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeID;

    /// Verify the LFSR generator produces the correct first alias for known NodeIDs.
    ///
    /// Reference values computed from OpenLCB_Java NIDa algorithm:
    ///   loadSeed → stepGenerator → computeAliasFromGenerator
    #[test]
    fn test_lfsr_first_alias_from_node_id() {
        // Test vector 1: NodeID used in OpenLCB_Java NIDaTest setUp
        let gen1 = AliasGenerator::new(&NodeID::new([0x00, 0x00, 0x00, 0x01, 0x00, 0x02]));
        assert_eq!(
            gen1.current_alias(), 0x50A,
            "First alias for NodeID 00.00.00.01.00.02 should be 0x50A"
        );

        // Test vector 2: NodeID used in existing CID segment tests
        let gen2 = AliasGenerator::new(&NodeID::new([0x02, 0x03, 0x04, 0x05, 0x06, 0x07]));
        assert_eq!(
            gen2.current_alias(), 0x285,
            "First alias for NodeID 02.03.04.05.06.07 should be 0x285"
        );
    }

    /// Verify successive aliases are all non-zero and no two consecutive are equal.
    #[test]
    fn test_lfsr_successive_aliases_differ_and_nonzero() {
        let mut gen = AliasGenerator::new(&NodeID::new([0x02, 0x03, 0x04, 0x05, 0x06, 0x07]));
        let mut prev = gen.current_alias();
        assert_ne!(prev, 0, "First alias must not be 0");

        for i in 0..100 {
            let next = gen.next_alias();
            assert_ne!(next, 0, "Alias {} must not be 0", i);
            assert_ne!(next, prev, "Alias {} must differ from previous", i);
            prev = next;
        }
    }

    /// Verify that the NodeID segment extraction matches the reference sequence
    /// from testStartup.py (NodeID = 02.03.04.05.06.07, alias = 0x573).
    ///
    /// Expected CID frames:
    ///   CID7: :X17020573N;  (segment 0x020)
    ///   CID6: :X16304573N;  (segment 0x304)
    ///   CID5: :X15050573N;  (segment 0x050)  — note: bits 23:12 of 0x020304050607
    ///   CID4: :X14607573N;  (segment 0x607)
    #[test]
    fn test_cid_frame_headers() {
        let id: u64 = 0x020304050607;
        let segs: [u32; 4] = [
            ((id >> 36) & 0xFFF) as u32,
            ((id >> 24) & 0xFFF) as u32,
            ((id >> 12) & 0xFFF) as u32,
            (id & 0xFFF) as u32,
        ];
        let alias: u32 = 0x573;
        let cid_types = [0x17u32, 0x16, 0x15, 0x14];
        let expected_headers = [0x17020573u32, 0x16304573, 0x15050573, 0x14607573];

        for i in 0..4 {
            let header = (cid_types[i] << 24) | (segs[i] << 12) | alias;
            assert_eq!(
                header, expected_headers[i],
                "CID{} header mismatch: got {:#010X}, want {:#010X}",
                7 - i,
                header,
                expected_headers[i]
            );
        }
    }

    /// Verify InitComplete carries the full NodeID payload
    #[test]
    fn test_init_complete_carries_node_id() {
        let node_id = NodeID::new([0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        let alias = 0xAAA;
        let frame =
            GridConnectFrame::from_mti(MTI::InitializationComplete, alias, node_id.as_bytes().to_vec())
                .unwrap();
        assert_eq!(frame.to_string(), ":X19100AAAN010203040506;");
    }

    /// Verify allocate() uses the LFSR generator for its alias candidate
    #[tokio::test]
    async fn test_allocate_uses_lfsr_alias() {
        use crate::transport::mock::MockTransport;

        let node_id = NodeID::new([0x00, 0x00, 0x00, 0x01, 0x00, 0x02]);
        let expected_alias = AliasGenerator::new(&node_id).current_alias();

        // No conflict responses → first LFSR alias should be accepted
        let mut transport: Box<dyn LccTransport> = Box::new(MockTransport::new());
        let alias = AliasAllocator::allocate(&node_id, &mut transport)
            .await
            .expect("allocation should succeed");

        assert_eq!(
            alias.value(), expected_alias,
            "allocate() should use the LFSR generator's first alias"
        );
    }
}
