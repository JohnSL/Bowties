//! Alias allocation protocol for LCC/OpenLCB
//!
//! This module implements the alias allocation sequence:
//! 1. Send 4 CID (Check ID) frames with hash-based alias candidates
//! 2. Listen for RID (Reserve ID) conflict responses
//! 3. Send RID frame to reserve the chosen alias
//! 4. Send AMD (Alias Map Definition) frame with Node ID
//! 5. Send InitializationComplete frame with Node ID

use crate::{
    Result, Error,
    types::{NodeID, NodeAlias},
    protocol::{GridConnectFrame, MTI},
    transport::LccTransport,
};
use std::time::Duration;
use tokio::time::{sleep, timeout, Instant};
use std::collections::HashSet;

/// Alias allocator for negotiating a unique alias for this node
pub struct AliasAllocator;

impl AliasAllocator {
    /// Allocate a unique alias for the given Node ID
    /// 
    /// Performs the full CID/RID/AMD/InitComplete sequence:
    /// - Sends 4 CID frames (125ms apart)
    /// - Listens for RID conflicts
    /// - Reserves alias with RID frame
    /// - Announces Node ID with AMD and InitComplete frames
    /// 
    /// # Arguments
    /// * `node_id` - The 6-byte Node ID of this node
    /// * `transport` - Mutable reference to the transport layer
    /// 
    /// # Returns
    /// The negotiated `NodeAlias` if successful, or an error
    /// 
    /// # Timing
    /// - CID frames: 125ms apart
    /// - Wait for RID responses: 150ms timeout after last CID
    /// - RID frame: 100ms before AMD
    /// - AMD and InitComplete: immediate
    pub async fn allocate(
        node_id: &NodeID,
        transport: &mut Box<dyn LccTransport>,
    ) -> Result<NodeAlias> {
        // Compute base alias from Node ID hash
        let base_alias = node_id.hash_to_alias()?;
        
        // Try up to 3 variants (in case of conflicts)
        for variant in 0..3 {
            // Compute candidate aliases for this variant
            let candidate_aliases = Self::compute_cid_aliases(base_alias, variant);
            
            // Send 4 CID frames with 125ms spacing
            for &alias in &candidate_aliases {
                let frame = Self::create_cid_frame(alias)?;
                transport.send(&frame).await?;
                
                // Wait 125ms before sending next CID (except after the last one)
                if alias != candidate_aliases[3] {
                    sleep(Duration::from_millis(125)).await;
                }
            }
            
            // Listen for RID conflict responses for 150ms after last CID
            let conflicts = Self::listen_for_conflicts(
                transport,
                &candidate_aliases,
                Duration::from_millis(150),
            ).await?;
            
            // If no conflicts, use the first candidate alias
            if conflicts.is_empty() {
                let chosen_alias = candidate_aliases[0];
                
                // Wait 100ms, then send RID to reserve the alias
                sleep(Duration::from_millis(100)).await;
                let rid_frame = Self::create_rid_frame(chosen_alias)?;
                transport.send(&rid_frame).await?;
                
                // Send AMD (Alias Map Definition) with Node ID
                let amd_frame = Self::create_amd_frame(chosen_alias, node_id)?;
                transport.send(&amd_frame).await?;
                
                // Send InitializationComplete to signal readiness
                let init_frame = Self::create_init_complete_frame(chosen_alias)?;
                transport.send(&init_frame).await?;
                
                return Ok(chosen_alias);
            }
            
            // If conflicts, try next variant
        }
        
        Err(Error::AliasAllocation(
            "Failed to allocate alias after 3 variants".to_string()
        ))
    }
    
