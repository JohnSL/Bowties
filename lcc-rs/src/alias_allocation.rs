//! Alias allocation protocol for LCC/OpenLCB
//!
//! This module implements the CAN alias allocation sequence per OpenLCB S-9.7.2.1:
//! 1. Send 4 CID frames (CID7→CID4): same alias, NodeID segments in header bits 23:12
//! 2. Listen 200 ms for conflict responses (RID from another node using our alias)
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
use std::time::Duration;
use tokio::time::{timeout, Instant};

/// Alias allocator for negotiating a unique alias for this node
pub struct AliasAllocator;

impl AliasAllocator {
    /// Allocate a unique alias for the given Node ID.
    ///
    /// Per OpenLCB S-9.7.2.1:
    /// - Sends CID7, CID6, CID5, CID4 frames (same alias; NodeID segments in header)
    /// - Waits 200 ms for any RID conflict frame carrying our alias
    /// - On no conflict: sends RID then InitializationComplete (with 6-byte NodeID)
    /// - Retries up to 3 times with offset aliases if a conflict is detected
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

        let base_alias = node_id
            .hash_to_alias()
            .map_err(|e| Error::AliasAllocation(e))?;

        // Try up to 3 alias candidates (offset by 0x400 each time)
        for attempt in 0u16..3 {
            let alias_val = (base_alias.value().wrapping_add(attempt * 0x400)) & 0xFFF;
            let alias = NodeAlias::new(alias_val)
                .map_err(|e| Error::AliasAllocation(e))?;

            // Send CID7→CID4: header = (0x17..0x14 << 24) | (segment << 12) | alias
            for (cid_type, &seg) in [0x17u32, 0x16, 0x15, 0x14].iter().zip(segments.iter()) {
                let header = (*cid_type << 24) | (seg << 12) | (alias_val as u32);
                let frame = GridConnectFrame::new(header, vec![])?;
                transport.send(&frame).await?;
            }

            // Wait 200 ms; abort early if we see a frame bearing our alias (conflict)
            let conflict = Self::listen_for_conflict(
                transport,
                alias_val,
                Duration::from_millis(200),
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
            "Failed to allocate alias after 3 attempts".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NodeID;

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
}
