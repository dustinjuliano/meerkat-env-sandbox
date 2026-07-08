//! Block range allocator with freelist reuse and dynamic growth

use super::{BlockId, Interval};

/// Allocator managing block range reuse and freelist tracking
pub(super) struct BlockAllocator {
    pub(super) block_freelist: Vec<Interval>,
    pub(super) block_next_id: BlockId,
}

impl BlockAllocator {
    /// Creates a new empty block allocator
    pub(super) fn new() -> Self {
        Self::default()
    }
}

impl Default for BlockAllocator {
    /// Creates a default block allocator starting at BlockId(1)
    fn default() -> Self {
        BlockAllocator {
            block_freelist: Vec::new(),
            block_next_id: BlockId(1),
        }
    }
}

impl BlockAllocator {
    /// Allocates a range of block identifiers
    ///
    /// Searches the freelist first, splitting any larger blocks found
    /// to satisfy the requested size, and falls back to growing the
    /// block arena if no suitable block is found on the freelist
    ///
    /// Args:
    ///     `size` (`u32`): The number of block identifiers to allocate
    ///     `block_arena` (`&mut Vec<super::block::Block>`): The block
    ///         arena
    ///
    /// Returns:
    ///     `Option<Interval>`: The allocated block interval range, or
    ///         `None` if the allocation would exceed limit constraints
    pub(super) fn alloc_block_range(
        &mut self,
        size: u32,
        block_arena: &mut Vec<super::block::Block>,
    ) -> Option<Interval> {
        let mut found_interval = None;
        for i in 0..self.block_freelist.len() {
            let r = self.block_freelist[i];
            debug_assert!(r.end.0 >= r.begin.0, "freelist interval invariant violated");
            let r_size = (r.end.0) - (r.begin.0);
            if r_size >= size {
                debug_assert!(
                    r.begin.0 <= r.end.0.saturating_sub(size),
                    "freelist split invariant violated"
                );
                self.block_freelist.swap_remove(i);
                if r_size > size {
                    self.block_freelist.push(Interval {
                        begin: BlockId((r.begin.0) + size),
                        end: r.end,
                    });
                }
                found_interval = Some((r.begin, BlockId((r.begin.0) + size)));
                break;
            }
        }

        let (begin, end) = match found_interval {
            Some(range) => range,
            None => {
                let begin = self.block_next_id;
                // Enforce that `begin` itself does not exceed the
                // maximum allowed value, ensuring adherence to the
                // invariant documented on `MAX_BLOCK_ID` in `env.rs`
                if begin.0 > super::MAX_BLOCK_ID {
                    return None;
                }
                // Enforce that the end boundary `begin + size` fits
                // inside a `u32`, since the interval is half-open
                // and `end` can be up to `u32::MAX`
                if ((begin.0 as u64) + (size as u64)) > (u32::MAX as u64) {
                    return None;
                }
                let end = BlockId((begin.0) + size);
                self.block_next_id = end;

                while (block_arena.len()) < ((end.0 as usize).saturating_sub(1)) {
                    block_arena.push(super::block::Block::default());
                }
                (begin, end)
            }
        };

        Some(Interval { begin, end })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verifies default initialization and new() of `BlockAllocator`
    #[test]
    fn test_new() {
        let alloc = BlockAllocator::new();
        assert_eq!(alloc.block_freelist.len(), 0);
        assert_eq!(alloc.block_next_id.0, 1);

        let def = BlockAllocator::default();
        assert_eq!(def.block_freelist.len(), 0);
        assert_eq!(def.block_next_id.0, 1);
    }

    /// Verifies block range allocation when the freelist is empty
    #[test]
    fn test_alloc_range_no_freelist() {
        let mut alloc = BlockAllocator::new();
        let mut arena = Vec::new();
        let r1 = alloc.alloc_block_range(5, &mut arena).unwrap();
        assert_eq!(r1.begin.0, 1);
        assert_eq!(r1.end.0, 6);
        assert_eq!(arena.len(), 5);
    }

    /// Verifies exact-match block range reuse from the freelist
    #[test]
    fn test_alloc_range_freelist_exact() {
        let mut alloc = BlockAllocator::new();
        let mut arena = Vec::new();
        let r1 = alloc.alloc_block_range(5, &mut arena).unwrap();
        alloc.block_freelist.push(r1);

        let r2 = alloc.alloc_block_range(5, &mut arena).unwrap();
        assert_eq!(r2.begin.0, 1);
        assert_eq!(r2.end.0, 6);
        assert_eq!(alloc.block_freelist.len(), 0);
    }

    /// Verifies partial-match block range reuse and remainder splitting
    #[test]
    fn test_alloc_range_freelist_partial() {
        let mut alloc = BlockAllocator::new();
        let mut arena = Vec::new();
        let r1 = alloc.alloc_block_range(5, &mut arena).unwrap();
        alloc.block_freelist.push(r1);

        let r2 = alloc.alloc_block_range(3, &mut arena).unwrap();
        assert_eq!(r2.begin.0, 1);
        assert_eq!(r2.end.0, 4);
        assert_eq!(alloc.block_freelist.len(), 1);
        assert_eq!(alloc.block_freelist[0].begin.0, 4);
        assert_eq!(alloc.block_freelist[0].end.0, 6);
    }

    /// Verifies allocation at the maximum boundaries (MAX_BLOCK_ID / u32::MAX)
    #[test]
    fn test_alloc_boundary_limits() {
        let mut alloc = BlockAllocator::new();
        let mut arena = Vec::new();

        // 1. If begin = u32::MAX, it exceeds MAX_BLOCK_ID, so it should fail (returns None early, no OOM)
        alloc.block_next_id = BlockId(u32::MAX);
        let r1 = alloc.alloc_block_range(1, &mut arena);
        assert!(r1.is_none());

        // 2. Size = 0 allocation at begin = u32::MAX should fail (exceeds MAX_BLOCK_ID) (returns None early, no OOM)
        let r2 = alloc.alloc_block_range(0, &mut arena);
        assert!(r2.is_none());

        // 3. Overflows u32 limit (begin = MAX_BLOCK_ID, size = 2) should fail (returns None early, no OOM)
        alloc.block_next_id = BlockId(u32::MAX - 1);
        let r3 = alloc.alloc_block_range(2, &mut arena);
        assert!(r3.is_none());
    }
}