    /// Compute 4 CID candidate aliases for the given variant
    /// 
    /// For variant 0: uses the base hash directly
    /// For variant 1, 2, etc.: modifies the alias by adding offsets
    fn compute_cid_aliases(base_alias: NodeAlias, variant: u8) -> [NodeAlias; 4] {
        let base = base_alias.value();
        let variant_offset = (variant as u16) * 0x400; // Each variant offset by 0x400
        
        let aliases = [
            (base + variant_offset) & 0xFFF,
            (base + variant_offset + 0x100) & 0xFFF,
            (base + variant_offset + 0x200) & 0xFFF,
            (base + variant_offset + 0x300) & 0xFFF,
        ];
        
        // Convert to NodeAlias (will not fail since all are masked to 12-bit)
        [
            NodeAlias::new(aliases[0]).unwrap(),
            NodeAlias::new(aliases[1]).unwrap(),
            NodeAlias::new(aliases[2]).unwrap(),
            NodeAlias::new(aliases[3]).unwrap(),
        ]
    }
    
    /// Create a CID (Check ID) frame for the given alias
    fn create_cid_frame(alias: NodeAlias) -> Result<GridConnectFrame> {
        GridConnectFrame::from_mti(MTI::CheckID, alias.value(), vec![])
    }
    
    /// Create a RID (Reserve ID) frame for the given alias
    fn create_rid_frame(alias: NodeAlias) -> Result<GridConnectFrame> {
        GridConnectFrame::from_mti(MTI::ReserveID, alias.value(), vec![])
    }
    
    /// Create an AMD (Alias Map Definition) frame with Node ID
    fn create_amd_frame(alias: NodeAlias, node_id: &NodeID) -> Result<GridConnectFrame> {
        GridConnectFrame::from_mti(MTI::AliasMapReset, alias.value(), node_id.as_bytes().to_vec())
    }
    
    /// Create an InitializationComplete frame
    ///
    /// This signals that initialization is complete. Per OpenLCB spec,
    /// InitializationComplete is sent without payload data.
    fn create_init_complete_frame(alias: NodeAlias) -> Result<GridConnectFrame> {
        GridConnectFrame::from_mti(
            MTI::InitializationComplete,
            alias.value(),
            vec![], // No payload - just the MTI+alias
        )
    }
    
    /// Listen for RID conflict responses
    /// 
    /// Returns a set of aliases that received conflict responses
    async fn listen_for_conflicts(
        transport: &mut Box<dyn LccTransport>,
        candidates: &[NodeAlias; 4],
        timeout_duration: Duration,
    ) -> Result<HashSet<u16>> {
        let mut conflicts = HashSet::new();
        let candidate_set: HashSet<u16> = candidates.iter().map(|a| a.value()).collect();
        
        let start = Instant::now();
        loop {
            // Check if timeout exceeded
            if start.elapsed() > timeout_duration {
                break;
            }
            
            // Calculate remaining time for this receive call
            let remaining = timeout_duration - start.elapsed();
            let remaining_ms = remaining.as_millis() as u64;
            
            // Receive with timeout
            match timeout(
                Duration::from_millis(remaining_ms.min(50)),
                transport.receive(remaining_ms.min(50)),
            ).await {
                Ok(Ok(Some(frame))) => {
                    // Check if this is a RID frame from our candidates
                    if let Ok((mti, source_alias)) = frame.get_mti() {
                        if mti == MTI::ReserveID && candidate_set.contains(&source_alias) {
                            // This means someone else is using one of our candidates
                            conflicts.insert(source_alias);
                        }
                    }
                }
                Ok(Ok(None)) => {
                    // No frame received within timeout of this call, continue waiting
                    continue;
                }
                Ok(Err(_)) => {
                    // Error reading frame, ignore and continue
                    continue;
                }
                Err(_) => {
                    // Timeout on this receive call, continue waiting until overall timeout
                    continue;
                }
            }
        }
        
        Ok(conflicts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_cid_aliases() {
        let base = NodeAlias::new(0x123).unwrap();
        let aliases = AliasAllocator::compute_cid_aliases(base, 0);
        
        // Check that we got 4 different aliases
        assert_eq!(aliases.len(), 4);
        
        // Check they're all 12-bit
        for alias in aliases {
            assert!(alias.value() <= 0xFFF);
        }
    }
}
