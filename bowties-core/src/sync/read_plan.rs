//! Range planner: merges (space, address, length) items into the fewest reads
//! of at most [`MAX_READ_BYTES`] bytes.
//!
//! Modelled on JMRI/OpenLCB_Java `RangeCacheUtil` / `MemorySpaceCache`, which
//! read a nearby range in one request rather than one request per field.
//! Items in the same space whose gap is [`MAX_MERGE_GAP`] bytes or less share a
//! read (the gap bytes are read and discarded); items longer than one read are
//! split into 64-byte chunks.

/// Largest single memory read.
pub const MAX_READ_BYTES: u32 = 64;

/// Largest gap between two items that still shares one read.
pub const MAX_MERGE_GAP: u32 = 8;

/// One range the caller wants read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadItem<K> {
    pub space: u8,
    pub address: u32,
    pub length: u32,
    pub key: K,
}

/// The part of an item covered by a [`PlannedRead`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSlice<K> {
    pub key: K,
    /// Offset of this slice within the read's bytes.
    pub offset: u32,
    pub length: u32,
    /// Offset of this slice within its item (non-zero for later chunks of a split item).
    pub item_offset: u32,
}

/// One read to issue, with the items it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRead<K> {
    pub space: u8,
    pub address: u32,
    pub length: u32,
    pub items: Vec<PlannedSlice<K>>,
}

/// Plan the reads for `items`, ordered by `(space, address)`.
pub fn plan_reads<K: Clone>(items: &[ReadItem<K>]) -> Vec<PlannedRead<K>> {
    // Split items longer than one read into chunks first.
    struct Chunk<K> {
        space: u8,
        address: u32,
        length: u32,
        key: K,
        item_offset: u32,
    }
    let mut chunks: Vec<Chunk<K>> = Vec::new();
    for item in items.iter().filter(|i| i.length > 0) {
        let mut done = 0;
        while done < item.length {
            let length = (item.length - done).min(MAX_READ_BYTES);
            chunks.push(Chunk {
                space: item.space,
                address: item.address + done,
                length,
                key: item.key.clone(),
                item_offset: done,
            });
            done += length;
        }
    }
    chunks.sort_by_key(|c| (c.space, c.address));

    let mut reads: Vec<PlannedRead<K>> = Vec::new();
    for chunk in chunks {
        if let Some(read) = reads.last_mut() {
            let end = read.address + read.length;
            let gap = chunk.address.saturating_sub(end);
            let new_end = end.max(chunk.address + chunk.length);
            if read.space == chunk.space
                && gap <= MAX_MERGE_GAP
                && new_end - read.address <= MAX_READ_BYTES
            {
                read.items.push(PlannedSlice {
                    key: chunk.key,
                    offset: chunk.address - read.address,
                    length: chunk.length,
                    item_offset: chunk.item_offset,
                });
                read.length = new_end - read.address;
                continue;
            }
        }
        reads.push(PlannedRead {
            space: chunk.space,
            address: chunk.address,
            length: chunk.length,
            items: vec![PlannedSlice {
                key: chunk.key,
                offset: 0,
                length: chunk.length,
                item_offset: chunk.item_offset,
            }],
        });
    }
    reads
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(space: u8, address: u32, length: u32, key: u32) -> ReadItem<u32> {
        ReadItem { space, address, length, key }
    }

    fn spans(reads: &[PlannedRead<u32>]) -> Vec<(u8, u32, u32)> {
        reads.iter().map(|r| (r.space, r.address, r.length)).collect()
    }

    #[test]
    fn contiguous_items_merge_into_one_read_with_offsets() {
        let reads = plan_reads(&[item(253, 0, 2, 1), item(253, 2, 4, 2), item(253, 6, 1, 3)]);
        assert_eq!(spans(&reads), vec![(253, 0, 7)]);
        let offsets: Vec<(u32, u32, u32)> =
            reads[0].items.iter().map(|s| (s.key, s.offset, s.length)).collect();
        assert_eq!(offsets, vec![(1, 0, 2), (2, 2, 4), (3, 6, 1)]);
    }

    #[test]
    fn gap_of_eight_bytes_merges() {
        let reads = plan_reads(&[item(253, 0, 2, 1), item(253, 10, 2, 2)]);
        assert_eq!(spans(&reads), vec![(253, 0, 12)]);
    }

    #[test]
    fn gap_of_nine_bytes_splits() {
        let reads = plan_reads(&[item(253, 0, 2, 1), item(253, 11, 2, 2)]);
        assert_eq!(spans(&reads), vec![(253, 0, 2), (253, 11, 2)]);
    }

    #[test]
    fn merged_read_never_exceeds_64_bytes() {
        let reads = plan_reads(&[item(253, 0, 40, 1), item(253, 40, 25, 2)]);
        assert_eq!(spans(&reads), vec![(253, 0, 40), (253, 40, 25)]);
        let reads = plan_reads(&[item(253, 0, 40, 1), item(253, 40, 24, 2)]);
        assert_eq!(spans(&reads), vec![(253, 0, 64)]);
    }

    #[test]
    fn different_spaces_never_merge() {
        let reads = plan_reads(&[item(254, 0, 1, 1), item(253, 0, 1, 2)]);
        assert_eq!(spans(&reads), vec![(253, 0, 1), (254, 0, 1)]);
    }

    #[test]
    fn item_over_64_bytes_splits_into_64_byte_chunks() {
        let reads = plan_reads(&[item(253, 100, 130, 7)]);
        assert_eq!(spans(&reads), vec![(253, 100, 64), (253, 164, 64), (253, 228, 2)]);
        let chunk_offsets: Vec<u32> = reads.iter().map(|r| r.items[0].item_offset).collect();
        assert_eq!(chunk_offsets, vec![0, 64, 128]);
    }

    #[test]
    fn reads_are_ordered_by_space_then_address() {
        let reads = plan_reads(&[item(253, 500, 1, 1), item(253, 0, 1, 2), item(251, 900, 1, 3)]);
        assert_eq!(spans(&reads), vec![(251, 900, 1), (253, 0, 1), (253, 500, 1)]);
    }

    #[test]
    fn overlapping_items_share_a_read() {
        let reads = plan_reads(&[item(253, 0, 4, 1), item(253, 2, 4, 2)]);
        assert_eq!(spans(&reads), vec![(253, 0, 6)]);
    }
}
